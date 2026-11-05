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

/// Les tests partagent le subscriber global : un à la fois, tampon vidé au départ.
static SERIE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Lance le bot sur `api` ; s'arrête dès que le journal contient `attendu`, que le dispatcher
/// finit (panique comprise) ou après 20 s. Rend le journal et l'issue du dispatcher.
async fn run_bot(api: &str, attendu: &str) -> (String, Result<(), tokio::task::JoinError>) {
    let buf = log();
    buf.lock().unwrap().clear();
    let h = tokio::spawn(bot::run(AppState::new(config(api))));
    for _ in 0..100 {
        if h.is_finished() || text(&buf).contains(attendu) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let fin = if h.is_finished() { h.await } else { Ok(()) };
    (text(&buf), fin)
}

#[tokio::test(flavor = "multi_thread")]
async fn connexion_refusee_ne_fuit_pas_le_jeton() {
    let _g = SERIE.lock().await;
    // Port fermé : teloxide panique dès getMe (stderr, hors tracing) ; la panique ne doit pas porter le jeton.
    let (t, fin) = run_bot("http://127.0.0.1:1", "JAMAIS").await;
    let msg = match fin {
        Err(e) if e.is_panic() => {
            let p = e.into_panic();
            p.downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default()
        }
        autre => panic!("panique du dispatcher attendue au getMe refusé : {autre:?}"),
    };
    assert!(!msg.is_empty());
    assert!(!t.contains("FAKE-TOKEN"), "jeton dans le journal : {t}");
    assert!(
        !msg.contains("FAKE-TOKEN"),
        "jeton dans le message de panique (stderr) : {msg}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn reponse_500_ne_fuit_pas_le_jeton() {
    use axum::{extract::Request, http::StatusCode, response::IntoResponse, Router};
    let _g = SERIE.lock().await;
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    // getWebhookInfo et getMe répondent 200 (le dispatcher démarre), getUpdates 500 en boucle.
    let app = Router::new().fallback(|r: Request| async move {
        let chemin = r.uri().path().to_lowercase();
        let ok = |v: serde_json::Value| axum::Json(serde_json::json!({"ok": true, "result": v}));
        if chemin.ends_with("/getwebhookinfo") {
            ok(
                serde_json::json!({"url": "", "has_custom_certificate": false,
                "pending_update_count": 0}),
            )
            .into_response()
        } else if chemin.ends_with("/getme") {
            ok(
                serde_json::json!({"id": 42, "is_bot": true, "first_name": "bot",
                "username": "test_bot", "can_join_groups": true,
                "can_read_all_group_messages": false, "supports_inline_queries": false,
                "can_connect_to_business": false, "has_main_web_app": false}),
            )
            .into_response()
        } else {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({"ok": false, "error_code": 500,
                    "description": "Internal Server Error"})),
            )
                .into_response()
        }
    });
    let srv = tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    let (t, fin) = run_bot(&url, "update listener").await;
    srv.abort();
    assert!(fin.is_ok(), "le dispatcher ne doit pas s'arrêter : {fin:?}");
    assert!(
        t.contains("update listener"),
        "aucune ligne d'erreur du dispatcher teloxide capturée : {t}"
    );
    assert!(!t.contains("FAKE-TOKEN"), "jeton dans le journal : {t}");
}

#[tokio::test(flavor = "multi_thread")]
async fn erreur_reqwest_directe_ne_fuit_pas_le_jeton() {
    let _g = SERIE.lock().await;
    let buf = log();
    buf.lock().unwrap().clear();
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
