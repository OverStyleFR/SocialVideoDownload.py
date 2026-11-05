//! /settings de bout en bout : vrai bot (bot::run) sur faux Telegram, fichier de réglages réel.

#[allow(dead_code)]
mod fake_telegram;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{routing::get, Router};
use fake_telegram::{Call, FakeTelegram};
use socialvideodownload::bot::{self, AppState};
use socialvideodownload::config::Config;
use socialvideodownload::download::Downloader;

const ADMIN: u64 = 42;
const USER: u64 = 9;
const NEW: u64 = 7;

struct Bot {
    tg: FakeTelegram,
    h: tokio::task::JoinHandle<()>,
}

impl Drop for Bot {
    fn drop(&mut self) {
        self.h.abort();
    }
}

async fn start(dir: &Path) -> Bot {
    start_with(dir, None).await
}

/// `api` = faux serveur d'API X : remplace la base du téléchargeur (l'état garde ses réglages).
async fn start_with(dir: &Path, api: Option<url::Url>) -> Bot {
    let tg = FakeTelegram::start().await;
    let mut env: HashMap<String, String> = [
        ("BOT_TOKEN", "123:abc"),
        ("TELEGRAM_API_URL", tg.url.as_str()),
        ("ALLOWED_USER_IDS", "42,9"),
        ("ADMIN_USER_IDS", "42"),
        ("RATE_LIMIT_PER_MIN", "100"),
        ("DOWNLOAD_DIR", dir.join("dl").to_str().unwrap()),
        ("SETTINGS_FILE", settings_file(dir).to_str().unwrap()),
        ("YTDLP_PATH", "absent-ytdlp-wftfxzuk"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    if api.is_some() {
        env.insert("UNSAFE_ALLOW_PRIVATE_HOSTS".into(), "1".into());
        for (k, name) in [
            ("YTDLP_PATH", "fake-ytdlp"),
            ("FFMPEG_PATH", "fake-ffmpeg"),
            ("FFPROBE_PATH", "fake-ffprobe"),
        ] {
            env.insert(k.into(), tool(dir, name).display().to_string());
        }
    }
    let cfg = Arc::new(Config::from_map(&env).unwrap());
    let mut state = AppState::new(cfg);
    if let Some(api) = api {
        // Même Downloader (donc mêmes réglages branchés par AppState), seule la base d'API change.
        let s = Arc::get_mut(&mut state).unwrap();
        let dl = Arc::try_unwrap(std::mem::replace(
            &mut s.downloader,
            Arc::new(Downloader::new(s.config.clone())),
        ))
        .ok()
        .unwrap();
        s.downloader = Arc::new(dl.with_api_base(api));
    }
    let h = tokio::spawn(bot::run(state));
    Bot { tg, h }
}

fn settings_file(dir: &Path) -> PathBuf {
    dir.join("conf").join("settings.json")
}

/// Données de rappel des boutons d'un appel (reply_markup JSON).
fn callbacks(c: &Call) -> Vec<String> {
    let v: serde_json::Value = serde_json::from_str(&c.body).unwrap_or_default();
    v["reply_markup"]["inline_keyboard"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|row| row.as_array().cloned().unwrap_or_default())
        .filter_map(|b| b["callback_data"].as_str().map(String::from))
        .collect()
}

fn text_of(c: &Call) -> String {
    let v: serde_json::Value = serde_json::from_str(&c.body).unwrap_or_default();
    v["text"].as_str().unwrap_or_default().to_string()
}

fn to(chat: u64) -> impl Fn(&Call) -> bool {
    move |c| {
        (c.method == "sendmessage" || c.method == "editmessagetext")
            && c.body.contains(&format!("\"chat_id\":{chat}"))
    }
}

async fn wait_text(tg: &FakeTelegram, chat: u64, needle: &str) -> Call {
    let p = to(chat);
    tg.wait_for(10, |c| p(c) && text_of(c).contains(needle))
        .await
        .unwrap_or_else(|| {
            let seen: Vec<String> = tg.calls().iter().filter(|c| p(c)).map(text_of).collect();
            panic!("« {needle} » jamais reçu par {chat} ; reçu : {seen:?}")
        })
}

fn audit(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("conf").join("settings-audit.jsonl")).unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread")]
async fn admin_voit_les_7_rubriques() {
    let d = tempfile::tempdir().unwrap();
    let b = start(d.path()).await;
    b.tg.send_text(ADMIN, "/settings");
    let c = wait_text(&b.tg, ADMIN, "Réglages").await;
    let sections: Vec<String> = callbacks(&c)
        .into_iter()
        .filter(|d| d.starts_with("s:o:"))
        .collect();
    assert_eq!(sections.len(), 7, "{sections:?}");

    // Navigation : le clic ÉDITE le message (pas de nouveau message).
    b.tg.click(ADMIN, "s:o:acc");
    let e =
        b.tg.wait_for(10, |c| {
            c.method == "editmessagetext" && callbacks(c).len() > 1
        })
        .await
        .expect("rubrique Accès affichée par édition");
    assert!(callbacks(&e).contains(&"s:k:acc.rate_limit_per_min".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn non_admin_refuse_sans_menu() {
    let d = tempfile::tempdir().unwrap();
    let b = start(d.path()).await;
    b.tg.send_text(USER, "/settings");
    wait_text(&b.tg, USER, "réservée aux administrateurs").await;
    // Un bouton forgé par un non-admin ne change rien et n'ouvre aucun menu.
    b.tg.click(USER, "s:v:acc.rate_limit_per_min:1");
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    assert!(
        b.tg.calls().iter().all(|c| callbacks(c).is_empty()),
        "aucun clavier pour un non-admin"
    );
    assert!(audit(d.path()).is_empty(), "aucun changement audité");
    assert!(!settings_file(d.path()).exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn valeur_hors_borne_refusee_avec_message() {
    let d = tempfile::tempdir().unwrap();
    let b = start(d.path()).await;
    b.tg.click(ADMIN, "s:p:acc.rate_limit_per_min");
    wait_text(&b.tg, ADMIN, "Envoie la nouvelle valeur").await;
    b.tg.send_text(ADMIN, "5000");
    wait_text(&b.tg, ADMIN, "❌").await;
    // Bouton forgé hors borne : refusé aussi.
    b.tg.click(ADMIN, "s:v:acc.rate_limit_per_min:0");
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let errs =
        b.tg.calls()
            .iter()
            .filter(|c| to(ADMIN)(c) && text_of(c).contains("❌"))
            .count();
    assert_eq!(errs, 2, "deux refus affichés");
    assert!(audit(d.path()).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn limite_de_debit_appliquee_a_chaud_et_auditee() {
    let d = tempfile::tempdir().unwrap();
    let b = start(d.path()).await;
    b.tg.click(ADMIN, "s:v:acc.rate_limit_per_min:1");
    wait_text(&b.tg, ADMIN, "Valeur : 1 (").await;
    // Parcours suivant : la 2e demande de la minute est refusée.
    b.tg.send_text(USER, "https://1.1.1.1/v/a");
    wait_text(&b.tg, USER, "Téléchargement en cours").await;
    b.tg.send_text(USER, "https://1.1.1.1/v/b");
    wait_text(&b.tg, USER, "Trop de demandes").await;
    let a = audit(d.path());
    assert!(
        a.contains("acc.rate_limit_per_min") && a.contains("42"),
        "{a}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn valeur_relue_apres_redemarrage() {
    let d = tempfile::tempdir().unwrap();
    {
        let b = start(d.path()).await;
        b.tg.click(ADMIN, "s:v:acc.rate_limit_per_min:3");
        wait_text(&b.tg, ADMIN, "Valeur : 3 (").await;
    }
    let b = start(d.path()).await;
    b.tg.click(ADMIN, "s:k:acc.rate_limit_per_min");
    wait_text(&b.tg, ADMIN, "Valeur : 3 (").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn utilisateur_ajoute_par_id_peut_telecharger() {
    let d = tempfile::tempdir().unwrap();
    let b = start(d.path()).await;
    b.tg.send_text(NEW, "https://1.1.1.1/v/a");
    wait_text(&b.tg, NEW, "pas autorisé").await;
    b.tg.click(ADMIN, "s:p:acc.allowed_users");
    wait_text(&b.tg, ADMIN, "IDs Telegram").await;
    b.tg.send_text(ADMIN, "42,9,7");
    wait_text(&b.tg, ADMIN, "Valeur : 7,9,42").await;
    b.tg.send_text(NEW, "https://1.1.1.1/v/b");
    wait_text(&b.tg, NEW, "Téléchargement en cours").await;
}

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

/// Faux API X (réponse fxtwitter + petit mp4) sur un port libre.
async fn fake_api() -> url::Url {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!(
        "http://127.0.0.1:{}/",
        listener.local_addr().unwrap().port()
    );
    let media = format!(
        r#"{{"tweet":{{"text":"titre api","media":{{"videos":[{{"url":"{base}v.mp4"}}]}}}}}}"#
    );
    let app = Router::new()
        .route("/u/status/{id}", get(move || async move { media }))
        .route("/v.mp4", get(|| async { vec![7u8; 1024] }));
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    url::Url::parse(&base).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn admin_active_provider_x_puis_lien_servi_sans_ytdlp() {
    let d = tempfile::tempdir().unwrap();
    let b = start_with(d.path(), Some(fake_api().await)).await;
    let calls = d.path().join("calls.txt");

    // Écran Sources : TikTok désactivé par défaut, avec son avertissement.
    b.tg.click(ADMIN, "s:k:providers.tiktok.enabled");
    wait_text(&b.tg, ADMIN, "API publique tierce").await;
    wait_text(&b.tg, ADMIN, "Valeur : désactivé").await;

    // Provider X désactivé : le lien passe par yt-dlp (faux).
    b.tg.send_text(USER, "https://x.com/u/status/111");
    b.tg.wait_for(15, |c| c.method == "sendvideo")
        .await
        .expect("via yt-dlp");
    assert_eq!(
        std::fs::read_to_string(&calls).unwrap_or_default().trim(),
        "111"
    );

    // L'admin active X par le bouton, à chaud.
    b.tg.click(ADMIN, "s:v:providers.x.enabled:on");
    wait_text(&b.tg, ADMIN, "Valeur : activé").await;
    b.tg.send_text(USER, "https://x.com/u/status/222");
    b.tg.wait_for(15, |c| c.method == "sendvideo" && c.body.contains("222"))
        .await
        .expect("servi par le provider");
    assert!(
        !std::fs::read_to_string(&calls)
            .unwrap_or_default()
            .contains("222"),
        "yt-dlp ne doit pas être appelé pour le lien servi par le provider"
    );
}
