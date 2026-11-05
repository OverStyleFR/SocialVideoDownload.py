//! Répertoires de travail par tâche (RAII) et nettoyage sans toucher aux tâches actives.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// Nom du fichier verrou présent tant qu'un JobDir est vivant.
pub const LOCK_FILE: &str = ".lock";

/// Répertoire de travail d'une tâche, verrouillé jusqu'à sa destruction.
#[derive(Debug)]
pub struct JobDir {
    path: PathBuf,
    id: String,
}

impl JobDir {
    /// Crée `root/<id>` avec son verrou ; `id` doit être un identifiant sûr (alphanumérique, -, _).
    pub fn create(root: &Path, id: &str) -> std::io::Result<Self> {
        let _ = (root, id);
        todo!("retention::JobDir::create")
    }

    /// Chemin du répertoire.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Identifiant de la tâche.
    pub fn id(&self) -> &str {
        &self.id
    }
}

impl Drop for JobDir {
    /// Libère le verrou (le contenu reste pour le cache jusqu'au nettoyage).
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.path.join(LOCK_FILE));
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

/// Supprime les JobDir plus vieux que `retention` (ou les plus anciens si l'espace libre < `min_free_mb`), jamais un verrouillé.
pub async fn cleanup(
    dir: &Path,
    retention: Duration,
    min_free_mb: u64,
) -> std::io::Result<CleanupReport> {
    let _ = (dir, retention, min_free_mb);
    todo!("retention::cleanup")
}
