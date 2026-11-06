//! Instagram : `instagram.com`.

pub const REFERER: &str = "https://www.instagram.com/";

pub fn matches(host: &str) -> bool {
    super::host_is(host, "instagram.com")
}

/// Paramètres de suivi propres à Instagram.
pub fn is_tracking(name: &str) -> bool {
    matches!(name, "igsh" | "igshid" | "img_index")
}
