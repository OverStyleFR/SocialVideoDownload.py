//! Repli d'hébergement externe pour les fichiers trop gros pour Telegram.

use std::path::Path;
use std::time::Duration;

use futures_util::stream;
use tokio::io::AsyncReadExt;
use url::Url;

use crate::config::Config;
use crate::error::EgressError;

/// Taille maximale lue dans la réponse : une URL tient en quelques centaines d'octets.
const MAX_RESPONSE_BYTES: usize = 4096;
/// Taille des blocs lus sur disque pour le flux multipart.
const CHUNK: usize = 64 * 1024;

fn http_err(e: reqwest::Error) -> EgressError {
    let what = if e.is_timeout() {
        "délai dépassé"
    } else if e.is_connect() {
        "connexion impossible"
    } else {
        "requête échouée"
    };
    EgressError::Http(what.into())
}

/// Envoie `path` vers EXTERNAL_UPLOAD_URL et rend le lien public ; `Disabled` si non configuré.
pub async fn upload_external(cfg: &Config, path: &Path) -> Result<Url, EgressError> {
    let target = cfg
        .external_upload_url
        .clone()
        .ok_or(EgressError::Disabled)?;
    let total = cfg.external_upload_timeout;
    let file = tokio::fs::File::open(path).await?;
    let len = file.metadata().await?.len();
    let name = path
        .file_name()
        .map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned());

    // Lecture par blocs : le fichier n'est jamais entièrement en mémoire.
    let body = reqwest::Body::wrap_stream(stream::unfold(file, |mut f| async move {
        let mut buf = vec![0u8; CHUNK];
        match f.read(&mut buf).await {
            Ok(0) => None,
            Ok(n) => {
                buf.truncate(n);
                Some((Ok::<_, std::io::Error>(buf), f))
            }
            Err(e) => Some((Err(e), f)),
        }
    }));
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::stream_with_length(body, len).file_name(name),
    );

    let client = reqwest::Client::builder()
        .connect_timeout(total.min(Duration::from_secs(10)))
        .timeout(total)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(http_err)?;
    let mut resp = client
        .post(target)
        .multipart(form)
        .send()
        .await
        .map_err(http_err)?;

    let status = resp.status();
    if !status.is_success() {
        return Err(EgressError::Http(format!("statut {}", status.as_u16())));
    }
    let mut raw = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(http_err)? {
        if raw.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(EgressError::BadResponse("réponse trop volumineuse".into()));
        }
        raw.extend_from_slice(&chunk);
    }
    parse_link(&raw)
}

/// La réponse doit être exactement une URL https ; tout le reste (page d'erreur, HTML) est refusé.
fn parse_link(raw: &[u8]) -> Result<Url, EgressError> {
    let bad = |m: &str| EgressError::BadResponse(m.into());
    let text = std::str::from_utf8(raw).map_err(|_| bad("réponse non textuelle"))?;
    let url = Url::parse(text.trim()).map_err(|_| bad("la réponse n'est pas une URL"))?;
    if url.scheme() != "https" || url.host_str().is_none() {
        return Err(bad("l'URL reçue n'est pas en https"));
    }
    Ok(url)
}

/// Texte demandant le consentement avant d'envoyer un fichier de `size_mb` Mo hors de Telegram.
///
/// Signature figée du squelette, sans l'hôte : l'appelant qui a la config utilise [`consent_text_for`].
pub fn consent_text(size_mb: u64) -> String {
    consent_text_for(size_mb, "un hébergeur tiers")
}

/// Variante nommant l'hôte configuré (`cfg.external_upload_url`).
pub fn consent_text_for(size_mb: u64, host: &str) -> String {
    format!(
        "Ce fichier fait {size_mb} Mo : il dépasse la limite d'envoi de Telegram. \
         Je peux le déposer chez un hébergeur tiers ({host}) et te donner un lien public. \
         Le fichier quitterait Telegram et serait stocké sur ce service externe. \
         Acceptes-tu cet envoi ? (Oui / Non)"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{http::StatusCode, routing::post, Router};
    use std::collections::HashMap;

    async fn serve(app: Router) -> String {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
        format!("http://{addr}/up")
    }

    fn cfg(url: &str, timeout_s: &str) -> Config {
        let m: HashMap<String, String> = [
            ("BOT_TOKEN", "t"),
            ("EXTERNAL_UPLOAD_URL", url),
            ("EXTERNAL_UPLOAD_TIMEOUT_S", timeout_s),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect();
        Config::from_map(&m).unwrap()
    }

    fn tmpfile() -> (tempfile::TempDir, std::path::PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("v.mp4");
        std::fs::write(&p, vec![7u8; 200_000]).unwrap();
        (d, p)
    }

    #[tokio::test]
    async fn egress_200_url_ok() {
        let url = serve(Router::new().route(
            "/up",
            post(|body: axum::body::Bytes| async move {
                assert!(body.len() > 200_000); // le fichier est bien dans le multipart
                "https://files.example/abc\n"
            }),
        ))
        .await;
        let (_d, p) = tmpfile();
        let u = upload_external(&cfg(&url, "5"), &p).await.unwrap();
        assert_eq!(u.as_str(), "https://files.example/abc");
    }

    #[tokio::test]
    async fn egress_500_refuse() {
        let url = serve(Router::new().route(
            "/up",
            post(|_b: axum::body::Bytes| async {
                (StatusCode::INTERNAL_SERVER_ERROR, "https://x.example/ok")
            }),
        ))
        .await;
        let (_d, p) = tmpfile();
        let e = upload_external(&cfg(&url, "5"), &p).await.unwrap_err();
        assert!(
            matches!(e, EgressError::Http(ref s) if s.contains("500")),
            "{e:?}"
        );
    }

    #[tokio::test]
    async fn egress_html_refuse() {
        let url = serve(Router::new().route(
            "/up",
            post(|_b: axum::body::Bytes| async { "<html><body>Bad gateway</body></html>" }),
        ))
        .await;
        let (_d, p) = tmpfile();
        let e = upload_external(&cfg(&url, "5"), &p).await.unwrap_err();
        assert!(matches!(e, EgressError::BadResponse(_)), "{e:?}");
        assert!(parse_link(b"http://x.example/a").is_err());
    }

    #[tokio::test]
    async fn egress_timeout_rapide() {
        let url = serve(Router::new().route(
            "/up",
            post(|| async {
                tokio::time::sleep(Duration::from_secs(30)).await;
                "https://x.example/late"
            }),
        ))
        .await;
        let (_d, p) = tmpfile();
        let t = std::time::Instant::now();
        let e = upload_external(&cfg(&url, "1"), &p).await.unwrap_err();
        assert!(
            matches!(e, EgressError::Http(ref s) if s.contains("délai")),
            "{e:?}"
        );
        assert!(t.elapsed() < Duration::from_secs(3));
    }

    #[tokio::test]
    async fn egress_desactive_sans_url() {
        let m: HashMap<String, String> = [("BOT_TOKEN".to_string(), "t".to_string())].into();
        let (_d, p) = tmpfile();
        let e = upload_external(&Config::from_map(&m).unwrap(), &p).await;
        assert!(matches!(e, Err(EgressError::Disabled)));
    }

    #[test]
    fn consentement_nomme_hote() {
        let t = consent_text_for(80, "files.example");
        assert!(t.contains("80 Mo") && t.contains("files.example") && t.contains("Telegram"));
    }
}
