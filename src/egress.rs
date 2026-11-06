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
/// Taille des blocs lus sur disque pour le corps de la requête.
const CHUNK: usize = 64 * 1024;
/// Essais d'envoi, comme utils/curl_uploader.py.
const ATTEMPTS: u32 = 3;

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

/// SHA-256 du fichier en hexadécimal (lecture par blocs, hors du runtime).
async fn sha256_hex(path: &Path) -> std::io::Result<String> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        use sha2::{Digest, Sha256};
        use std::io::Read;
        let mut f = std::fs::File::open(path)?;
        let mut h = Sha256::new();
        let mut buf = vec![0u8; CHUNK];
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
        }
        Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
    })
    .await
    .map_err(std::io::Error::other)?
}

/// URL cible : `<EXTERNAL_UPLOAD_URL>/<sha256><ext>` (renommage par hash du bot Python).
pub fn target_url(base: &Url, hash: &str, path: &Path) -> Result<Url, EgressError> {
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let mut base = base.clone();
    if !base.path().ends_with('/') {
        base.set_path(&format!("{}/", base.path()));
    }
    base.join(&format!("{hash}{ext}"))
        .map_err(|_| EgressError::BadResponse("URL d'envoi invalide".into()))
}

/// Envoie `path` par PUT vers EXTERNAL_UPLOAD_URL (curl.libriciel.fr par défaut, comme le bot
/// Python), 3 essais, et rend le lien public ; `Disabled` si l'envoi externe est coupé.
/// `progress` reçoit 10, 20 … 100 au fil de l'envoi.
pub async fn upload_external(
    cfg: &Config,
    path: &Path,
    progress: Option<tokio::sync::mpsc::UnboundedSender<u8>>,
) -> Result<Url, EgressError> {
    let base = cfg
        .external_upload_url
        .clone()
        .ok_or(EgressError::Disabled)?;
    let target = target_url(&base, &sha256_hex(path).await?, path)?;
    tracing::info!(
        "[CURL UPLOAD] Upload du fichier '{}' vers '{target}'.",
        path.display()
    );
    let total = cfg.external_upload_timeout;
    let client = reqwest::Client::builder()
        .connect_timeout(total.min(Duration::from_secs(10)))
        .timeout(total)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(http_err)?;
    let mut last = EgressError::Disabled;
    for attempt in 1..=ATTEMPTS {
        match put_once(&client, &target, path, progress.clone()).await {
            Ok(u) => {
                tracing::info!(
                    "[CURL UPLOAD] Upload terminé pour '{}'. Réponse: {u}",
                    target
                );
                return Ok(u);
            }
            // Réponse illisible : pas la peine de réessayer, le service répondra pareil.
            Err(e @ EgressError::BadResponse(_)) => return Err(e),
            Err(e) => {
                tracing::error!(
                    "[CURL UPLOAD] Tentative {attempt} échouée pour '{}': {e}",
                    path.display()
                );
                last = e;
            }
        }
    }
    Err(last)
}

async fn put_once(
    client: &reqwest::Client,
    target: &Url,
    path: &Path,
    progress: Option<tokio::sync::mpsc::UnboundedSender<u8>>,
) -> Result<Url, EgressError> {
    let file = tokio::fs::File::open(path).await?;
    let len = file.metadata().await?.len();
    // Lecture par blocs : le fichier n'est jamais entièrement en mémoire.
    let state = (file, 0u64, 0u8);
    let body =
        reqwest::Body::wrap_stream(stream::unfold(state, move |(mut f, mut sent, mut step)| {
            let progress = progress.clone();
            async move {
                let mut buf = vec![0u8; CHUNK];
                match f.read(&mut buf).await {
                    Ok(0) => None,
                    Ok(n) => {
                        buf.truncate(n);
                        sent += n as u64;
                        if let Some(p) = (sent * 100).checked_div(len) {
                            let pct = p.min(100) as u8 / 10 * 10;
                            if pct > step {
                                step = pct;
                                if let Some(tx) = &progress {
                                    let _ = tx.send(pct);
                                }
                            }
                        }
                        Some((Ok::<_, std::io::Error>(buf), (f, sent, step)))
                    }
                    Err(e) => Some((Err(e), (f, sent, step))),
                }
            }
        }));
    let mut resp = client
        .put(target.clone())
        .header(reqwest::header::CONTENT_LENGTH, len)
        .body(body)
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
    use axum::{http::StatusCode, routing::put, Router};
    use std::collections::HashMap;

    async fn serve(app: Router) -> String {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
        format!("http://{addr}")
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
            "/{name}",
            put(
                |axum::extract::Path(name): axum::extract::Path<String>,
                 body: axum::body::Bytes| async move {
                    assert_eq!(body.len(), 200_000, "le fichier entier, tel quel");
                    assert_eq!(name.len(), 64 + 4, "<sha256>.mp4 : {name}");
                    "https://files.example/abc\n"
                },
            ),
        ))
        .await;
        let (_d, p) = tmpfile();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let u = upload_external(&cfg(&url, "5"), &p, Some(tx))
            .await
            .unwrap();
        assert_eq!(u.as_str(), "https://files.example/abc");
        let mut steps = Vec::new();
        while let Ok(x) = rx.try_recv() {
            steps.push(x);
        }
        assert_eq!(steps.last(), Some(&100), "progression : {steps:?}");
    }

    #[tokio::test]
    async fn egress_500_refuse() {
        let url = serve(Router::new().route(
            "/{name}",
            put(|_b: axum::body::Bytes| async {
                (StatusCode::INTERNAL_SERVER_ERROR, "https://x.example/ok")
            }),
        ))
        .await;
        let (_d, p) = tmpfile();
        let e = upload_external(&cfg(&url, "5"), &p, None)
            .await
            .unwrap_err();
        assert!(
            matches!(e, EgressError::Http(ref s) if s.contains("500")),
            "{e:?}"
        );
    }

    #[tokio::test]
    async fn egress_html_refuse() {
        let url = serve(Router::new().route(
            "/{name}",
            put(|_b: axum::body::Bytes| async { "<html><body>Bad gateway</body></html>" }),
        ))
        .await;
        let (_d, p) = tmpfile();
        let e = upload_external(&cfg(&url, "5"), &p, None)
            .await
            .unwrap_err();
        assert!(matches!(e, EgressError::BadResponse(_)), "{e:?}");
        assert!(parse_link(b"http://x.example/a").is_err());
    }

    #[tokio::test]
    async fn egress_timeout_rapide() {
        let url = serve(Router::new().route(
            "/{name}",
            put(|| async {
                tokio::time::sleep(Duration::from_secs(30)).await;
                "https://x.example/late"
            }),
        ))
        .await;
        let (_d, p) = tmpfile();
        let t = std::time::Instant::now();
        let e = upload_external(&cfg(&url, "1"), &p, None)
            .await
            .unwrap_err();
        assert!(
            matches!(e, EgressError::Http(ref s) if s.contains("délai")),
            "{e:?}"
        );
        assert!(t.elapsed() < Duration::from_secs(5), "3 essais de 1 s");
    }

    #[tokio::test]
    async fn egress_desactive_sans_url() {
        let off = cfg("off", "5");
        assert!(off.external_upload_url.is_none());
        let (_d, p) = tmpfile();
        let e = upload_external(&off, &p, None).await;
        assert!(matches!(e, Err(EgressError::Disabled)));
    }

    #[tokio::test]
    async fn egress_reessaie_jusqu_a_trois_fois() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let url = serve(Router::new().route(
            "/{name}",
            put(|_b: axum::body::Bytes| async {
                if N.fetch_add(1, Ordering::SeqCst) < 2 {
                    (StatusCode::BAD_GATEWAY, String::new())
                } else {
                    (StatusCode::OK, "https://files.example/ok".to_string())
                }
            }),
        ))
        .await;
        let (_d, p) = tmpfile();
        let u = upload_external(&cfg(&url, "5"), &p, None).await.unwrap();
        assert_eq!(
            (u.as_str(), N.load(Ordering::SeqCst)),
            ("https://files.example/ok", 3)
        );
    }

    #[test]
    fn libriciel_par_defaut_et_nom_par_hash() {
        let m: HashMap<String, String> = [("BOT_TOKEN".to_string(), "t".to_string())].into();
        let c = Config::from_map(&m).unwrap();
        let base = c.external_upload_url.expect("libriciel par défaut");
        assert_eq!(base.as_str(), "https://curl.libriciel.fr/");
        let u = target_url(&base, "ab12", Path::new("/x/v.mp4")).unwrap();
        assert_eq!(u.as_str(), "https://curl.libriciel.fr/ab12.mp4");
        let sub = Url::parse("http://h/up").unwrap();
        assert_eq!(
            target_url(&sub, "ab", Path::new("a.mp3")).unwrap().as_str(),
            "http://h/up/ab.mp3"
        );
    }

    #[test]
    fn consentement_nomme_hote() {
        let t = consent_text_for(80, "files.example");
        assert!(t.contains("80 Mo") && t.contains("files.example") && t.contains("Telegram"));
    }
}
