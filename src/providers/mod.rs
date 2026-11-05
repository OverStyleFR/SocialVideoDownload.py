//! Détection de plateforme et arguments yt-dlp spécifiques.

use url::Url;

/// Plateforme reconnue d'après l'hôte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    TikTok,
    Instagram,
    Twitter,
    YouTube,
    Generic,
}

/// Détecte la plateforme d'une URL.
pub fn detect(url: &Url) -> Platform {
    let _ = url;
    todo!("providers::detect")
}

/// Arguments yt-dlp propres à la plateforme (User-Agent, Referer…).
pub fn ytdlp_args(platform: Platform) -> Vec<String> {
    let _ = platform;
    todo!("providers::ytdlp_args")
}

/// Extrait les URL http(s) d'un texte libre, dans l'ordre, sans doublon.
pub fn extract_urls(text: &str) -> Vec<String> {
    let _ = text;
    todo!("providers::extract_urls")
}
