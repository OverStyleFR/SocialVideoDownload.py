//! Analyse et conversion média via ffprobe/ffmpeg.

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::MediaError;

/// Caractéristiques d'un fichier média.
#[derive(Debug, Clone, PartialEq)]
pub struct Probe {
    /// Durée en secondes.
    pub duration: f64,
    /// Présence d'une piste audio.
    pub has_audio: bool,
    /// Présence d'une piste vidéo.
    pub has_video: bool,
    /// Taille en octets.
    pub size: u64,
}

/// Analyse `path` avec ffprobe.
pub async fn probe(cfg: &Config, path: &Path) -> Result<Probe, MediaError> {
    let _ = (cfg, path);
    todo!("media::probe")
}

/// Recompresse `path` pour passer sous `limit_bytes` ; rend le chemin du fichier produit.
pub async fn compress_under(
    cfg: &Config,
    path: &Path,
    limit_bytes: u64,
) -> Result<PathBuf, MediaError> {
    let _ = (cfg, path, limit_bytes);
    todo!("media::compress_under")
}

/// Convertit `src` en MP3 dans `dst` de façon atomique (fichier temporaire puis renommage).
pub async fn to_mp3_atomic(cfg: &Config, src: &Path, dst: &Path) -> Result<PathBuf, MediaError> {
    let _ = (cfg, src, dst);
    todo!("media::to_mp3_atomic")
}
