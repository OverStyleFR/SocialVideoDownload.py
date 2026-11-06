//! YouTube : `youtube.com`, `youtu.be`.

pub const REFERER: &str = "https://www.youtube.com/";

pub fn matches(host: &str) -> bool {
    super::host_is(host, "youtube.com") || super::host_is(host, "youtu.be")
}

/// Paramètres de suivi propres à YouTube (`v`, `list`, `t` sont conservés).
pub fn is_tracking(name: &str) -> bool {
    matches!(name, "si" | "feature" | "pp")
}
