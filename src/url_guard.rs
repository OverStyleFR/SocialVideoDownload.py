//! Validation anti-SSRF des URL fournies par les utilisateurs.

use std::net::IpAddr;

use url::Url;

use crate::error::UrlError;

/// Valide une URL : http(s) seulement, résolution DNS, refus des adresses privées sauf `allow_private`.
pub async fn validate(url: &str, allow_private: bool) -> Result<Url, UrlError> {
    let _ = (url, allow_private);
    todo!("url_guard::validate")
}

/// Vrai si l'adresse est publique (pas privée, loopback, link-local, CGNAT, ULA, multicast, non spécifiée).
pub fn is_public_ip(ip: IpAddr) -> bool {
    let _ = ip;
    todo!("url_guard::is_public_ip")
}
