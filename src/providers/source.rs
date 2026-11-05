//! Socle commun des providers « API publique » : type `Resolved`, erreurs, client HTTP borné
//! et enum `Provider` (pas de `async_trait` dans le projet : un enum suffit pour trois providers).

use std::time::Duration;

use reqwest::redirect::Policy;
use serde_json::Value;
use url::Url;

use super::{bluesky::Bluesky, tiktok_api::TikTokApi, x_api::XApi};
use crate::{error::UrlError, url_guard};

/// Taille maximale d'une réponse JSON d'API (2 Mo).
pub const MAX_JSON_BYTES: usize = 2 * 1024 * 1024;
const USER_AGENT: &str = "SocialVideoDownload/0.1 (+bot Telegram)";

/// Nature du média résolu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Video,
}

/// Média résolu par un provider : `media_url` est déjà validée anti-SSRF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub media_url: Url,
    pub title: String,
    pub duration_s: Option<u64>,
    pub kind: MediaKind,
}

/// Échec d'un provider (message sans secret : jamais d'en-tête ni de jeton).
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("requête échouée : {0}")]
    Http(String),
    #[error("statut HTTP {0}")]
    Status(u16),
    #[error("délai dépassé")]
    Timeout,
    #[error("réponse trop volumineuse (> {0} octets)")]
    TooLarge(usize),
    #[error("JSON invalide : {0}")]
    Json(String),
    #[error("aucun média exploitable : {0}")]
    NoMedia(String),
    #[error("URL refusée : {0}")]
    Url(#[from] UrlError),
}

/// Client HTTP des providers : pas de redirection, délais, lecture bornée, anti-SSRF.
#[derive(Clone)]
pub struct Http {
    client: reqwest::Client,
    max_bytes: usize,
    /// Autorise une IP privée pour l'URL de l'API (serveurs factices des tests seulement).
    allow_private_api: bool,
    /// Autorise une IP privée pour `media_url` (UNSAFE_ALLOW_PRIVATE_HOSTS seulement).
    allow_private_media: bool,
}

impl Http {
    /// Réglage de production : `allow_private` = `UNSAFE_ALLOW_PRIVATE_HOSTS` (faux par défaut).
    pub fn new(allow_private: bool) -> Self {
        Self::with_limits(
            Duration::from_secs(15),
            MAX_JSON_BYTES,
            allow_private,
            allow_private,
        )
    }

    pub fn with_limits(
        total_timeout: Duration,
        max_bytes: usize,
        allow_private_api: bool,
        allow_private_media: bool,
    ) -> Self {
        let client = reqwest::Client::builder()
            .redirect(Policy::none()) // une redirection = statut 3xx = échec, jamais suivie
            .connect_timeout(Duration::from_secs(5).min(total_timeout))
            .timeout(total_timeout)
            .user_agent(USER_AGENT)
            .build()
            .expect("client reqwest (rustls) constructible");
        Self {
            client,
            max_bytes,
            allow_private_api,
            allow_private_media,
        }
    }

    /// GET JSON borné : URL validée, statut 2xx, corps lu en flux et coupé à `max_bytes`.
    pub(crate) async fn get_json(&self, url: &Url) -> Result<Value, ProviderError> {
        url_guard::validate(url.as_str(), self.allow_private_api).await?;
        let mut resp = self.client.get(url.clone()).send().await.map_err(map_err)?;
        if !resp.status().is_success() {
            return Err(ProviderError::Status(resp.status().as_u16()));
        }
        if resp
            .content_length()
            .is_some_and(|n| n > self.max_bytes as u64)
        {
            return Err(ProviderError::TooLarge(self.max_bytes));
        }
        let mut body = Vec::new();
        while let Some(chunk) = resp.chunk().await.map_err(map_err)? {
            if body.len() + chunk.len() > self.max_bytes {
                return Err(ProviderError::TooLarge(self.max_bytes));
            }
            body.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&body).map_err(|e| ProviderError::Json(e.to_string()))
    }

    /// Valide une URL de média renvoyée par une API (jamais de confiance dans la réponse).
    pub(crate) async fn media_url(&self, base: &Url, raw: &str) -> Result<Url, ProviderError> {
        let url = base
            .join(raw)
            .map_err(|_| ProviderError::NoMedia("URL de média illisible".into()))?;
        Ok(url_guard::validate(url.as_str(), self.allow_private_media).await?)
    }
}

fn map_err(e: reqwest::Error) -> ProviderError {
    if e.is_timeout() {
        ProviderError::Timeout
    } else {
        ProviderError::Http(e.without_url().to_string())
    }
}

/// Un provider d'API publique.
pub enum Provider {
    X(XApi),
    Bluesky(Bluesky),
    TikTok(TikTokApi),
    /// Faux provider pour tester l'ordre du routeur.
    #[cfg(test)]
    Stub {
        id: &'static str,
        ok: bool,
    },
}

impl Provider {
    pub fn id(&self) -> &'static str {
        match self {
            Self::X(_) => "x",
            Self::Bluesky(_) => "bluesky",
            Self::TikTok(_) => "tiktok",
            #[cfg(test)]
            Self::Stub { id, .. } => id,
        }
    }

    pub fn handles(&self, url: &Url) -> bool {
        match self {
            Self::X(p) => p.handles(url),
            Self::Bluesky(p) => p.handles(url),
            Self::TikTok(p) => p.handles(url),
            #[cfg(test)]
            Self::Stub { .. } => true,
        }
    }

    pub async fn resolve(&self, url: &Url) -> Result<Resolved, ProviderError> {
        match self {
            Self::X(p) => p.resolve(url).await,
            Self::Bluesky(p) => p.resolve(url).await,
            Self::TikTok(p) => p.resolve(url).await,
            #[cfg(test)]
            Self::Stub { id, ok } => {
                if *ok {
                    Ok(Resolved {
                        media_url: Url::parse(&format!("https://93.184.215.14/{id}.mp4")).unwrap(),
                        title: (*id).into(),
                        duration_s: None,
                        kind: MediaKind::Video,
                    })
                } else {
                    Err(ProviderError::NoMedia("stub".into()))
                }
            }
        }
    }
}

/// Aides de test : serveur axum local (réponse fixe, délai optionnel).
#[cfg(test)]
pub(crate) mod testsrv {
    use std::time::Duration;

    use axum::{http::StatusCode, response::IntoResponse, Router};
    use url::Url;

    /// Sert `body` avec `status` sur toutes les routes après `delay_ms` ; renvoie la base.
    pub async fn serve(status: u16, body: String, delay_ms: u64) -> Url {
        let app = Router::new().fallback(move || {
            let body = body.clone();
            async move {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                (StatusCode::from_u16(status).unwrap(), body).into_response()
            }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, app).await.ok() });
        Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap()
    }

    pub const PUBLIC_MEDIA: &str = "https://93.184.215.14/v/clip.mp4";

    /// Fixture JSON dont le jeton `__MEDIA__` est remplacé par `media`.
    pub fn fx(raw: &str, media: &str) -> String {
        raw.replace("__MEDIA__", media)
    }
}
