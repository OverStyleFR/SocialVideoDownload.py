//! Répertoires de travail par tâche (RAII) et nettoyage sans toucher aux tâches actives.

use std::collections::HashSet;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use crate::config::Config;

/// Nom du fichier verrou présent tant qu'un JobDir est vivant.
pub const LOCK_FILE: &str = ".lock";

/// Quota de taille du dossier de téléchargements, base de l'estimation d'espace libre.
// shortcut: la std n'expose pas l'espace disque libre (fs2/sysinfo absents) ; on estime
// « libre = quota - taille du dossier », à remplacer par un vrai statvfs si une dépendance est admise.
pub const MAX_DOWNLOAD_DIR_MB: u64 = 10_240;

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

/// Supprime les JobDir plus vieux que `retention` (ou les plus anciens si l'espace libre < `min_free_mb`), jamais un verrouillé.
pub async fn cleanup(
    dir: &Path,
    retention: Duration,
    min_free_mb: u64,
) -> std::io::Result<CleanupReport> {
    let dir = dir.to_owned();
    tokio::task::spawn_blocking(move || cleanup_sync(&dir, retention, min_free_mb))
        .await
        .map_err(std::io::Error::other)?
}

fn cleanup_sync(
    dir: &Path,
    retention: Duration,
    min_free_mb: u64,
) -> std::io::Result<CleanupReport> {
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
    let mut free_mb = MAX_DOWNLOAD_DIR_MB.saturating_sub(total >> 20);
    for (mtime, path, size) in candidates {
        let expired = now.duration_since(mtime).unwrap_or_default() >= retention;
        if !expired && free_mb >= min_free_mb {
            continue;
        }
        // Revérifie juste avant de supprimer : un JobDir a pu être créé entre-temps.
        if path.canonicalize().map_or(true, |c| is_locked(&c)) {
            report.skipped_locked += 1;
            continue;
        }
        if std::fs::remove_dir_all(&path).is_ok() {
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

/// Lance le nettoyage périodique ; la tâche vit jusqu'à l'arrêt du runtime.
/// Avec `settings` : intervalle `sto.cleanup_min` lu au lancement (effet au redémarrage),
/// rétention et espace libre relus à chaque passage (à chaud). Sans : 10 min et la configuration.
pub fn spawn_janitor(
    cfg: &Config,
    settings: Option<crate::settings::SettingsStore>,
) -> tokio::task::JoinHandle<()> {
    let (dir, cfg_retention, cfg_min_free) = (
        cfg.download_dir.clone(),
        cfg.retention,
        cfg.min_free_disk_mb,
    );
    let every_min = settings
        .as_ref()
        .and_then(|s| s.get_u64("sto.cleanup_min"))
        .unwrap_or(10);
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(every_min.max(1) * 60));
        loop {
            tick.tick().await;
            let s = settings.as_ref();
            // shortcut: cleanup() ne distingue pas petits et gros fichiers, on garde la plus longue
            // des deux rétentions (jamais de suppression avant l'heure promise).
            let retention = s
                .and_then(|s| {
                    let small = s.get_u64("sto.retention_small_h")?;
                    Some(small.max(s.get_u64("sto.retention_large_h")?))
                })
                .map_or(cfg_retention, |h| Duration::from_secs(h * 3600));
            let min_free = s
                .and_then(|s| s.get_u64("sto.min_free_mb"))
                .unwrap_or(cfg_min_free);
            match cleanup(&dir, retention, min_free).await {
                Ok(r) if r.removed > 0 => {
                    tracing::info!(removed = r.removed, freed = r.freed_bytes, "nettoyage")
                }
                Ok(_) => {}
                Err(e) => tracing::warn!(error = %e, "nettoyage échoué"),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
