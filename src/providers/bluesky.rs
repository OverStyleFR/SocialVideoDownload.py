//! Bluesky via l'API publique (`app.bsky.feed.getPostThread`) ; la vidéo est une playlist HLS.

use url::Url;

use super::source::{Http, MediaKind, ProviderError, Resolved};

pub const DEFAULT_BASE: &str = "https://public.api.bsky.app/";

pub struct Bluesky {
    http: Http,
    base: Url,
}

impl Bluesky {
    pub fn new(http: Http, base: Url) -> Self {
        Self { http, base }
    }

    pub fn with_default_base(http: Http) -> Self {
        Self::new(
            http,
            Url::parse(DEFAULT_BASE).expect("base constante valide"),
        )
    }

    /// `(identifiant du compte, rkey)` d'un lien `bsky.app/profile/{h}/post/{rkey}`.
    fn parse(url: &Url) -> Option<(String, String)> {
        if !super::host_is(url.host_str()?, "bsky.app") {
            return None;
        }
        let s: Vec<&str> = url.path_segments()?.collect();
        let ok = |t: &str, colon: bool| {
            !t.is_empty()
                && t.bytes().all(|b| {
                    b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || (colon && b == b':')
                })
        };
        match s.as_slice() {
            ["profile", actor, "post", rkey, ..] if ok(actor, true) && ok(rkey, false) => {
                Some(((*actor).into(), (*rkey).into()))
            }
            _ => None,
        }
    }

    pub fn handles(&self, url: &Url) -> bool {
        Self::parse(url).is_some()
    }

    pub async fn resolve(&self, url: &Url) -> Result<Resolved, ProviderError> {
        let (actor, rkey) = Self::parse(url)
            .ok_or_else(|| ProviderError::NoMedia("lien Bluesky non reconnu".into()))?;
        let mut api = self
            .base
            .join("xrpc/app.bsky.feed.getPostThread")
            .map_err(|_| ProviderError::NoMedia("base invalide".into()))?;
        api.query_pairs_mut()
            .append_pair("uri", &format!("at://{actor}/app.bsky.feed.post/{rkey}"))
            .append_pair("depth", "0")
            .append_pair("parentHeight", "0");
        let v = self.http.get_json(&api).await?;
        let post = &v["thread"]["post"];
        let embed = &post["embed"];
        // Vidéo directe, ou vidéo dans un « recordWithMedia ».
        let playlist = [embed, &embed["media"]]
            .into_iter()
            .find(|e| {
                e["$type"]
                    .as_str()
                    .is_some_and(|t| t.starts_with("app.bsky.embed.video"))
            })
            .and_then(|e| e["playlist"].as_str())
            .ok_or_else(|| ProviderError::NoMedia("pas de vidéo dans ce post".into()))?;
        Ok(Resolved {
            media_url: self.http.media_url(&self.base, playlist).await?,
            title: post["record"]["text"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            duration_s: None,
            kind: MediaKind::Video,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::providers::source::testsrv::{fx, serve, PUBLIC_MEDIA};

    const OK: &str = include_str!("../../tests/fixtures/providers/bluesky_thread.json");
    const LINK: &str = "https://bsky.app/profile/alice.bsky.social/post/3myi3k45jh22z";

    fn http() -> Http {
        Http::with_limits(Duration::from_millis(400), 4096, true, false)
    }

    async fn provider(status: u16, body: String) -> Bluesky {
        Bluesky::new(http(), serve(status, body, 0).await)
    }

    #[test]
    fn handles_only_post_links() {
        let p = Bluesky::with_default_base(http());
        assert!(p.handles(&Url::parse(LINK).unwrap()));
        assert!(!p.handles(&Url::parse("https://bsky.app/profile/alice.bsky.social").unwrap()));
        assert!(!p.handles(&Url::parse("https://x.com/profile/a/post/b").unwrap()));
    }

    #[tokio::test]
    async fn nominal() {
        let p = provider(200, fx(OK, PUBLIC_MEDIA)).await;
        let r = p.resolve(&Url::parse(LINK).unwrap()).await.unwrap();
        assert_eq!(r.media_url.as_str(), PUBLIC_MEDIA);
        assert_eq!(r.title, "Ma video Bluesky");
    }

    #[tokio::test]
    async fn invalid_json() {
        let p = provider(200, "<html>pas du json".into()).await;
        assert!(matches!(
            p.resolve(&Url::parse(LINK).unwrap()).await,
            Err(ProviderError::Json(_))
        ));
    }

    #[tokio::test]
    async fn server_error_status() {
        let p = provider(503, "{}".into()).await;
        assert!(matches!(
            p.resolve(&Url::parse(LINK).unwrap()).await,
            Err(ProviderError::Status(503))
        ));
    }
}
