//! Répertoires de travail par tâche (RAII) et nettoyage sans toucher aux tâches actives.

use std::collections::HashSet;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use crate::config::Config;

/// Nom du fichier verrou présent tant qu'un JobDir est vivant.
pub const LOCK_FILE: &str = ".lock";

/// Quota du dossier de téléchargements : l'espace libre retenu est le plus petit entre le disque
/// réel (statvfs, Unix) et « quota - taille du dossier ».
pub const MAX_DOWNLOAD_DIR_MB: u64 = 10_240;

/// Espace libre retenu (Mo) pour un dossier de `dir_total` octets.
fn free_space_mb(dir: &Path, dir_total: u64) -> u64 {
    let quota = MAX_DOWNLOAD_DIR_MB.saturating_sub(dir_total >> 20);
    crate::system::free_bytes(dir).map_or(quota, |b| (b >> 20).min(quota))
}

type Registry = Arc<Mutex<HashSet<PathBuf>>>;

/// Registre en mémoire des JobDir vivants (chemins canoniques).
fn registry() -> &'static Registry {
    static R: OnceLock<Registry> = OnceLock::new();
    R.get_or_init(Registry::default)
}

fn is_locked(canon: &Path) -> bool {
    registry().lock().map_or(true, |r| r.contains(canon))
}

/// Répertoire de travail d'une tâche, verrouillé jusqu'à sa destruction.
#[derive(Debug)]
pub struct JobDir {
    path: PathBuf,
    id: String,
    canon: PathBuf,
    lock: Option<File>,
    keep: bool,
}

impl JobDir {
    /// Crée `root/<id>` avec son verrou ; `id` doit être un identifiant sûr (alphanumérique, -, _).
    pub fn create(root: &Path, id: &str) -> std::io::Result<Self> {
        let invalid = |m: &str| std::io::Error::new(std::io::ErrorKind::InvalidInput, m.to_owned());
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(invalid("identifiant de tâche invalide"));
        }
        std::fs::create_dir_all(root)?;
        let path = root.join(id);
        std::fs::create_dir(&path)?; // échoue si l'id existe déjà : unicité
        let lock = File::create(path.join(LOCK_FILE))?;
        let canon = path.canonicalize()?;
        registry()
            .lock()
            .map_err(|_| invalid("registre empoisonné"))?
            .insert(canon.clone());
        Ok(Self {
            path,
            id: id.to_owned(),
            canon,
            lock: Some(lock),
            keep: false,
        })
    }

    /// Chemin du répertoire.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Identifiant de la tâche.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Conserve le contenu à la destruction (cache) ; `cleanup` l'effacera après la rétention.
    pub fn keep(&mut self) {
        self.keep = true;
    }
}

impl Drop for JobDir {
    /// Libère le verrou et supprime le répertoire, sauf si `keep()` a été appelé.
    fn drop(&mut self) {
        if let Ok(mut r) = registry().lock() {
            r.remove(&self.canon);
        }
        drop(self.lock.take());
        let _ = std::fs::remove_file(self.path.join(LOCK_FILE));
        if !self.keep {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

/// Bilan d'un passage de nettoyage.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CleanupReport {
    /// Répertoires supprimés.
    pub removed: usize,
    /// Répertoires ignorés car verrouillés.
    pub skipped_locked: usize,
    /// Octets libérés.
    pub freed_bytes: u64,
}

fn dir_size(p: &Path) -> u64 {
    std::fs::read_dir(p).map_or(0, |rd| {
        rd.flatten()
            .map(|e| match e.file_type() {
                Ok(t) if t.is_dir() => dir_size(&e.path()),
                Ok(t) if t.is_file() => e.metadata().map_or(0, |m| m.len()),
                _ => 0,
            })
            .sum()
    })
}

/// Rétention à deux niveaux, comme le bot Python : un répertoire contenant un mp3 ou pesant moins
/// de `small_bytes` est « petit » (`small`), sinon « gros » (`large`).
#[derive(Debug, Clone, Copy)]
pub struct Tiers {
    pub small: Duration,
    pub large: Duration,
    pub small_bytes: u64,
}

impl Tiers {
    fn for_dir(&self, path: &Path, size: u64) -> Duration {
        if size < self.small_bytes || has_mp3(path) {
            self.small
        } else {
            self.large
        }
    }
}

fn has_mp3(p: &Path) -> bool {
    std::fs::read_dir(p).is_ok_and(|rd| {
        rd.flatten().any(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("mp3"))
        })
    })
}

/// Supprime les JobDir plus vieux que `retention` (ou les plus anciens si l'espace libre < `min_free_mb`), jamais un verrouillé.
pub async fn cleanup(
    dir: &Path,
    retention: Duration,
    min_free_mb: u64,
) -> std::io::Result<CleanupReport> {
    let tiers = Tiers {
        small: retention,
        large: retention,
        small_bytes: 0,
    };
    cleanup_tiered(dir, tiers, min_free_mb).await
}

/// Comme [`cleanup`], avec une rétention selon le type et la taille du répertoire.
pub async fn cleanup_tiered(
    dir: &Path,
    tiers: Tiers,
    min_free_mb: u64,
) -> std::io::Result<CleanupReport> {
    let dir = dir.to_owned();
    tokio::task::spawn_blocking(move || cleanup_sync(&dir, tiers, min_free_mb))
        .await
        .map_err(std::io::Error::other)?
}

/// Comme `check_and_clean_if_needed()` du bot Python, avant chaque téléchargement : sous le seuil
/// d'espace libre, supprime les expirés puis les plus anciens, jamais un téléchargement en cours
/// (l'original vidait tout le dossier, envois en cours compris).
pub async fn check_and_clean_if_needed(
    cfg: &Config,
    settings: Option<&crate::settings::SettingsStore>,
) {
    let min_free = settings
        .and_then(|s| s.get_u64("sto.min_free_mb"))
        .unwrap_or(cfg.min_free_disk_mb);
    let free = free_mb_now(&cfg.download_dir).await;
    tracing::debug!("[DISK_MANAGER] Espace libre : {free} Mo (seuil : {min_free} Mo)");
    if free >= min_free {
        return;
    }
    tracing::warn!(
        "[DISK_MANAGER] Espace libre insuffisant ({free} Mo < {min_free} Mo). Nettoyage par rétention..."
    );
    match cleanup_tiered(&cfg.download_dir, current_tiers(cfg, settings), min_free).await {
        Ok(r) => log_report(&r),
        Err(e) => tracing::warn!(error = %e, "nettoyage échoué"),
    }
    let after = free_mb_now(&cfg.download_dir).await;
    if after < min_free {
        tracing::warn!(
            "[DISK_MANAGER] Toujours insuffisant après nettoyage ({after} Mo) : il ne reste que les téléchargements en cours."
        );
    }
    tracing::info!("[DISK_MANAGER] Nettoyage d'urgence terminé.");
}

async fn free_mb_now(dir: &Path) -> u64 {
    let dir = dir.to_owned();
    tokio::task::spawn_blocking(move || free_space_mb(&dir, dir_size(&dir)))
        .await
        .unwrap_or(u64::MAX)
}

/// Journaux de cleanup_by_retention() du bot Python.
fn log_report(r: &CleanupReport) {
    if r.removed > 0 {
        tracing::info!(
            "[DISK_MANAGER] Nettoyage par rétention terminé — {} téléchargement(s) supprimé(s), {} Mo libérés.",
            r.removed,
            r.freed_bytes >> 20
        );
    } else {
        tracing::debug!("[DISK_MANAGER] Aucun fichier expiré trouvé.");
    }
}

fn cleanup_sync(dir: &Path, tiers: Tiers, min_free_mb: u64) -> std::io::Result<CleanupReport> {
    let mut report = CleanupReport::default();
    let now = SystemTime::now();
    // (mtime, chemin, taille) des répertoires non verrouillés ; les liens symboliques et fichiers sont ignorés.
    let mut candidates = Vec::new();
    for e in std::fs::read_dir(dir)?.flatten() {
        if !e.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let path = e.path();
        if path.canonicalize().map_or(true, |c| is_locked(&c)) {
            report.skipped_locked += 1;
            continue;
        }
        let mtime = e.metadata().and_then(|m| m.modified()).unwrap_or(now);
        candidates.push((mtime, path.clone(), dir_size(&path)));
    }
    candidates.sort_by_key(|c| c.0); // plus anciens d'abord

    let total: u64 = candidates.iter().map(|c| c.2).sum::<u64>() + locked_size(dir);
    let mut free_mb = free_space_mb(dir, total);
    for (mtime, path, size) in candidates {
        let expired = now.duration_since(mtime).unwrap_or_default() >= tiers.for_dir(&path, size);
        if !expired && free_mb >= min_free_mb {
            continue;
        }
        // Revérifie juste avant de supprimer : un JobDir a pu être créé entre-temps.
        if path.canonicalize().map_or(true, |c| is_locked(&c)) {
            report.skipped_locked += 1;
            continue;
        }
        if std::fs::remove_dir_all(&path).is_ok() {
            if expired {
                tracing::info!(
                    "[DISK_MANAGER] Fichier expiré supprimé : {}",
                    path.display()
                );
            } else {
                tracing::warn!(
                    "[DISK_MANAGER] Supprimé pour libérer de l'espace : {}",
                    path.display()
                );
            }
            report.removed += 1;
            report.freed_bytes += size;
            free_mb += size >> 20;
        }
    }
    Ok(report)
}

fn locked_size(dir: &Path) -> u64 {
    std::fs::read_dir(dir).map_or(0, |rd| {
        rd.flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter(|e| e.path().canonicalize().is_ok_and(|c| is_locked(&c)))
            .map(|e| dir_size(&e.path()))
            .sum()
    })
}

/// Vide le dossier de téléchargements au démarrage (comme `clear_downloads()` du bot Python) ;
/// rend le nombre d'entrées supprimées. Le dossier lui-même est conservé.
pub fn clear_downloads(dir: &Path) -> usize {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return 0;
    };
    rd.flatten()
        .filter(|e| {
            let p = e.path();
            if p.is_dir() && !p.is_symlink() {
                std::fs::remove_dir_all(&p).is_ok()
            } else {
                std::fs::remove_file(&p).is_ok()
            }
        })
        .count()
}

/// Rétentions en vigueur : /settings (à chaud) sinon la configuration.
pub fn current_tiers(cfg: &Config, settings: Option<&crate::settings::SettingsStore>) -> Tiers {
    let hours = |k: &str, def: Duration| {
        settings
            .and_then(|s| s.get_u64(k))
            .map_or(def, |h| Duration::from_secs(h * 3600))
    };
    Tiers {
        small: hours("sto.retention_small_h", cfg.retention_small),
        large: hours("sto.retention_large_h", cfg.retention_large),
        small_bytes: cfg.small_file_size_mb << 20,
    }
}

/// Chiffres de rétention pour /stats (équivalent de `retention_stats()` du bot Python).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RetentionStats {
    pub files: u64,
    pub bytes: u64,
    /// Prochain à expirer : (nom du fichier, secondes restantes, 0 si déjà périmé).
    pub next: Option<(String, u64)>,
    /// Libérable d'ici 2 h : (fichiers, octets) ; idem 24 h.
    pub freed_2h: (u64, u64),
    pub freed_24h: (u64, u64),
}

/// Parcourt les répertoires de travail de `dir` : un répertoire = un téléchargement, expirant à
/// `mtime + rétention` selon `tiers`.
pub fn stats(dir: &Path, tiers: Tiers) -> RetentionStats {
    let now = SystemTime::now();
    let mut out = RetentionStats::default();
    let mut next: Option<(u64, String)> = None;
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if !e.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let path = e.path();
        let size = dir_size(&path);
        let age = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|m| now.duration_since(m).ok())
            .unwrap_or_default();
        let left = tiers.for_dir(&path, size).saturating_sub(age).as_secs();
        let name = std::fs::read_dir(&path)
            .into_iter()
            .flatten()
            .flatten()
            .map(|f| f.file_name().to_string_lossy().into_owned())
            .find(|n| n != LOCK_FILE)
            .unwrap_or_else(|| e.file_name().to_string_lossy().into_owned());
        out.files += 1;
        out.bytes += size;
        if left <= 7200 {
            out.freed_2h = (out.freed_2h.0 + 1, out.freed_2h.1 + size);
        }
        if left <= 86_400 {
            out.freed_24h = (out.freed_24h.0 + 1, out.freed_24h.1 + size);
        }
        if next.as_ref().is_none_or(|n| left < n.0) {
            next = Some((left, name));
        }
    }
    out.next = next.map(|(left, name)| (name, left));
    out
}

/// Lance le nettoyage périodique ; la tâche vit jusqu'à l'arrêt du runtime.
/// Intervalle : `sto.cleanup_min` (/settings, lu au lancement) sinon CLEANUP_INTERVAL_HOURS.
/// Rétentions petits/gros, seuil et espace libre relus à chaque passage (à chaud).
pub fn spawn_janitor(
    cfg: &Config,
    settings: Option<crate::settings::SettingsStore>,
) -> tokio::task::JoinHandle<()> {
    let cfg = cfg.clone();
    let every_s = settings
        .as_ref()
        .and_then(|s| s.get_u64("sto.cleanup_min"))
        .map_or(cfg.cleanup_interval.as_secs(), |m| m * 60)
        .max(60);
    // Journaux de scheduled_cleanup() du bot Python.
    let every = if every_s.is_multiple_of(3600) {
        format!("{}h", every_s / 3600)
    } else {
        format!("{} min", every_s / 60)
    };
    tracing::info!("[CLEANUP] Rotation planifiée activée — nettoyage toutes les {every}.");
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(every_s));
        loop {
            tick.tick().await;
            tracing::info!("[CLEANUP] Nettoyage périodique (rétention)...");
            let tiers = current_tiers(&cfg, settings.as_ref());
            let min_free = settings
                .as_ref()
                .and_then(|s| s.get_u64("sto.min_free_mb"))
                .unwrap_or(cfg.min_free_disk_mb);
            match cleanup_tiered(&cfg.download_dir, tiers, min_free).await {
                Ok(r) => log_report(&r),
                Err(e) => tracing::warn!(error = %e, "nettoyage échoué"),
            }
            tracing::info!("[CLEANUP] Nettoyage périodique terminé.");
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn nettoyage_d_urgence_sous_le_seuil() {
        let root = tempfile::tempdir().unwrap();
        let env: std::collections::HashMap<String, String> = [
            ("BOT_TOKEN", "x"),
            ("DOWNLOAD_DIR", root.path().to_str().unwrap()),
            ("MIN_FREE_DISK_MB", "99999999"),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect();
        let cfg = Config::from_map(&env).unwrap();
        let old = root.path().join("ancien");
        std::fs::create_dir(&old).unwrap();
        check_and_clean_if_needed(&cfg, None).await;
        assert!(!old.exists(), "sous le seuil, le plus ancien part");
    }

    #[test]
    fn stats_prochain_et_liberable_comme_le_bot_python() {
        let root = tempfile::tempdir().unwrap();
        let mk = |n: &str, size: usize| {
            let d = root.path().join(n);
            std::fs::create_dir(&d).unwrap();
            std::fs::write(d.join(format!("{n}.mp4")), vec![0u8; size]).unwrap();
        };
        mk("petit", 100); // < seuil : rétention « petit » 24 h
        mk("gros", 3000); // ≥ seuil : rétention « gros » 2 h
        let tiers = Tiers {
            small: Duration::from_secs(86_400),
            large: Duration::from_secs(7_200),
            small_bytes: 1000,
        };
        let s = stats(root.path(), tiers);
        assert_eq!((s.files, s.bytes), (2, 3100));
        let (name, left) = s.next.unwrap();
        assert_eq!(name, "gros.mp4");
        assert!((7190..=7200).contains(&left), "{left}");
        assert_eq!(s.freed_2h, (1, 3000));
        assert_eq!(s.freed_24h, (2, 3100));
        assert_eq!(
            stats(&root.path().join("absent"), tiers),
            RetentionStats::default()
        );
    }

    #[tokio::test]
    async fn retention_jobdir_vivant_survit() {
        let root = tempfile::tempdir().unwrap();
        let job = JobDir::create(root.path(), "job-live").unwrap();
        std::fs::write(job.path().join("a.mp4"), b"x").unwrap();
        let r = cleanup(root.path(), Duration::ZERO, u64::MAX)
            .await
            .unwrap();
        assert!(job.path().join("a.mp4").exists());
        assert_eq!((r.removed, r.skipped_locked), (0, 1));
    }

    #[tokio::test]
    async fn retention_vieux_dossier_non_verrouille_supprime() {
        let root = tempfile::tempdir().unwrap();
        let stale = root.path().join("vieux"); // verrou orphelin d'un crash
        std::fs::create_dir(&stale).unwrap();
        std::fs::write(stale.join("f"), vec![0u8; 10]).unwrap();
        std::fs::write(stale.join(LOCK_FILE), b"").unwrap();
        let fresh = root.path().join("recent");
        std::fs::create_dir(&fresh).unwrap();
        // rétention longue : rien n'est assez vieux
        let r0 = cleanup(root.path(), Duration::from_secs(3600), 0)
            .await
            .unwrap();
        assert_eq!(r0.removed, 0);
        std::thread::sleep(Duration::from_millis(20));
        // rétention nulle : tout non verrouillé est périmé
        let r = cleanup(root.path(), Duration::ZERO, 0).await.unwrap();
        assert!(!stale.exists() && !fresh.exists());
        assert_eq!((r.removed, r.freed_bytes), (2, 10));
    }

    #[tokio::test]
    async fn retention_manque_de_place_supprime_les_plus_anciens_pas_le_verrouille() {
        let root = tempfile::tempdir().unwrap();
        let job = JobDir::create(root.path(), "actif").unwrap();
        let a = root.path().join("a");
        std::fs::create_dir(&a).unwrap();
        // seuil impossible à satisfaire : tout ce qui est libérable part, le vivant reste
        let r = cleanup(root.path(), Duration::from_secs(86400), u64::MAX)
            .await
            .unwrap();
        assert!(!a.exists() && job.path().exists());
        assert_eq!(r.removed, 1);
    }

    #[test]
    fn retention_drop_nettoie_et_keep_conserve() {
        let root = tempfile::tempdir().unwrap();
        let j = JobDir::create(root.path(), "j1").unwrap();
        let p = j.path().to_owned();
        std::fs::write(p.join("f"), b"x").unwrap();
        drop(j);
        assert!(!p.exists());
        let mut k = JobDir::create(root.path(), "j2").unwrap();
        k.keep();
        let kp = k.path().to_owned();
        std::fs::write(kp.join("f"), b"x").unwrap();
        drop(k);
        assert!(kp.join("f").exists() && !kp.join(LOCK_FILE).exists());
    }

    #[test]
    fn retention_id_invalide_ou_doublon_refuse() {
        let root = tempfile::tempdir().unwrap();
        assert!(JobDir::create(root.path(), "../x").is_err());
        let _a = JobDir::create(root.path(), "dup").unwrap();
        assert!(JobDir::create(root.path(), "dup").is_err());
    }

    #[test]
    fn clear_downloads_vide_le_dossier_et_le_garde() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.mp4"), b"x").unwrap();
        std::fs::create_dir(d.path().join("job")).unwrap();
        std::fs::write(d.path().join("job/b.mp3"), b"y").unwrap();
        assert_eq!(clear_downloads(d.path()), 2);
        assert!(d.path().exists() && std::fs::read_dir(d.path()).unwrap().count() == 0);
    }
}
