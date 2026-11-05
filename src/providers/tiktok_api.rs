//! TikTok via une API publique tierce de type tikwm (`/api/?url=…`).

use url::Url;

use super::source::{Http, MediaKind, ProviderError, Resolved};

pub const DEFAULT_BASE: &str = "https://www.tikwm.com/";

pub struct TikTokApi {
    http: Http,
    base: Url,
}

impl TikTokApi {
    pub fn new(http: Http, base: Url) -> Self {
        Self { http, base }
    }

    pub fn with_default_base(http: Http) -> Self {
        Self::new(
            http,
            Url::parse(DEFAULT_BASE).expect("base constante valide"),
        )
    }

    pub fn handles(&self, url: &Url) -> bool {
        url.host_str().is_some_and(super::tiktok::matches)
    }

    pub async fn resolve(&self, url: &Url) -> Result<Resolved, ProviderError> {
        let mut api = self
            .base
            .join("api/")
            .map_err(|_| ProviderError::NoMedia("base invalide".into()))?;
        api.query_pairs_mut()
            .append_pair("url", url.as_str())
            .append_pair("hd", "1");
        let v = self.http.get_json(&api).await?;
        if v["code"].as_i64() != Some(0) {
            // ex. « Free Api Limit: 1 request/second » : texte de l'API, tronqué.
            let msg = v["msg"].as_str().unwrap_or("erreur inconnue");
            return Err(ProviderError::NoMedia(msg.chars().take(120).collect()));
        }
        let d = &v["data"];
        let play = d["hdplay"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| d["play"].as_str())
            .ok_or_else(|| ProviderError::NoMedia("pas de vidéo (diaporama ?)".into()))?;
        Ok(Resolved {
            media_url: self.http.media_url(&self.base, play).await?,
            title: d["title"].as_str().unwrap_or_default().to_string(),
            duration_s: d["duration"].as_u64(),
            kind: MediaKind::Video,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::error::UrlError;
    use crate::providers::source::testsrv::{fx, serve, PUBLIC_MEDIA};

    const OK: &str = include_str!("../../tests/fixtures/providers/tiktok_ok.json");
    const LIMIT: &str = include_str!("../../tests/fixtures/providers/tiktok_limit.json");
    const LINK: &str = "https://www.tiktok.com/@scout2015/video/6718335390845095173";

    async fn provider(body: String, max: usize) -> TikTokApi {
        let http = Http::with_limits(Duration::from_millis(400), max, true, false);
        TikTokApi::new(http, serve(200, body, 0).await)
    }

    #[test]
    fn handles_tiktok_only() {
        let p = TikTokApi::with_default_base(Http::new(false));
        assert!(p.handles(&Url::parse(LINK).unwrap()));
        assert!(p.handles(&Url::parse("https://vm.tiktok.com/abc/").unwrap()));
        assert!(!p.handles(&Url::parse("https://x.com/a/status/1").unwrap()));
    }

    #[tokio::test]
    async fn nominal() {
        let p = provider(fx(OK, PUBLIC_MEDIA), 4096).await;
        let r = p.resolve(&Url::parse(LINK).unwrap()).await.unwrap();
        assert_eq!(r.media_url.as_str(), PUBLIC_MEDIA);
        assert_eq!(r.duration_s, Some(21));
    }

    #[tokio::test]
    async fn api_limit_is_error() {
        let p = provider(LIMIT.into(), 4096).await;
        let r = p.resolve(&Url::parse(LINK).unwrap()).await;
        assert!(matches!(r, Err(ProviderError::NoMedia(m)) if m.contains("Free Api Limit")));
    }

    #[tokio::test]
    async fn oversized_response_refused() {
        let big = fx(OK, &format!("https://93.184.215.14/{}", "a".repeat(10_000)));
        let p = provider(big, 4096).await;
        assert!(matches!(
            p.resolve(&Url::parse(LINK).unwrap()).await,
            Err(ProviderError::TooLarge(4096))
        ));
    }

    #[tokio::test]
    async fn private_media_url_refused() {
        let private = format!("http://{}/v.mp4", std::net::Ipv4Addr::new(192, 168, 1, 5));
        let p = provider(fx(OK, &private), 4096).await;
        assert!(matches!(
            p.resolve(&Url::parse(LINK).unwrap()).await,
            Err(ProviderError::Url(UrlError::Forbidden(_)))
        ));
    }

    #[tokio::test]
    async fn localhost_media_url_refused() {
        let p = provider(fx(OK, "http://localhost:9/v.mp4"), 4096).await;
        assert!(matches!(
            p.resolve(&Url::parse(LINK).unwrap()).await,
            Err(ProviderError::Url(UrlError::Forbidden(_)))
        ));
    }
}
