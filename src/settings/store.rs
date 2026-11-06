//! Stockage persistant des réglages : fichier JSON (surcharges seulement), écriture atomique,
//! journal d'audit JSONL et notification des changements.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::watch;

use super::schema::{self, defaults_from, Kind, Value, Values};
use crate::config::Config;

/// Nom du journal d'audit, créé dans le dossier du fichier de réglages.
pub const AUDIT_FILE: &str = "settings-audit.jsonl";

/// Erreur de réglage ; `Display` est en français et montrable à un administrateur.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("réglage inconnu : {0}")]
    UnknownKey(String),
    #[error("{0} est en lecture seule")]
    ReadOnly(&'static str),
    #[error("{key} : {reason}")]
    Invalid { key: &'static str, reason: String },
    #[error("fichier de réglages illisible : {0}")]
    Corrupt(String),
    #[error("écriture des réglages impossible : {0}")]
    Io(#[from] std::io::Error),
}

/// Résultat d'un `set` réussi (aussi diffusé aux abonnés).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub key: &'static str,
    pub old: Value,
    pub new: Value,
    /// true = appliqué à chaud ; false = effet au prochain redémarrage.
    pub hot: bool,
}

struct Inner {
    path: PathBuf,
    audit_path: PathBuf,
    defaults: Values,
    /// Surcharges persistées (le reste vient de `defaults`, donc de l'environnement).
    overrides: RwLock<Values>,
    /// Sérialise les `set` (écriture fichier + audit dans le même ordre que la mémoire).
    write: Mutex<()>,
    tx: watch::Sender<Option<Change>>,
}

/// Réglages partagés ; `Clone` ne copie qu'un `Arc`.
#[derive(Clone)]
pub struct SettingsStore {
    inner: Arc<Inner>,
}

impl SettingsStore {
    /// Charge `path` (absent = aucun réglage modifié). Le fichier surcharge les défauts de `cfg`.
    /// JSON corrompu = erreur ; clé inconnue ou valeur devenue invalide = ignorée (avertissement).
    pub fn load(path: &Path, cfg: &Config) -> Result<Self, SettingsError> {
        let mut overrides = Values::new();
        match fs::read_to_string(path) {
            Ok(text) => {
                let raw: BTreeMap<String, String> = serde_json::from_str(&text)
                    .map_err(|e| SettingsError::Corrupt(e.to_string()))?;
                for (k, v) in raw {
                    match schema::spec(&k) {
                        Some(s) if s.kind != Kind::ReadOnly => match schema::parse(s, &v) {
                            Ok(val) => {
                                overrides.insert(s.key, val);
                            }
                            Err(e) => tracing::warn!(key = %k, "réglage ignoré : {e}"),
                        },
                        _ => tracing::warn!(key = %k, "réglage inconnu ignoré"),
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        let dir = path.parent().unwrap_or(Path::new("."));
        Ok(Self {
            inner: Arc::new(Inner {
                path: path.to_path_buf(),
                audit_path: dir.join(AUDIT_FILE),
                defaults: defaults_from(cfg),
                overrides: RwLock::new(overrides),
                write: Mutex::new(()),
                tx: watch::channel(None).0,
            }),
        })
    }

    /// Valeur courante d'une clé (surcharge sinon défaut) ; `None` si la clé est inconnue.
    pub fn get(&self, key: &str) -> Option<Value> {
        let o = self
            .inner
            .overrides
            .read()
            .unwrap_or_else(|p| p.into_inner());
        o.get(key).or_else(|| self.inner.defaults.get(key)).cloned()
    }

    /// Entier ; `None` si clé inconnue ou d'un autre type.
    pub fn get_u64(&self, key: &str) -> Option<u64> {
        match self.get(key)? {
            Value::U64(n) => Some(n),
            _ => None,
        }
    }

    /// Booléen ; `None` si clé inconnue ou d'un autre type.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get(key)? {
            Value::Bool(b) => Some(b),
            _ => None,
        }
    }

    /// Choix (ou texte lecture seule) ; `None` si clé inconnue ou d'un autre type.
    pub fn get_choice(&self, key: &str) -> Option<String> {
        match self.get(key)? {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }

    /// Liste d'IDs triée ; `None` si clé inconnue ou d'un autre type.
    pub fn get_ids(&self, key: &str) -> Option<Vec<u64>> {
        match self.get(key)? {
            Value::Ids(v) => Some(v),
            _ => None,
        }
    }

    /// Liste ordonnée ; `None` si clé inconnue ou d'un autre type.
    pub fn get_order(&self, key: &str) -> Option<Vec<String>> {
        match self.get(key)? {
            Value::List(v) => Some(v),
            _ => None,
        }
    }

    /// Toutes les valeurs courantes (pour les menus).
    pub fn snapshot(&self) -> Values {
        let o = self
            .inner
            .overrides
            .read()
            .unwrap_or_else(|p| p.into_inner());
        let mut all = self.inner.defaults.clone();
        all.extend(o.iter().map(|(k, v)| (*k, v.clone())));
        all
    }

    /// Abonnement aux changements : la dernière `Change` appliquée (`None` avant le premier).
    pub fn subscribe(&self) -> watch::Receiver<Option<Change>> {
        self.inner.tx.subscribe()
    }

    /// Valide `raw` STRICTEMENT, persiste (atomique), journalise (audit + tracing) et notifie.
    /// `actor` = ID Telegram de l'administrateur (l'appelant a déjà vérifié qu'il est admin).
    /// En cas d'erreur, rien n'est modifié (ni mémoire, ni fichier).
    pub fn set(&self, key: &str, raw: &str, actor: u64) -> Result<Change, SettingsError> {
        let spec = schema::spec(key).ok_or_else(|| SettingsError::UnknownKey(clip(key)))?;
        if spec.kind == Kind::ReadOnly {
            return Err(SettingsError::ReadOnly(spec.key));
        }
        let new = schema::parse(spec, raw).map_err(|reason| SettingsError::Invalid {
            key: spec.key,
            reason,
        })?;
        let _w = self.inner.write.lock().unwrap_or_else(|p| p.into_inner());
        let old = self.get(spec.key).expect("clé du schéma");
        let mut next = self
            .inner
            .overrides
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        next.insert(spec.key, new.clone());
        let file: BTreeMap<&str, String> = next.iter().map(|(k, v)| (*k, v.to_string())).collect();
        let json =
            serde_json::to_vec_pretty(&file).map_err(|e| SettingsError::Corrupt(e.to_string()))?;
        write_atomic(&self.inner.path, &json)?;
        *self
            .inner
            .overrides
            .write()
            .unwrap_or_else(|p| p.into_inner()) = next;

        let change = Change {
            key: spec.key,
            old,
            new,
            hot: spec.hot,
        };
        tracing::info!(actor, key = change.key, old = %change.old, new = %change.new, "réglage modifié");
        if let Err(e) = self.audit(&change, actor) {
            tracing::warn!("journal d'audit des réglages non écrit : {e}");
        }
        self.inner.tx.send_replace(Some(change.clone()));
        Ok(change)
    }

    fn audit(&self, c: &Change, actor: u64) -> std::io::Result<()> {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let line = serde_json::json!({
            "ts": ts, "actor": actor, "key": c.key,
            "old": c.old.to_string(), "new": c.new.to_string(),
        });
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.inner.audit_path)?;
        restrict(&f)?;
        writeln!(f, "{line}")
    }
}

/// Coupe une clé inconnue (donnée de l'utilisateur) avant de la renvoyer dans un message.
fn clip(s: &str) -> String {
    s.chars().take(40).collect()
}

/// Droits 0600 sous Unix (aucun effet ailleurs).
fn restrict(f: &fs::File) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        f.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = f;
    Ok(())
}

/// Écrit dans un fichier temporaire du même dossier puis le renomme sur `path` :
/// le fichier final est soit l'ancien, soit le nouveau complet, jamais partiel.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = path.parent().unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .map_or("settings".into(), |n| n.to_string_lossy());
    let tmp = dir.join(format!(
        ".{name}.tmp-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let res = (|| {
        let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        restrict(&f)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)
    })();
    if res.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::schema::tests::cfg;

    const ADMIN: u64 = 111_111_111;

    fn store(dir: &Path) -> SettingsStore {
        SettingsStore::load(&dir.join("settings.json"), &cfg()).unwrap()
    }

    #[test]
    fn persistance_entre_deux_chargements() {
        let d = tempfile::tempdir().unwrap();
        let s = store(d.path());
        assert_eq!(s.get_u64("dl.max_filesize_mb"), Some(500));
        let c = s.set("dl.max_filesize_mb", "750", ADMIN).unwrap();
        assert_eq!(
            (c.old, c.new, c.hot),
            (Value::U64(500), Value::U64(750), true)
        );
        s.set("providers.order", "tiktok,x,bluesky", ADMIN).unwrap();
        s.set("acc.allowed_users", "5,4", ADMIN).unwrap();
        // nouveau chargement = redémarrage : le fichier l'emporte sur l'environnement
        let s2 = store(d.path());
        assert_eq!(s2.get_u64("dl.max_filesize_mb"), Some(750));
        assert_eq!(
            s2.get_order("providers.order").unwrap(),
            ["tiktok", "x", "bluesky"]
        );
        assert_eq!(s2.get_ids("acc.allowed_users"), Some(vec![4, 5]));
        assert_eq!(s2.get_bool("providers.tiktok.enabled"), Some(false));
        assert_eq!(s2.get_choice("sys.log_level").as_deref(), Some("debug"));
        assert_eq!(s2.get_u64("audio.music_enabled"), None); // mauvais type
        assert_eq!(s2.get_u64("inconnu"), None);
    }

    #[test]
    fn refus_ne_modifie_rien() {
        let d = tempfile::tempdir().unwrap();
        let s = store(d.path());
        assert!(matches!(
            s.set("dl.max_filesize_mb", "5000", ADMIN),
            Err(SettingsError::Invalid { .. })
        ));
        assert!(matches!(
            s.set("acc.admins", "@pseudo", ADMIN),
            Err(SettingsError::Invalid { .. })
        ));
        assert!(matches!(
            s.set("sys.version", "2", ADMIN),
            Err(SettingsError::ReadOnly(_))
        ));
        assert!(matches!(
            s.set("nope", "1", ADMIN),
            Err(SettingsError::UnknownKey(_))
        ));
        assert!(!d.path().join("settings.json").exists());
        assert!(!d.path().join(AUDIT_FILE).exists());
        assert_eq!(s.get_ids("acc.admins"), Some(vec![ADMIN]));
    }

    #[test]
    fn ecriture_atomique_sans_fichier_partiel() {
        let d = tempfile::tempdir().unwrap();
        let s = store(d.path());
        s.set("dl.max_duration_s", "600", ADMIN).unwrap();
        let ok = fs::read_to_string(d.path().join("settings.json")).unwrap();
        // la cible devient un dossier non vide : le rename échoue
        fs::remove_file(d.path().join("settings.json")).unwrap();
        fs::create_dir(d.path().join("settings.json")).unwrap();
        fs::write(d.path().join("settings.json").join("x"), "x").unwrap();
        assert!(matches!(
            s.set("dl.max_duration_s", "900", ADMIN),
            Err(SettingsError::Io(_))
        ));
        assert_eq!(
            s.get_u64("dl.max_duration_s"),
            Some(600),
            "mémoire inchangée"
        );
        let restes: Vec<_> = fs::read_dir(d.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp-"))
            .collect();
        assert!(restes.is_empty(), "temporaire laissé : {restes:?}");
        // le fichier écrit avant était un JSON complet
        let m: BTreeMap<String, String> = serde_json::from_str(&ok).unwrap();
        assert_eq!(m["dl.max_duration_s"], "600");
    }

    #[test]
    fn json_corrompu_refuse_et_valeur_invalide_ignoree() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("settings.json");
        fs::write(&p, "{\"dl.max_filesize_mb\": ").unwrap();
        assert!(matches!(
            SettingsStore::load(&p, &cfg()),
            Err(SettingsError::Corrupt(_))
        ));
        fs::write(
            &p,
            r#"{"dl.max_filesize_mb":"999999","sys.version":"9","x":"1","dl.quality":"720"}"#,
        )
        .unwrap();
        let s = SettingsStore::load(&p, &cfg()).unwrap();
        assert_eq!(s.get_u64("dl.max_filesize_mb"), Some(500));
        assert_eq!(
            s.get_choice("sys.version").as_deref(),
            Some(cfg().version.as_str())
        );
        assert_eq!(s.get_choice("dl.quality").as_deref(), Some("720"));
    }

    #[test]
    fn audit_ecrit_sans_secret() {
        let d = tempfile::tempdir().unwrap();
        let s = store(d.path());
        s.set("acc.rate_limit_per_min", "9", ADMIN).unwrap();
        s.set("providers.tiktok.enabled", "on", ADMIN).unwrap();
        let audit = fs::read_to_string(d.path().join(AUDIT_FILE)).unwrap();
        let lignes: Vec<serde_json::Value> = audit
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lignes.len(), 2);
        assert_eq!(lignes[0]["actor"], ADMIN);
        assert_eq!(lignes[0]["key"], "acc.rate_limit_per_min");
        assert_eq!(
            (lignes[0]["old"].as_str(), lignes[0]["new"].as_str()),
            (Some("10"), Some("9"))
        );
        assert!(lignes[1]["ts"].as_u64().unwrap() > 0);
        let fichier = fs::read_to_string(d.path().join("settings.json")).unwrap();
        for t in [&audit, &fichier] {
            assert!(!t.contains("SECRET-TOKEN") && !t.contains("secret-host"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn droits_0600() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let s = store(d.path());
        s.set("send.compress", "on", ADMIN).unwrap();
        for f in ["settings.json", AUDIT_FILE] {
            let m = fs::metadata(d.path().join(f)).unwrap().permissions().mode();
            assert_eq!(m & 0o777, 0o600, "{f}");
        }
    }

    #[test]
    fn abonnes_notifies_et_clone_partage() {
        let d = tempfile::tempdir().unwrap();
        let s = store(d.path());
        let mut rx = s.subscribe();
        let s2 = s.clone();
        s2.set("sto.cleanup_min", "30", ADMIN).unwrap();
        assert!(rx.has_changed().unwrap());
        let c = rx.borrow_and_update().clone().unwrap();
        assert_eq!((c.key, c.hot), ("sto.cleanup_min", false));
        assert_eq!(s.get_u64("sto.cleanup_min"), Some(30));
    }
}
