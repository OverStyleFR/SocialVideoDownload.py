//! Routeur de providers branché dans le téléchargeur : faux serveur d'API X + faux yt-dlp.

use std::collections::HashMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{body::Body, extract::State, http::StatusCode, routing::get, Router};
use socialvideodownload::config::Config;
use socialvideodownload::download::{Downloader, Mode};
use socialvideodownload::error::DownloadError;
use socialvideodownload::retention::JobDir;
use socialvideodownload::settings::SettingsStore;
use url::Url;

const MIB: usize = 1024 * 1024;

/// Copie un faux outil de tests/fixtures dans `dir` (même logique que tests/e2e.rs).
fn tool(dir: &Path, name: &str) -> PathBuf {
    let ext = if cfg!(windows) { "cmd" } else { "sh" };
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{name}.{ext}"));
    let dst = dir.join(format!("{name}.{ext}"));
    let text = std::fs::read_to_string(src).unwrap().replace("\r\n", "\n");
    std::fs::write(&dst, text).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dst
}

/// Faux API X : `/u/status/<id>` renvoie du JSON fxtwitter (sauf id 404), `/v.mp4` sert `media_len` octets.
async fn fake_api(media_len: usize) -> Url {
    let app = Router::new()
        .route(
            "/u/status/{id}",
            get(|State(base): State<String>, axum::extract::Path(id): axum::extract::Path<String>| async move {
                if id == "404" {
                    return (StatusCode::NOT_FOUND, String::new());
                }
                (
                    StatusCode::OK,
                    format!(r#"{{"tweet":{{"text":"titre api","media":{{"videos":[{{"url":"{base}v.mp4"}}]}}}}}}"#),
                )
            }),
        )
        .route(
            "/v.mp4",
            get(move || async move {
                // Flux sans Content-Length : la borne doit s'appliquer pendant la lecture.
                let chunks = (0..media_len / MIB).map(|_| Ok::<_, Infallible>(vec![7u8; MIB]));
                Body::from_stream(futures_util::stream::iter(chunks))
            }),
        )
        .with_state(String::new());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!(
        "http://127.0.0.1:{}/",
        listener.local_addr().unwrap().port()
    );
    // L'état porte la base réelle, connue seulement après le bind.
    let app = app.with_state(base.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    Url::parse(&base).unwrap()
}

struct Env {
    dir: tempfile::TempDir,
    dl: Downloader,
    store: SettingsStore,
}

impl Env {
    fn ytdlp_calls(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.path().join("calls.txt"))
            .unwrap_or_default()
            .lines()
            .map(|l| l.trim().to_string())
            .collect()
    }
    async fn fetch(
        &self,
        id: &str,
    ) -> Result<socialvideodownload::download::Downloaded, DownloadError> {
        let job = JobDir::create(&self.dir.path().join("jobs"), id).unwrap();
        let url = Url::parse(&format!("https://x.com/u/status/{id}")).unwrap();
        let r = self.dl.fetch(&url, Mode::Video, &job).await;
        std::mem::forget(job); // le fichier doit survivre à la lecture du test
        r
    }
}

async fn env(media_len: usize, max_mb: &str, enable_x: bool, with_settings: bool) -> Env {
    let api = fake_api(media_len).await;
    let dir = tempfile::tempdir().unwrap();
    let map: HashMap<String, String> = [
        ("BOT_TOKEN", "123:abc"),
        ("ALLOWED_USER_IDS", "111"),
        ("ADMIN_USER_IDS", "111"),
        ("UNSAFE_ALLOW_PRIVATE_HOSTS", "1"),
        ("MAX_FILESIZE_MB", max_mb),
        ("DOWNLOAD_DIR", dir.path().join("dl").to_str().unwrap()),
        (
            "YTDLP_PATH",
            tool(dir.path(), "fake-ytdlp").to_str().unwrap(),
        ),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    let cfg = Arc::new(Config::from_map(&map).unwrap());
    let store = SettingsStore::load(&dir.path().join("settings.json"), &cfg).unwrap();
    if enable_x {
        store.set("providers.x.enabled", "on", 111).unwrap();
    }
    let mut dl = Downloader::new(cfg).with_api_base(api);
    if with_settings {
        dl = dl.with_settings(store.clone());
    }
    Env { dir, dl, store }
}

#[tokio::test(flavor = "multi_thread")]
async fn provider_actif_telecharge_sans_ytdlp() {
    let e = env(MIB, "5", true, true).await;
    let d = e.fetch("123").await.expect("téléchargement direct");
    assert_eq!(d.id, "123");
    assert_eq!(d.title, "titre api");
    assert_eq!(d.size, MIB as u64);
    assert_eq!(std::fs::metadata(&d.path).unwrap().len(), MIB as u64);
    assert!(e.ytdlp_calls().is_empty(), "yt-dlp ne doit pas être appelé");
}

#[tokio::test(flavor = "multi_thread")]
async fn provider_en_echec_retombe_sur_ytdlp() {
    let e = env(MIB, "5", true, true).await;
    let d = e.fetch("404").await.expect("repli yt-dlp");
    assert_eq!(d.title, "titre 404");
    assert_eq!(e.ytdlp_calls(), ["404"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn sans_repli_l_echec_du_provider_est_refuse() {
    let e = env(MIB, "5", true, true).await;
    e.store.set("providers.ytdlp_fallback", "off", 111).unwrap();
    assert!(matches!(e.fetch("404").await, Err(DownloadError::Tool(_))));
    assert!(e.ytdlp_calls().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn reglages_par_defaut_ou_absents_donnent_ytdlp() {
    for (enable, settings) in [(false, true), (true, false)] {
        let e = env(MIB, "5", enable, settings).await;
        e.fetch("123").await.expect("yt-dlp seul");
        assert_eq!(e.ytdlp_calls(), ["123"]);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn taille_depassee_erreur_propre_sans_fichier() {
    let e = env(2 * MIB, "1", true, true).await;
    match e.fetch("123").await {
        Err(DownloadError::TooLarge { limit_mb: 1, .. }) => {}
        other => panic!("TooLarge attendu, reçu {other:?}"),
    }
    assert!(e.ytdlp_calls().is_empty());
    assert!(!e.dir.path().join("jobs/123/123.mp4").exists());
}
