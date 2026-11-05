//! X / Twitter via une API publique de type fxtwitter (`/{user}/status/{id}`).

use url::Url;

use super::source::{Http, MediaKind, ProviderError, Resolved};

pub const DEFAULT_BASE: &str = "https://api.fxtwitter.com/";

pub struct XApi {
    http: Http,
    base: Url,
}

impl XApi {
    pub fn new(http: Http, base: Url) -> Self {
        Self { http, base }
    }

    pub fn with_default_base(http: Http) -> Self {
        Self::new(
            http,
            Url::parse(DEFAULT_BASE).expect("base constante valide"),
        )
    }

    /// `(utilisateur, identifiant)` d'un lien `…/{user}/status/{id}`.
    fn parse(url: &Url) -> Option<(String, String)> {
        if !super::twitter::matches(url.host_str()?) {
            return None;
        }
        let mut s = url.path_segments()?;
        let user = s.next()?;
        let id = match (s.next()?, s.next()?) {
            ("status", id) if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) => id,
            _ => return None,
        };
        let user_ok =
            !user.is_empty() && user.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
        user_ok.then(|| (user.to_string(), id.to_string()))
    }

    pub fn handles(&self, url: &Url) -> bool {
        Self::parse(url).is_some()
    }

    pub async fn resolve(&self, url: &Url) -> Result<Resolved, ProviderError> {
        let (user, id) =
            Self::parse(url).ok_or_else(|| ProviderError::NoMedia("lien X non reconnu".into()))?;
        let mut api = self.base.clone();
        api.path_segments_mut()
            .map_err(|_| ProviderError::NoMedia("base invalide".into()))?
            .pop_if_empty()
            .extend([user.as_str(), "status", id.as_str()]);
        let v = self.http.get_json(&api).await?;
        let tweet = &v["tweet"];
        let video = &tweet["media"]["videos"][0];
        let src = video["url"]
            .as_str()
            .ok_or_else(|| ProviderError::NoMedia("pas de vidéo dans ce tweet".into()))?;
        Ok(Resolved {
            media_url: self.http.media_url(&self.base, src).await?,
            title: tweet["text"].as_str().unwrap_or_default().to_string(),
            duration_s: video["duration"].as_f64().map(|d| d.round() as u64),
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

    const OK: &str = include_str!("../../tests/fixtures/providers/x_tweet.json");
    const NO_VIDEO: &str = include_str!("../../tests/fixtures/providers/x_no_video.json");
    const LINK: &str = "https://x.com/freethenipple/status/643211948184596480?s=20";

    fn http() -> Http {
        Http::with_limits(Duration::from_millis(400), 4096, true, false)
    }

    async fn provider(status: u16, body: String, delay: u64) -> XApi {
        XApi::new(http(), serve(status, body, delay).await)
    }

    #[test]
    fn handles_only_status_links() {
        let p = XApi::with_default_base(http());
        assert!(p.handles(&Url::parse(LINK).unwrap()));
        assert!(!p.handles(&Url::parse("https://x.com/home").unwrap()));
        assert!(!p.handles(&Url::parse("https://x.com/a/status/12ab").unwrap()));
        assert!(!p.handles(&Url::parse("https://tiktok.com/a/status/12").unwrap()));
    }

    #[tokio::test]
    async fn nominal() {
        let p = provider(200, fx(OK, PUBLIC_MEDIA), 0).await;
        let r = p.resolve(&Url::parse(LINK).unwrap()).await.unwrap();
        assert_eq!(r.media_url.as_str(), PUBLIC_MEDIA);
        assert_eq!(r.title, "Une video de test");
        assert_eq!(r.duration_s, Some(12));
    }

    #[tokio::test]
    async fn no_video_is_error() {
        let p = provider(200, NO_VIDEO.into(), 0).await;
        assert!(matches!(
            p.resolve(&Url::parse(LINK).unwrap()).await,
            Err(ProviderError::NoMedia(_))
        ));
    }

    #[tokio::test]
    async fn timeout() {
        let p = provider(200, fx(OK, PUBLIC_MEDIA), 1500).await;
        assert!(matches!(
            p.resolve(&Url::parse(LINK).unwrap()).await,
            Err(ProviderError::Timeout)
        ));
    }

    #[tokio::test]
    async fn private_media_url_refused() {
        // IP privée construite à l'exécution, jamais littérale dans le source.
        let private = format!("http://{}/v.mp4", std::net::Ipv4Addr::new(10, 0, 0, 1));
        let p = provider(200, fx(OK, &private), 0).await;
        assert!(matches!(
            p.resolve(&Url::parse(LINK).unwrap()).await,
            Err(ProviderError::Url(UrlError::Forbidden(_)))
        ));
    }
}
