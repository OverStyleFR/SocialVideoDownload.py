//! TikTok : `tiktok.com` et `vm.tiktok.com` (liens courts).

/// Referer envoyé à yt-dlp (repris de l'ancienne configuration Python).
pub const REFERER: &str = "https://www.tiktok.com/";

pub fn matches(host: &str) -> bool {
    super::host_is(host, "tiktok.com")
}

/// Paramètres de suivi propres à TikTok.
pub fn is_tracking(name: &str) -> bool {
    matches!(
        name,
        "_r" | "_t" | "is_from_webapp" | "sender_device" | "sender_web_id" | "refer" | "checksum"
    )
}
