//! Twitter / X : `twitter.com`, `x.com`.

pub const REFERER: &str = "https://x.com/";

pub fn matches(host: &str) -> bool {
    super::host_is(host, "twitter.com") || super::host_is(host, "x.com")
}

/// Paramètres de suivi propres à Twitter/X.
pub fn is_tracking(name: &str) -> bool {
    matches!(name, "s" | "t" | "ref_src" | "ref_url")
}
