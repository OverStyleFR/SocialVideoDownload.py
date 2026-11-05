//! Le jeton n'apparaît dans AUCUNE ligne de journal, erreurs réseau comprises.

use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use socialvideodownload::bot::{self, AppState};
use socialvideodownload::config::Config;
use socialvideodownload::telemetry;

const TOKEN: &str = "123456:FAKE-TOKEN-xyz";

static LOG: OnceLock<Arc<Mutex<Vec<u8>>>> = OnceLock::new();

struct Sink(Arc<Mutex<Vec<u8>>>);
impl Write for Sink {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Installe (une fois) le subscriber vers un tampon mémoire ; rend le tampon.
fn log() -> Arc<Mutex<Vec<u8>>> {
    LOG.get_or_init(|| {
        let buf = Arc::new(Mutex::new(Vec::new()));
        let b = buf.clone();
        telemetry::init_with_writer(TOKEN, move || Sink(b.clone()));
        buf
    })
    .clone()
}

fn text(buf: &Arc<Mutex<Vec<u8>>>) -> String {
    String::from_utf8_lossy(&buf.lock().unwrap()).into_owned()
}

fn config(api: &str) -> Arc<Config> {
    let env: HashMap<String, String> = [
        ("BOT_TOKEN", TOKEN),
        ("TELEGRAM_API_URL", api),
        ("ALLOWED_USER_IDS", "111"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    Arc::new(Config::from_map(&env).unwrap())
}

async fn run_bot_then_check(api: &str, marque: &str) {
    let buf = log();
    let h = tokio::spawn(bot::run(AppState::new(config(api))));
    tokio::time::sleep(Duration::from_secs(6)).await;
    h.abort();
    let t = text(&buf);
    assert!(
        t.contains("ERROR") || t.contains("WARN") || t.contains("error"),
        "{marque} : aucune ligne d'erreur capturée : {t}"
    );
    assert!(
        !t.contains("FAKE-TOKEN"),
        "{marque} : jeton dans le journal : {t}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn connexion_refusee_ne_fuit_pas_le_jeton() {
    run_bot_then_check("http://127.0.0.1:1", "port fermé").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn reponse_500_ne_fuit_pas_le_jeton() {
    use axum::{http::StatusCode, Router};
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    let app = Router::new().fallback(|| async { StatusCode::INTERNAL_SERVER_ERROR });
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    run_bot_then_check(&url, "500").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn erreur_reqwest_directe_ne_fuit_pas_le_jeton() {
    let buf = log();
    let err = reqwest::get(format!("http://127.0.0.1:1/bot{TOKEN}/getMe"))
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("FAKE-TOKEN"),
        "l'erreur brute doit contenir le jeton (sinon le test ne prouve rien)"
    );
    tracing::error!(%err, "appel direct");
    tracing::error!(?err, "appel direct debug");
    let t = text(&buf);
    assert!(t.contains("appel direct"));
    assert!(!t.contains("FAKE-TOKEN"), "jeton dans le journal : {t}");
}

#[test]
fn redact_motif_sans_jeton_connu() {
    let s = telemetry::redact(
        r#"url (https://api.telegram.org/bot999:AA-b_c/getMe) "x""#,
        "autre",
    );
    assert!(!s.contains("AA-b_c") && s.contains("/bot***/getMe"), "{s}");
}
