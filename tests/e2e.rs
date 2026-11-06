//! Parcours de bout en bout : vrai bot (bot::run) sur faux Telegram, faux yt-dlp/ffmpeg/ffprobe.

mod fake_telegram;
#[allow(dead_code)]
mod fixtures;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use fake_telegram::{Call, FakeTelegram};
use fixtures::tool;
use socialvideodownload::bot::{self, AppState};
use socialvideodownload::config::Config;
use socialvideodownload::retention;

const ADMIN: u64 = 111;
const OTHER: u64 = 222;
const STRANGER: u64 = 999;

struct Env {
    tg: FakeTelegram,
    dir: tempfile::TempDir,
    bot: tokio::task::JoinHandle<bool>,
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
        // Jamais le vrai curl.libriciel.fr depuis les tests : chaque test f le remplace.
        ("EXTERNAL_UPLOAD_URL", "off"),
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

// Lu dans le champ chat_id : un `contains("111")` sur tout le corps multipart prenait parfois la
// frontière aléatoire (64 caractères hexadécimaux) pour l'identifiant du chat.
fn is_media(c: &Call, method: &str, chat: u64) -> bool {
    c.method == method && fake_telegram::chat_of(&c.body) == chat
}

#[test]
fn is_media_lit_le_champ_chat_id() {
    let body =
        "--a1117f\r\nContent-Disposition: form-data; name=\"chat_id\"\r\n\r\n222\r\n--a1117f--\r\n";
    let c = Call {
        method: "sendvideo".into(),
        body: body.into(),
    };
    assert!(!is_media(&c, "sendvideo", ADMIN), "« 111 » de la frontière");
    assert!(is_media(&c, "sendvideo", OTHER));
}

// (a) URL autorisée -> sendVideo reçu.
#[tokio::test(flavor = "multi_thread")]
async fn a_allowed_url_sends_video() {
    let e = start(&[]).await;
    e.tg.send_text(ADMIN, "regarde https://1.1.1.1/v/clip1");
    let c = e.tg.wait_for(15, |c| c.method == "sendvideo").await;
    let c = c.expect("sendVideo attendu");
    // Envoyé en réponse au message, sans légende (comme le bot Python).
    assert!(c.body.contains("reply_parameters") && !c.body.contains("titre clip1"));
    // Nom affiché tiré du titre, comme `%(title)s.%(ext)s` du bot Python.
    assert!(
        c.body.contains("filename=\"titre_clip1.mp4\""),
        "{}",
        c.body
    );
    assert_eq!(e.ytdlp_calls(), ["clip1"]);
}

// (b) 2 utilisateurs, le premier lent : le second reçoit sa vidéo avant la fin du premier.
#[tokio::test(flavor = "multi_thread")]
async fn b_slow_user_does_not_block_other() {
    let e = start(&[]).await;
    e.tg.send_text(ADMIN, "https://1.1.1.1/v/slow");
    tokio::time::sleep(Duration::from_millis(300)).await;
    e.tg.send_text(OTHER, "https://1.1.1.1/v/fast");
    e.tg.wait_for(20, |c| is_media(c, "sendvideo", ADMIN))
        .await
        .expect("vidéo lente jamais reçue");
    let chats: Vec<u64> =
        e.tg.of("sendvideo")
            .iter()
            .map(|c| fake_telegram::chat_of(&c.body))
            .collect();
    assert_eq!(chats, [OTHER, ADMIN], "le rapide doit arriver en premier");
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
    assert!(c
        .expect("sendVideo attendu")
        .body
        .contains("reply_parameters"));
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

// (f0) EXTERNAL_UPLOAD_CONSENT=0 : envoi externe direct comme le bot Python
// (PUT <base>/<sha256>.mp4, lien en réponse).
#[tokio::test(flavor = "multi_thread")]
async fn f0_oversize_goes_external_like_python() {
    let host = FakeTelegram::start().await; // sert d'hébergeur externe
    let up = format!("{}/upload", host.url);
    let e = start(&[
        ("TELEGRAM_UPLOAD_LIMIT_MB", "0"),
        ("EXTERNAL_UPLOAD_URL", &up),
        ("EXTERNAL_UPLOAD_CONSENT", "0"),
    ])
    .await;
    e.tg.send_text(ADMIN, "https://1.1.1.1/v/huge");
    let done =
        e.tg.wait_for(15, |c| {
            c.method == "sendmessage"
                && c.body
                    .contains("Veuillez le télécharger ici : https://files.example/abc")
        })
        .await
        .unwrap_or_else(|| panic!("lien externe attendu : {:?}", e.texts(ADMIN)));
    assert!(done
        .body
        .contains("Le fichier est trop volumineux pour être envoyé directement par Telegram."));
    let texts = e.texts(ADMIN).join("\n");
    assert!(texts.contains("Upload externe en cours : 0% ⏳"), "{texts}");
    assert!(!texts.contains("ext:y:"), "aucun consentement demandé");
    assert!(
        !e.tg.of("deletemessage").is_empty(),
        "progression supprimée"
    );
    let puts = host.of("/upload");
    assert_eq!(puts.len(), 1);
    let name = puts[0]
        .body
        .lines()
        .next()
        .unwrap()
        .trim_start_matches("/upload/");
    assert!(
        name.len() == 68
            && name.ends_with(".mp4")
            && name[..64].bytes().all(|b| b.is_ascii_hexdigit()),
        "nom par sha256 attendu : {name}"
    );
    assert!(e.tg.of("sendvideo").is_empty());
}

// (f0 bis) même lien redemandé : le lien externe vient du cache, pas de second envoi.
#[tokio::test(flavor = "multi_thread")]
async fn f0_lien_externe_en_cache() {
    let host = FakeTelegram::start().await;
    let up = format!("{}/upload", host.url);
    let e = start(&[
        ("TELEGRAM_UPLOAD_LIMIT_MB", "0"),
        ("EXTERNAL_UPLOAD_URL", &up),
        ("EXTERNAL_UPLOAD_CONSENT", "0"),
    ])
    .await;
    let link = |c: &Call| c.method == "sendmessage" && c.body.contains("https://files.example/abc");
    e.tg.send_text(ADMIN, "https://1.1.1.1/v/huge");
    e.tg.wait_for(15, link).await.expect("1er lien attendu");
    e.tg.send_text(ADMIN, "https://1.1.1.1/v/huge");
    e.tg.wait_for(15, |_| e.tg.calls().iter().filter(|c| link(c)).count() >= 2)
        .await
        .unwrap_or_else(|| panic!("2e lien attendu : {:?}", e.texts(ADMIN)));
    assert_eq!(host.of("/upload").len(), 1, "un seul envoi externe");
}

// (f) par défaut : consentement d'abord, envoi seulement après « Oui ».
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
    let ask =
        e.tg.wait_for(15, |c| c.body.contains("ext:y:"))
            .await
            .expect("demande de consentement attendue");
    assert!(
        ask.body.contains("(127.0.0.1)"),
        "l'hôte nommé : {}",
        ask.body
    );
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

// (f ter) « Non » : rien ne part ; puis « Accord permanent » activé dans /settings : plus de question.
#[tokio::test(flavor = "multi_thread")]
async fn f_refus_puis_accord_permanent() {
    let host = FakeTelegram::start().await;
    let up = format!("{}/upload", host.url);
    let e = start(&[
        ("TELEGRAM_UPLOAD_LIMIT_MB", "0"),
        ("EXTERNAL_UPLOAD_URL", &up),
    ])
    .await;
    e.tg.send_text(ADMIN, "https://1.1.1.1/v/huge");
    e.tg.wait_for(15, |c| c.body.contains("ext:y:"))
        .await
        .expect("demande de consentement attendue");
    e.tg.click(ADMIN, &format!("ext:n:{}", consent_id(&e)));
    e.tg.wait_for(15, |c| c.body.contains("Envoi annulé."))
        .await
        .expect("refus confirmé");
    assert!(host.of("/upload").is_empty(), "rien envoyé après Non");

    e.tg.click(ADMIN, "s:v:send.external_always_yes:on");
    e.tg.wait_for(15, |c| c.body.contains("✅ Enregistré."))
        .await
        .expect("accord permanent enregistré");
    e.tg.send_text(ADMIN, "https://1.1.1.1/v/huge2");
    e.tg.wait_for(15, |c| c.body.contains("https://files.example/abc"))
        .await
        .unwrap_or_else(|| panic!("envoi direct attendu : {:?}", e.texts(ADMIN)));
    assert_eq!(host.of("/upload").len(), 1);
    let asks =
        e.tg.calls()
            .iter()
            .filter(|c| c.body.contains("ext:y:"))
            .count();
    assert_eq!(asks, 1, "une seule question, celle d'avant l'accord");
}

// (f bis) le repli répond 500 : 3 essais puis le message d'erreur d'origine, jamais de lien.
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
    e.tg.click(ADMIN, &format!("ext:y:{}", consent_id(&e)));
    e.tg.wait_for(15, |c| {
        c.method == "sendmessage"
            && c.body
                .contains("Erreur lors de l'upload externe du fichier.")
    })
    .await
    .unwrap_or_else(|| panic!("message d'erreur attendu : {:?}", e.texts(ADMIN)));
    assert_eq!(
        host.of("/upload500").len(),
        3,
        "3 essais comme curl_uploader.py"
    );
    assert!(!e.texts(ADMIN).join("\n").contains("files.example"));
    assert!(e.tg.of("sendvideo").is_empty());
}

// (g) /stats : refusé au non-admin avec le texte d'origine, mise en page du bot Python pour l'admin.
#[tokio::test(flavor = "multi_thread")]
async fn g_stats_admin_only_and_escaped() {
    let e = start(&[]).await;
    let job = e.downloads().join("job1");
    std::fs::create_dir_all(&job).unwrap();
    // `*` est interdit dans un nom de fichier Windows : le cas complet est couvert sous Unix.
    let (name, escaped) = if cfg!(windows) {
        ("a_b[c].mp4", r"a\_b\[c].mp4")
    } else {
        ("a_b*[c].mp4", r"a\_b\*\[c].mp4")
    };
    std::fs::write(job.join(name), vec![0u8; 2048]).unwrap();
    e.tg.send_text(OTHER, "/stats");
    e.tg.send_text(ADMIN, "/stats");
    let admin =
        e.tg.wait_for(10, |c| c.body.contains("Statistiques du Bot"))
            .await
            .expect("stats admin");
    let v: serde_json::Value = serde_json::from_str(&admin.body).unwrap();
    assert_eq!(v["chat_id"], ADMIN);
    assert_eq!(
        v["parse_mode"], "Markdown",
        "Markdown historique comme le bot Python"
    );
    let text = v["text"].as_str().unwrap();
    for rubrique in [
        "🗂️ *Cache (session)*",
        "💾 *Disque*",
        "⏳ *Rétention*",
        "📝 *Logs*",
        "🖥️ *Système*",
        "`Prochain:` ",
    ] {
        assert!(text.contains(rubrique), "{rubrique} absent : {text}");
    }
    assert!(text.contains(escaped), "{text}");
    let other = e.texts(OTHER);
    assert!(
        other.len() == 1
            && other[0].contains("Accès refusé. Cette commande est réservée au développeur."),
        "{other:?}"
    );
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
        c.body.contains(&format!("\"chat_id\":{OTHER}"))
            && c.body.contains("Erreur lors de la conversion en audio")
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
        if e.ytdlp_calls() == ["fail", "fail", "fail"]
            && t.last().is_some_and(|b| {
                !b.contains('⏳')
                    && b.contains("❌ Échec du téléchargement après plusieurs tentatives.")
            })
        {
            ok = true;
            break;
        }
    }
    assert!(ok, "aucun message d'erreur final : {:?}", e.texts(ADMIN));
    assert!(e.tg.of("sendvideo").is_empty());
}

// (k2) erreur définitive (lien non pris en charge) : une seule tentative et le message précis,
// au lieu de 3 essais et d'un « après plusieurs tentatives » qui le masquait.
#[tokio::test(flavor = "multi_thread")]
async fn k2_erreur_definitive_sans_nouvelle_tentative() {
    let e = start(&[]).await;
    e.tg.send_text(ADMIN, "https://1.1.1.1/v/unsup");
    e.tg.wait_for(15, |c| c.body.contains("pas pris en charge"))
        .await
        .expect("message précis");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(e.ytdlp_calls(), ["unsup"]);
}

// (n) Parité avec le bot Python : menu des commandes, messages sans lien, commande inconnue ignorée,
// message de cache et progression en réponse.
#[tokio::test(flavor = "multi_thread")]
async fn n_parite_flux_du_bot_python() {
    let e = start(&[]).await;
    e.tg.send_text(ADMIN, "/download");
    e.tg.send_text(ADMIN, "/music");
    e.tg.send_text(ADMIN, "/inconnue https://1.1.1.1/v/zzz");
    e.tg.wait_for(10, |c| c.body.contains("Ex: /music <LIEN>"))
        .await
        .expect("message /music sans lien");
    let t = e.texts(ADMIN).join("\n");
    assert!(t.contains("Veuillez fournir un lien pour le téléchargement. Ex: /download <LIEN>"));
    assert!(t.contains("Veuillez fournir un lien pour télécharger l'audio. Ex: /music <LIEN>"));
    assert!(
        e.ytdlp_calls().is_empty(),
        "une commande inconnue ne télécharge rien"
    );
    let menu = e.tg.of("setmycommands");
    assert!(
        menu.iter().any(|c| c.body.contains("Pour commencer")
            && c.body.contains("Pour obtenir de l'aide")
            && c.body.contains("Télécharger une vidéo avec yt-dlp")
            && c.body.contains("Télécharger de la musique avec yt-dlp")),
        "{menu:?}"
    );

    e.tg.send_text(ADMIN, "https://1.1.1.1/v/cached");
    e.tg.wait_for(15, |c| c.method == "sendvideo")
        .await
        .unwrap();
    e.tg.send_text(ADMIN, "https://1.1.1.1/v/cached");
    e.tg.wait_for(15, |c| {
        c.method == "sendvideo" && c.body.contains("Envoyé depuis le cache")
    })
    .await
    .expect("légende du cache attendue");
    assert_eq!(
        e.ytdlp_calls(),
        ["cached"],
        "le second envoi vient du cache"
    );
}

// (o) Token refusé par Telegram (401) : arrêt propre, sans la panique de teloxide
// (« Couldn't prepare dispatching context: Api(InvalidToken) », vue au lancement réel en Docker).
#[tokio::test(flavor = "multi_thread")]
async fn o_token_refuse_arret_propre() {
    let mut e = start(&[("BOT_TOKEN", "111111111:REFUSE")]).await;
    let fin = tokio::time::timeout(Duration::from_secs(10), &mut e.bot).await;
    assert!(
        matches!(fin, Ok(Ok(false))),
        "arrêt propre attendu : {fin:?}"
    );
}
