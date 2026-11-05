//! Routeur : choisit un provider d'API activé qui gère l'URL, sinon retombe sur yt-dlp.
//!
//! Réglages attendus : `providers.order`, `providers.<id>.enabled`, `providers.ytdlp_fallback`.
//! Sans aucun provider activé (ou qui gère l'URL), la décision est toujours `YtDlp`.

use url::Url;

use super::source::{Provider, ProviderError, Resolved};

/// Résultat du routage.
#[derive(Debug)]
pub enum Decision {
    /// Média résolu par le provider dont l'id est donné.
    Direct(Resolved, String),
    /// Laisser yt-dlp traiter l'URL.
    YtDlp,
    /// Tous les providers ont échoué et le repli yt-dlp est désactivé (dernière erreur).
    Refused(ProviderError),
}

pub struct Router {
    providers: Vec<Provider>,
    order: Vec<String>,
    enabled: Box<dyn Fn(&str) -> bool + Send + Sync>,
    ytdlp_fallback: bool,
}

impl Router {
    pub fn new(
        providers: Vec<Provider>,
        order: Vec<String>,
        enabled: impl Fn(&str) -> bool + Send + Sync + 'static,
        ytdlp_fallback: bool,
    ) -> Self {
        Self {
            providers,
            order,
            enabled: Box::new(enabled),
            ytdlp_fallback,
        }
    }

    pub async fn route(&self, url: &Url) -> Decision {
        // Tri stable : ids de `order` d'abord (dans cet ordre), les autres ensuite.
        let rank = |p: &Provider| {
            self.order
                .iter()
                .position(|o| o == p.id())
                .unwrap_or(usize::MAX)
        };
        let mut candidates: Vec<&Provider> = self
            .providers
            .iter()
            .filter(|p| (self.enabled)(p.id()) && p.handles(url))
            .collect();
        candidates.sort_by_key(|p| rank(p));
        let mut last = None;
        for p in candidates {
            match p.resolve(url).await {
                Ok(r) => return Decision::Direct(r, p.id().to_string()),
                Err(e) => {
                    tracing::warn!(provider = p.id(), error = %e, "provider en échec");
                    last = Some(e);
                }
            }
        }
        match (self.ytdlp_fallback, last) {
            (false, Some(e)) => Decision::Refused(e),
            _ => Decision::YtDlp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url() -> Url {
        Url::parse("https://x.com/a/status/1").unwrap()
    }

    fn stub(id: &'static str, ok: bool) -> Provider {
        Provider::Stub { id, ok }
    }

    fn label(d: &Decision) -> String {
        match d {
            Decision::Direct(_, id) => id.clone(),
            Decision::YtDlp => "ytdlp".into(),
            Decision::Refused(_) => "refused".into(),
        }
    }

    #[tokio::test]
    async fn default_nothing_enabled_is_ytdlp() {
        let r = Router::new(vec![stub("x", true)], vec![], |_| false, false);
        assert_eq!(label(&r.route(&url()).await), "ytdlp");
        let r = Router::new(vec![], vec![], |_| true, false);
        assert_eq!(label(&r.route(&url()).await), "ytdlp");
    }

    #[tokio::test]
    async fn order_is_respected() {
        let provs = || vec![stub("tiktok", true), stub("x", true)];
        let r = Router::new(provs(), vec!["x".into(), "tiktok".into()], |_| true, false);
        assert_eq!(label(&r.route(&url()).await), "x");
        let r = Router::new(provs(), vec!["tiktok".into(), "x".into()], |_| true, false);
        assert_eq!(label(&r.route(&url()).await), "tiktok");
    }

    #[tokio::test]
    async fn disabled_provider_is_skipped() {
        let provs = vec![stub("x", true), stub("tiktok", true)];
        let r = Router::new(provs, vec!["x".into()], |id| id == "tiktok", false);
        assert_eq!(label(&r.route(&url()).await), "tiktok");
    }

    #[tokio::test]
    async fn falls_through_failures_then_ytdlp() {
        let provs = vec![stub("x", false), stub("bluesky", true)];
        let r = Router::new(provs, vec!["x".into(), "bluesky".into()], |_| true, true);
        assert_eq!(label(&r.route(&url()).await), "bluesky");
        let r = Router::new(vec![stub("x", false)], vec![], |_| true, true);
        assert_eq!(label(&r.route(&url()).await), "ytdlp");
    }

    #[tokio::test]
    async fn fallback_disabled_gives_error() {
        let r = Router::new(vec![stub("x", false)], vec![], |_| true, false);
        assert!(matches!(
            r.route(&url()).await,
            Decision::Refused(ProviderError::NoMedia(_))
        ));
    }

    #[tokio::test]
    async fn real_provider_failure_falls_back() {
        use crate::providers::{source::testsrv::serve, source::Http, x_api::XApi};
        let base = serve(500, "{}".into(), 0).await;
        let http = Http::with_limits(std::time::Duration::from_millis(400), 4096, true, false);
        let r = Router::new(
            vec![Provider::X(XApi::new(http, base))],
            vec![],
            |_| true,
            true,
        );
        assert_eq!(label(&r.route(&url()).await), "ytdlp");
    }
}
