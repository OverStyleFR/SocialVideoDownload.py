//! Téléchargement via yt-dlp.

use std::path::PathBuf;
use std::sync::Arc;

use url::Url;

use crate::config::Config;
use crate::error::DownloadError;
use crate::retention::JobDir;

/// Ce qu'on veut récupérer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    Video,
    Audio,
}

/// Résultat d'un téléchargement réussi.
#[derive(Debug, Clone)]
pub struct Downloaded {
    /// Fichier produit, dans le JobDir.
    pub path: PathBuf,
    /// Identifiant du média côté plateforme.
    pub id: String,
    /// Titre du média.
    pub title: String,
    /// Durée en secondes si connue.
    pub duration_s: Option<u64>,
    /// Taille en octets.
    pub size: u64,
}

/// Pilote yt-dlp selon la configuration.
#[derive(Debug, Clone)]
pub struct Downloader {
    /// Configuration partagée.
    pub config: Arc<Config>,
}

impl Downloader {
    /// Construit un Downloader.
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }

    /// Télécharge `url` dans `job` selon `mode`, en respectant taille et durée max.
    pub async fn fetch(
        &self,
        url: &Url,
        mode: Mode,
        job: &JobDir,
    ) -> Result<Downloaded, DownloadError> {
        let _ = (url, mode, job);
        todo!("Downloader::fetch")
    }

    /// Met à jour yt-dlp (yt-dlp -U ou équivalent).
    pub async fn update_ytdlp(&self) -> Result<(), DownloadError> {
        todo!("Downloader::update_ytdlp")
    }
}
