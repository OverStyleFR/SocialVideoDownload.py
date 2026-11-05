//! Parcours de bout en bout : vrai bot (bot::run) sur faux Telegram, faux yt-dlp/ffmpeg/ffprobe.

mod fake_telegram;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use fake_telegram::{Call, FakeTelegram};
use socialvideodownload::bot::{self, AppState};
use socialvideodownload::config::Config;
use socialvideodownload::retention;

const ADMIN: u64 = 111;
const OTHER: u64 = 222;
const STRANGER: u64 = 999;

struct Env {
    tg: FakeTelegram,
    dir: tempfile::TempDir,
    bot: tokio::task::JoinHandle<()>,
}

impl Drop for Env {
    fn drop(&mut self) {
        self.bot.abort();
    }
}

impl Env {
    fn downloads(&self) -> PathBuf {
        self.dir.path().join("dl")
    }
    /// IDs passés au faux yt-dlp, dans l'ordre.
    fn ytdlp_calls(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.path().join("calls.txt"))
            .unwrap_or_default()
            .lines()
            .map(|l| l.trim().to_string())
            .collect()
    }
    /// Textes envoyés ou édités pour `chat`.
    fn texts(&self, chat: u64) -> Vec<String> {
        self.tg
            .calls()
            .into_iter()
            .filter(|c| c.method == "sendmessage" || c.method == "editmessagetext")
            .filter(|c| c.body.contains(&format!("\"chat_id\":{chat}")))
            .map(|c| c.body)
            .collect()
    }
}

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

async fn start(extra: &[(&str, &str)]) -> Env {
    let tg = FakeTelegram::start().await;
    let dir = tempfile::tempdir().unwrap();
    let dl = dir.path().join("dl");
    let mut env: HashMap<String, String> = [
        ("BOT_TOKEN", "123:abc"),
        ("TELEGRAM_API_URL", tg.url.as_str()),
        ("ALLOWED_USER_IDS", "111,222"),
        ("ADMIN_USER_IDS", "111"),
        ("UNSAFE_ALLOW_PRIVATE_HOSTS", "0"),
        ("RATE_LIMIT_PER_MIN", "100"),
        ("DOWNLOAD_DIR", dl.to_str().unwrap()),
    ]
    .into_iter()
    .chain(extra.iter().copied())
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    for (k, name) in [
        ("YTDLP_PATH", "fake-ytdlp"),
        ("FFMPEG_PATH", "fake-ffmpeg"),
        ("FFPROBE_PATH", "fake-ffprobe"),
    ] {
        env.insert(k.into(), tool(dir.path(), name).display().to_string());
    }
    let cfg = Arc::new(Config::from_map(&env).unwrap());
    let bot = tokio::spawn(bot::run(AppState::new(cfg)));
    Env { tg, dir, bot }
}

fn is_media(c: &Call, method: &str, chat: u64) -> bool {
    c.method == method && c.body.contains(&chat.to_string())
}

// (a) URL autorisée -> sendVideo reçu.
#[tokio::test(flavor = "multi_thread")]
async fn a_allowed_url_sends_video() {
    let e = start(&[]).await;
    e.tg.send_text(ADMIN, "regarde https://1.1.1.1/v/clip1");
    let c = e.tg.wait_for(15, |c| c.method == "sendvideo").await;
    assert!(c.expect("sendVideo attendu").body.contains("titre clip1"));
    assert_eq!(e.ytdlp_calls(), ["clip1"]);
}

// (b) 2 utilisateurs, le premier lent : le second reçoit sa vidéo avant la fin du premier.
#[tokio::test(flavor = "multi_thread")]
async fn b_slow_user_does_not_block_other() {
    let e = start(&[]).await;
    e.tg.send_text(ADMIN, "https://1.1.1.1/v/slow");
    tokio::time::sleep(Duration::from_millis(300)).await;
    e.tg.send_text(OTHER, "https://1.1.1.1/v/fast");
    e.tg.wait_for(20, |c| {
        c.method == "sendvideo" && c.body.contains("titre slow")
    })
    .await
    .expect("vidéo lente jamais reçue");
    let videos: Vec<String> = e.tg.of("sendvideo").into_iter().map(|c| c.body).collect();
    assert_eq!(videos.len(), 2, "{videos:?}");
    assert!(
        videos[0].contains("titre fast"),
        "le rapide doit arriver en premier"
    );
}

// (c) utilisateur hors liste -> refus, aucun yt-dlp.
#[tokio::test(flavor = "multi_thread")]
async fn c_stranger_refused_without_ytdlp() {
    let e = start(&[]).await;
    e.tg.send_text(STRANGER, "https://1.1.1.1/v/x");
    let c =
        e.tg.wait_for(10, |c| {
            c.method == "sendmessage" && c.body.contains("pas autorisé")
        })
        .await;
    assert!(c.is_some(), "message de refus attendu");
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(e.ytdlp_calls().is_empty());
    assert!(e.tg.of("sendvideo").is_empty());
}

// (d) cibles internes -> refus SSRF, aucun yt-dlp.
#[tokio::test(flavor = "multi_thread")]
async fn d_ssrf_refused() {
    let e = start(&[]).await;
    e.tg.send_text(ADMIN, "http://127.0.0.1/a");
    e.tg.send_text(OTHER, "http://169.254.169.254/latest/meta-data");
    for chat in [ADMIN, OTHER] {
        let found =
            e.tg.wait_for(10, |c| {
                c.body.contains(&format!("\"chat_id\":{chat}")) && c.body.contains("Ce lien")
            })
            .await;
        assert!(
            found.is_some(),
            "refus attendu pour {chat} : {:?}",
            e.texts(chat)
        );
    }
    assert!(e.ytdlp_calls().is_empty());
    assert!(e.tg.of("sendvideo").is_empty());
}

// (e) message ÉDITÉ contenant une URL -> traité, sans panique.
#[tokio::test(flavor = "multi_thread")]
async fn e_edited_message_is_processed() {
    let e = start(&[]).await;
    e.tg.send_edited(ADMIN, "corrigé : https://1.1.1.1/v/edited");
    let c = e.tg.wait_for(15, |c| c.method == "sendvideo").await;
    assert!(c.expect("sendVideo attendu").body.contains("titre edited"));
    assert!(!e.bot.is_finished());
}

fn consent_id(e: &Env) -> String {
    let body =
        e.tg.calls()
            .into_iter()
            .find(|c| c.body.contains("ext:y:"))
            .unwrap()
            .body;
    let rest = body.split("ext:y:").nth(1).unwrap();
    rest.split('"').next().unwrap().to_string()
}

// (f) trop gros + repli : consentement d'abord, envoi seulement après « Oui ».
#[tokio::test(flavor = "multi_thread")]
async fn f_oversize_needs_consent_then_uploads() {
    let tg_probe = FakeTelegram::start().await; // sert d'hébergeur externe
    let up = format!("{}/upload", tg_probe.url);
    let e = start(&[
        ("TELEGRAM_UPLOAD_LIMIT_MB", "0"),
        ("EXTERNAL_UPLOAD_URL", &up),
    ])
    .await;
    e.tg.send_text(ADMIN, "https://1.1.1.1/v/huge");
    e.tg.wait_for(15, |c| c.body.contains("ext:y:"))
        .await
        .expect("demande de consentement attendue");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        tg_probe.of("/upload").is_empty(),
        "aucun envoi avant le Oui"
    );
    assert!(e.tg.of("sendvideo").is_empty());
    e.tg.click(ADMIN, &format!("ext:y:{}", consent_id(&e)));
    e.tg.wait_for(15, |c| c.body.contains("https://files.example/abc"))
        .await
        .unwrap_or_else(|| panic!("lien externe attendu : {:?}", e.tg.calls()));
    assert_eq!(tg_probe.of("/upload").len(), 1);
}

// (f bis) le repli répond 500 -> message d'erreur, pas d'URL.
#[tokio::test(flavor = "multi_thread")]
async fn f_external_500_gives_error_without_link() {
    let host = FakeTelegram::start().await;
    let up = format!("{}/upload500", host.url);
    let e = start(&[
        ("TELEGRAM_UPLOAD_LIMIT_MB", "0"),
        ("EXTERNAL_UPLOAD_URL", &up),
    ])
    .await;
    e.tg.send_text(ADMIN, "https://1.1.1.1/v/huge");
    e.tg.wait_for(15, |c| c.body.contains("ext:y:"))
        .await
        .unwrap();
    let n = e.tg.calls().len();
    e.tg.click(ADMIN, &format!("ext:y:{}", consent_id(&e)));
    e.tg.wait_for(15, |c| c.method == "answercallbackquery")
        .await
        .unwrap();
    // Dernier texte : une erreur, jamais une URL.
    let mut last = None;
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let calls = e.tg.calls();
        if host.of("/upload500").len() == 1 && calls.len() > n + 2 {
            last = calls.last().cloned();
            break;
        }
    }
    let last = last.expect("message d'erreur attendu").body;
    assert!(!last.contains("https://"), "{last}");
    assert!(e.tg.of("sendvideo").is_empty());
}

// (g) /stats : refusé au non-admin, MarkdownV2 échappé pour l'admin.
#[tokio::test(flavor = "multi_thread")]
async fn g_stats_admin_only_and_escaped() {
    let e = start(&[]).await;
    let job = e.downloads().join("job1");
    std::fs::create_dir_all(&job).unwrap();
    // `*` est interdit dans un nom de fichier Windows : le cas complet est couvert sous Unix.
    let (name, escaped) = if cfg!(windows) {
        ("a_b[c].mp4", r"a\_b\[c\]\.mp4")
    } else {
        ("a_b*[c].mp4", r"a\_b\*\[c\]\.mp4")
    };
    std::fs::write(job.join(name), vec![0u8; 2048]).unwrap();
    e.tg.send_text(OTHER, "/stats");
    e.tg.send_text(ADMIN, "/stats");
    let admin =
        e.tg.wait_for(10, |c| c.body.contains("MarkdownV2"))
            .await
            .expect("stats admin");
    let v: serde_json::Value = serde_json::from_str(&admin.body).unwrap();
    assert_eq!(v["chat_id"], ADMIN);
    let text = v["text"].as_str().unwrap();
    assert!(text.contains(escaped), "{text}");
    assert!(markdown_v2_valid(text), "MarkdownV2 invalide : {text}");
    let other = e.texts(OTHER);
    assert_eq!(other.len(), 1, "{other:?}");
    assert!(!other[0].contains("MarkdownV2") && !other[0].contains("Statistiques"));
}

/// Tout caractère réservé hors d'un `*gras*` doit être précédé d'un antislash.
fn markdown_v2_valid(t: &str) -> bool {
    const R: &str = "_*[]()~`>#+-=|{}.!";
    let mut chars = t.chars();
    let mut bold = 0;
    while let Some(c) = chars.next() {
        if c == '\\' {
            if chars.next().is_none() {
                return false;
            }
        } else if c == '*' {
            bold += 1;
        } else if R.contains(c) {
            return false;
        }
    }
    bold % 2 == 0
}

// (h) /music -> sendAudio ; échec ffmpeg -> aucun mp3 laissé, message d'erreur.
#[tokio::test(flavor = "multi_thread")]
async fn h_music_sends_audio_and_failure_leaves_no_mp3() {
    let e = start(&[]).await;
    e.tg.send_text(ADMIN, "/music https://1.1.1.1/v/song");
    e.tg.wait_for(15, |c| is_media(c, "sendaudio", ADMIN))
        .await
        .expect("sendAudio attendu");
    e.tg.send_text(OTHER, "/music https://1.1.1.1/v/badmp3");
    e.tg.wait_for(15, |c| {
        c.body.contains(&format!("\"chat_id\":{OTHER}")) && c.body.contains("traitement du fichier")
    })
    .await
    .unwrap_or_else(|| panic!("erreur attendue : {:?}", e.texts(OTHER)));
    tokio::time::sleep(Duration::from_millis(300)).await;
    let mp3: Vec<_> = walk(&e.downloads())
        .into_iter()
        .filter(|p| p.to_string_lossy().contains(".mp3"))
        .collect();
    assert!(mp3.is_empty(), "mp3 restants : {mp3:?}");
    assert_eq!(e.tg.of("sendaudio").len(), 1);
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    for ent in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = ent.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

// (i) même URL deux fois -> second envoi par file_id, yt-dlp lancé une seule fois.
#[tokio::test(flavor = "multi_thread")]
async fn i_same_url_twice_uses_cache() {
    let e = start(&[]).await;
    e.tg.send_text(ADMIN, "https://1.1.1.1/v/dup");
    e.tg.wait_for(15, |c| c.method == "sendvideo")
        .await
        .unwrap();
    e.tg.send_text(OTHER, "https://1.1.1.1/v/dup#x");
    let second =
        e.tg.wait_for(15, |c| c.method == "sendvideo" && c.body.contains("FILE-"))
            .await
            .unwrap_or_else(|| {
                panic!(
                    "second envoi par file_id : {:?} {:?}",
                    e.tg.calls(),
                    e.ytdlp_calls()
                )
            });
    assert!(second.body.contains("FILE-"), "{}", second.body);
    assert_eq!(e.ytdlp_calls(), ["dup"]);
}

// (j) nettoyage pendant un téléchargement : le fichier en cours survit.
#[tokio::test(flavor = "multi_thread")]
async fn j_cleanup_spares_running_job() {
    let e = start(&[]).await;
    e.tg.send_text(ADMIN, "https://1.1.1.1/v/slow");
    for _ in 0..100 {
        if !e.ytdlp_calls().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    let report = retention::cleanup(&e.downloads(), Duration::ZERO, u64::MAX / 2)
        .await
        .unwrap();
    assert_eq!(report.skipped_locked, 1, "{report:?}");
    assert_eq!(report.removed, 0, "{report:?}");
    let c = e.tg.wait_for(20, |c| c.method == "sendvideo").await;
    assert!(
        c.is_some(),
        "vidéo perdue après nettoyage : {:?}",
        e.texts(ADMIN)
    );
}

// (k) toute erreur produit un message : yt-dlp en panne -> message d'erreur.
#[tokio::test(flavor = "multi_thread")]
async fn k_errors_are_never_silent() {
    let e = start(&[]).await;
    e.tg.send_text(ADMIN, "https://1.1.1.1/v/fail");
    e.tg.wait_for(15, |c| c.method == "sendmessage")
        .await
        .unwrap();
    // Le message de progression doit finir sur une erreur, pas sur « Téléchargement… ».
    let mut ok = false;
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let t = e.texts(ADMIN);
        if e.ytdlp_calls() == ["fail"]
            && t.last()
                .is_some_and(|b| !b.contains('⏳') && !b.contains('⏬'))
        {
            ok = true;
            break;
        }
    }
    assert!(ok, "aucun message d'erreur final : {:?}", e.texts(ADMIN));
    assert!(e.tg.of("sendvideo").is_empty());
}
