//! Validation anti-SSRF des URL fournies par les utilisateurs.

use std::net::IpAddr;

use url::{Host, Url};

use crate::error::UrlError;

/// Valide une URL : http(s) seulement, résolution DNS, refus des adresses privées sauf `allow_private`.
///
/// shortcut: la résolution suivie ensuite par yt-dlp peut différer de la nôtre (DNS rebinding) ;
/// limite connue, atténuée par le pare-feu du conteneur ; à durcir en épinglant l'IP si besoin.
pub async fn validate(url: &str, allow_private: bool) -> Result<Url, UrlError> {
    let parsed = Url::parse(url.trim()).map_err(|_| UrlError::Malformed)?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(UrlError::Scheme(parsed.scheme().to_string()));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(UrlError::Malformed);
    }
    let port = parsed.port_or_known_default().unwrap_or(80);
    let ips: Vec<IpAddr> = match parsed.host().ok_or(UrlError::NoHost)? {
        // Url::parse normalise déjà les formes décimales/octales/hexa en IPv4.
        Host::Ipv4(ip) => vec![IpAddr::V4(ip)],
        Host::Ipv6(ip) => vec![IpAddr::V6(ip)],
        Host::Domain(d) => {
            if d == "localhost" || d.ends_with(".localhost") {
                vec![IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)]
            } else {
                tokio::net::lookup_host((d, port))
                    .await
                    .map_err(|e| UrlError::Dns(e.to_string()))?
                    .map(|s| s.ip())
                    .collect()
            }
        }
    };
    if ips.is_empty() {
        return Err(UrlError::Dns("aucune adresse".into()));
    }
    if !allow_private {
        if let Some(bad) = ips.into_iter().find(|ip| !is_public_ip(*ip)) {
            return Err(UrlError::Forbidden(bad));
        }
    }
    Ok(parsed)
}

/// Vrai si l'adresse est publique (pas privée, loopback, link-local, CGNAT, ULA, multicast, non spécifiée).
pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            !(v.is_loopback()
                || v.is_private()
                || v.is_link_local()
                || v.is_unspecified()
                || v.is_broadcast()
                || v.is_multicast()
                || o[0] == 0
                || (o[0] == 100 && (o[1] & 0xc0) == 64))
        }
        IpAddr::V6(v) => match v.to_ipv4_mapped() {
            Some(m) => is_public_ip(IpAddr::V4(m)),
            None => {
                let s = v.segments();
                !(v.is_loopback()
                    || v.is_unspecified()
                    || v.is_multicast()
                    || (s[0] & 0xffc0) == 0xfe80
                    || (s[0] & 0xfe00) == 0xfc00)
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn refuses_internal_targets() {
        let bad = [
            "http://127.0.0.1/",
            "http://localhost/",
            "http://[::1]/",
            "http://10.1.2.3/",
            "http://172.31.0.5/",
            "http://3232235777/", // plage 192.168/16, forme décimale
            "http://169.254.169.254/latest/meta-data",
            "http://100.64.0.1/",
            "http://[fd00::1]/",
            "http://[::ffff:127.0.0.1]/",
            "http://2130706433/",
            "http://0177.0.0.1/",
            "http://0x7f.1/",
            "http://0.0.0.0/",
            "http://[fe80::1]/",
            "http://224.0.0.1/",
            "http://255.255.255.255/",
            "http://foo.localhost/",
        ];
        for u in bad {
            assert!(
                matches!(validate(u, false).await, Err(UrlError::Forbidden(_))),
                "{u} devrait être refusée"
            );
        }
    }

    #[tokio::test]
    async fn refuses_bad_scheme_and_credentials() {
        assert!(matches!(
            validate("file:///etc/passwd", false).await,
            Err(UrlError::Scheme(_))
        ));
        assert!(matches!(
            validate("ftp://1.1.1.1/", false).await,
            Err(UrlError::Scheme(_))
        ));
        assert!(matches!(
            validate("http://user:pw@1.1.1.1/", false).await,
            Err(UrlError::Malformed)
        ));
        assert!(matches!(
            validate("pas une url", false).await,
            Err(UrlError::Malformed)
        ));
    }

    #[tokio::test]
    async fn accepts_public_literal_and_allow_private() {
        let u = validate("https://1.1.1.1:8443/v?a=1", false).await.unwrap();
        assert_eq!(u.host_str(), Some("1.1.1.1"));
        assert!(validate("http://93.184.215.14/", false).await.is_ok());
        assert!(validate("http://127.0.0.1/", true).await.is_ok());
    }
}
