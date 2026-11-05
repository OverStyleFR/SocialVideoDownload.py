//! Gestionnaires des commandes, messages (édités compris) et boutons Telegram.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use teloxide::prelude::*;
use teloxide::types::{ChatId, InlineKeyboardButton, InlineKeyboardMarkup, InputFile, MessageId};
use teloxide::utils::command::BotCommands;

use super::stats::stats_text;
use super::AppState;
use crate::download::Mode;
use crate::error::{AccessError, AppError, DownloadError, EgressError, MediaError};
use crate::providers::{self, Platform};
use crate::retention::JobDir;
use crate::settings::{self, menu, Action};
use crate::{egress, media, url_guard};

/// Durée de vie d'un envoi en attente de consentement.
const CONSENT_TTL: Duration = Duration::from_secs(600);
const START_TEXT: &str = "Salut ! Envoie-moi un lien (YouTube, TikTok, Instagram, X…) et je te renvoie la vidéo. /music <lien> pour extraire l'audio, /help pour l'aide.";

/// Commandes du bot.
#[derive(BotCommands, Clone, Debug, PartialEq, Eq)]
#[command(rename_rule = "lowercase", description = "Commandes disponibles :")]
pub enum Command {
    /// Accueil.
    #[command(description = "démarrer")]
    Start,
    /// Aide.
    #[command(description = "afficher l'aide")]
    Help,
    /// Télécharger une vidéo.
    #[command(description = "télécharger une vidéo : /download <lien>")]
    Download(String),
    /// Extraire l'audio en MP3.
    #[command(description = "extraire la musique : /music <lien>")]
    Music(String),
    /// Statistiques (admins).
    #[command(description = "statistiques (admins)")]
    Stats,
    /// Réglages (admins).
    #[command(description = "réglages (admins)")]
    Settings,
}

/// Fichier trop gros pour Telegram, gardé le temps que l'utilisateur réponde.
#[derive(Debug)]
pub struct Pending {
    user: u64,
    path: std::path::PathBuf,
    _job: JobDir,
}

/// Route de départ d'une demande une fois l'accès accordé.
#[derive(Debug, PartialEq, Eq)]
pub enum Route {
    /// Le cache connaît ce média : renvoyer ce file_id.
    CacheHit(String),
    /// Il faut télécharger.
    Fetch,
}

/// Que faire d'un fichier final selon la limite de Telegram.
#[derive(Debug, PartialEq, Eq)]
pub enum Fit {
    Send,
    /// Demander le consentement avant d'envoyer chez le tiers.
    AskConsent {
        size_mb: u64,
    },
    TooBig,
}

/// Premier garde : accès puis débit (un refusé ne consomme pas de débit).
pub fn admit(
    allowed: bool,
    rate: impl FnOnce() -> Result<(), AccessError>,
) -> Result<(), AccessError> {
    if !allowed {
        return Err(AccessError::NotAllowed);
    }
    rate()
}

/// Cache d'abord.
pub fn route(cached: Option<String>) -> Route {
    cached.map_or(Route::Fetch, Route::CacheHit)
}

/// Décide l'envoi direct, la demande de consentement ou le refus.
pub fn fit(size: u64, limit: u64, external_configured: bool) -> Fit {
    if size <= limit {
        Fit::Send
    } else if external_configured {
        Fit::AskConsent {
            size_mb: size.div_ceil(1024 * 1024),
        }
    } else {
        Fit::TooBig
    }
}

/// Erreur d'une tâche : message pour l'utilisateur + détail pour les journaux.
struct Failure {
    user: String,
    detail: String,
}

fn app(e: impl Into<AppError>) -> Failure {
    let e = e.into();
    Failure {
        user: e.user_message(),
        detail: e.to_string(),
    }
}

fn tg(e: teloxide::RequestError) -> Failure {
    Failure {
        user: "L'envoi vers Telegram a échoué, réessaie plus tard.".into(),
        detail: e.to_string(),
    }
}

fn internal(what: &str) -> Failure {
    Failure {
        user: "Erreur interne, réessaie plus tard.".into(),
        detail: what.into(),
    }
}

/// Contexte d'une tâche : message de progression édité en place.
struct Cx {
    bot: Bot,
    state: Arc<AppState>,
    chat: ChatId,
    user: u64,
    platform: Platform,
    progress: Option<MessageId>,
}

impl Cx {
    async fn status(&mut self, text: &str) {
        let r = match self.progress {
            Some(id) => self
                .bot
                .edit_message_text(self.chat, id, text)
                .await
                .map(|_| ()),
            None => self
                .bot
                .send_message(self.chat, text)
                .await
                .map(|m| self.progress = Some(m.id)),
        };
        if let Err(e) = r {
            let err = self.redact(e.to_string());
            tracing::warn!(user_id = self.user, error = %err, "message de progression non envoyé");
        }
    }

    /// Retire le token de tout texte destiné aux journaux (les erreurs reqwest contiennent l'URL).
    fn redact(&self, s: String) -> String {
        s.replace(&self.state.config.bot_token, "***")
    }

    async fn clear(&mut self) {
        if let Some(id) = self.progress.take() {
            let _ = self.bot.delete_message(self.chat, id).await;
        }
    }

    /// Jamais de silence : l'erreur finit chez l'utilisateur.
    async fn fail(&mut self, f: Failure) {
        let err = self.redact(f.detail);
        tracing::warn!(user_id = self.user, platform = ?self.platform, error = %err, "tâche en échec");
        self.status(&f.user).await;
    }

    async fn send_file(
        &self,
        input: InputFile,
        mode: Mode,
        caption: &str,
    ) -> Result<String, Failure> {
        let caption: String = caption.chars().take(200).collect();
        let id = match mode {
            Mode::Video => {
                let m = self
                    .bot
                    .send_video(self.chat, input)
                    .caption(caption)
                    .await
                    .map_err(tg)?;
                m.video().map(|v| v.file.id.0.clone())
            }
            Mode::Audio => {
                let m = self
                    .bot
                    .send_audio(self.chat, input)
                    .caption(caption)
                    .await
                    .map_err(tg)?;
                m.audio().map(|a| a.file.id.0.clone())
            }
        };
        id.ok_or_else(|| internal("réponse Telegram sans média"))
    }
}

static JOB_SEQ: AtomicU64 = AtomicU64::new(0);

fn job_id(user: u64) -> String {
    format!(
        "{user}-{}-{}",
        std::process::id(),
        JOB_SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

/// Lance une tâche indépendante : un téléchargement ne bloque jamais le dispatcher.
fn spawn_job(bot: Bot, state: Arc<AppState>, chat: ChatId, user: u64, raw: String, mode: Mode) {
    tokio::spawn(async move {
        let mut cx = Cx {
            bot,
            state,
            chat,
            user,
            platform: Platform::Generic,
            progress: None,
        };
        let t0 = std::time::Instant::now();
        let res = job(&mut cx, &raw, mode).await;
        let source = format!("{:?}", cx.platform);
        let secs = t0.elapsed().as_secs_f64();
        match res {
            Ok(()) => cx.state.metrics.record(&source, secs, None),
            Err(f) => {
                cx.state.metrics.record(&source, secs, Some(&f.user));
                cx.fail(f).await;
            }
        }
    });
}

async fn job(cx: &mut Cx, raw: &str, mode: Mode) -> Result<(), Failure> {
    let state = cx.state.clone();
    let cfg = &state.config;
    admit(state.access.is_allowed(cx.user), || {
        state.access.check_rate(cx.user)
    })
    .map_err(app)?;
    cx.status("⏳ Vérification du lien…").await;
    let url = url_guard::validate(raw, cfg.unsafe_allow_private_hosts)
        .await
        .map_err(app)?;
    cx.platform = providers::detect(&url);

    if let Route::CacheHit(file_id) = route(state.cache.get(&url, mode)) {
        match cx
            .send_file(InputFile::file_id(file_id.into()), mode, "")
            .await
        {
            Ok(_) => {
                cx.clear().await;
                return Ok(());
            }
            Err(f) => {
                tracing::warn!(user_id = cx.user, error = %f.detail, "file_id du cache refusé, retéléchargement");
            }
        }
    }

    cx.status("⏳ En attente d'une place…").await;
    let _permit = state
        .semaphore
        .acquire()
        .await
        .map_err(|_| internal("sémaphore fermé"))?;
    cx.status("⏬ Téléchargement…").await;
    let job = JobDir::create(&cfg.download_dir, &job_id(cx.user))
        .map_err(|e| app(DownloadError::Io(e)))?;
    let dl = state
        .downloader
        .fetch(&url, mode, &job)
        .await
        .map_err(app)?;

    let limit = state
        .settings
        .get_u64("send.telegram_limit_mb")
        .map_or(cfg.upload_limit_bytes(), |mb| mb * 1024 * 1024);
    let external = cfg.external_upload_url.is_some()
        && state
            .settings
            .get_bool("send.external_fallback")
            .unwrap_or(true);
    let path = match mode {
        Mode::Video => {
            if let Some(issue) = media::audio_issue(cfg, &dl.path).await.map_err(app)? {
                let what = match issue {
                    media::AudioIssue::NoAudioTrack => "n'a pas de piste audio",
                    media::AudioIssue::SilentTrack => "a une piste audio muette",
                };
                let _ = cx
                    .bot
                    .send_message(cx.chat, format!("⚠️ Cette vidéo {what} à la source."))
                    .await;
            }
            cx.status("🗜️ Préparation du fichier…").await;
            match media::compress_under(cfg, &dl.path, limit).await {
                Ok(p) => p,
                Err(MediaError::CannotFit { .. }) => dl.path.clone(),
                Err(e) => return Err(app(e)),
            }
        }
        Mode::Audio => {
            cx.status("🎵 Conversion en MP3…").await;
            let dst = dl.path.with_file_name(format!("{}.audio.mp3", job.id()));
            media::to_mp3_atomic(cfg, &dl.path, &dst)
                .await
                .map_err(app)?
        }
    };

    let size = tokio::fs::metadata(&path)
        .await
        .map_err(|e| app(MediaError::Io(e)))?
        .len();
    match fit(size, limit, external) {
        Fit::TooBig => Err(app(EgressError::Disabled)),
        Fit::AskConsent { size_mb } => {
            let id = job.id().to_string();
            let kb = InlineKeyboardMarkup::new([[
                InlineKeyboardButton::callback("Oui", format!("ext:y:{id}")),
                InlineKeyboardButton::callback("Non", format!("ext:n:{id}")),
            ]]);
            state
                .pending
                .lock()
                .map_err(|_| internal("verrou pending"))?
                .insert(
                    id.clone(),
                    Pending {
                        user: cx.user,
                        path,
                        _job: job,
                    },
                );
            let st = state.clone();
            tokio::spawn(async move {
                tokio::time::sleep(CONSENT_TTL).await;
                if let Ok(mut p) = st.pending.lock() {
                    p.remove(&id);
                }
            });
            cx.clear().await;
            cx.bot
                .send_message(cx.chat, egress::consent_text(size_mb))
                .reply_markup(kb)
                .await
                .map_err(tg)?;
            Ok(())
        }
        Fit::Send => {
            cx.status("📤 Envoi…").await;
            let file_id = cx.send_file(InputFile::file(path), mode, &dl.title).await?;
            state.cache.put(&url, mode, file_id);
            cx.clear().await;
            Ok(())
        }
    }
}

fn not_admin() -> String {
    AppError::from(AccessError::NotAdmin).user_message()
}

/// Applique une valeur venue d'un bouton ou d'une saisie (revalidée par le store) et rend l'écran
/// du réglage, précédé de l'erreur de validation s'il y en a une.
fn apply_setting(
    state: &AppState,
    key: &'static str,
    raw: &str,
    user: u64,
) -> (String, InlineKeyboardMarkup) {
    let res = state.settings.set(key, raw, user);
    let (text, kb) = menu::item(key, &state.settings.snapshot()).unwrap_or_else(menu::root);
    let head = match res {
        Ok(_) => "✅ Enregistré.".to_string(),
        Err(e) => format!("❌ {e}"),
    };
    (format!("{head}\n\n{text}"), kb)
}

/// Boutons `s:` du menu /settings : l'admin est revérifié (par ID) à CHAQUE clic, et le message
/// du menu est édité en place.
async fn settings_callback(
    bot: &Bot,
    q: &CallbackQuery,
    data: &str,
    state: &AppState,
) -> ResponseResult<()> {
    let user = q.from.id.0;
    if !state.access.is_admin(user) {
        tracing::warn!(
            user_id = user,
            "bouton /settings refusé : pas administrateur"
        );
        return Ok(());
    }
    let (Some(msg), Some(action)) = (q.regular_message(), settings::parse_callback(data)) else {
        return Ok(());
    };
    let (chat, mid) = (msg.chat.id, msg.id);
    if let Ok(mut p) = state.prompts.lock() {
        p.remove(&user);
    }
    let values = state.settings.snapshot();
    let screen = match action {
        Action::Back => Some(menu::root()),
        Action::OpenSection(s) => Some(menu::section(s, &values)),
        Action::OpenKey(k) => menu::item(k, &values),
        Action::Set { key, value } => Some(apply_setting(state, key, &value, user)),
        Action::Prompt(key) => {
            if let Ok(mut p) = state.prompts.lock() {
                p.insert(user, (key, chat));
            }
            menu::prompt_text(key).map(|t| {
                let back = InlineKeyboardButton::callback("⬅️ Annuler", format!("s:k:{key}"));
                (t, InlineKeyboardMarkup::new([[back]]))
            })
        }
    };
    if let Some((text, kb)) = screen {
        bot.edit_message_text(chat, mid, text)
            .reply_markup(kb)
            .await?;
    }
    Ok(())
}

/// Boutons : menu /settings (`s:`) ou consentement d'envoi externe (`ext:`).
/// Pour `ext:`, envoi chez le tiers seulement après « Oui ».
pub async fn handle_callback(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<AppState>,
) -> ResponseResult<()> {
    bot.answer_callback_query(q.id.clone()).await?;
    if let Some(data) = q.data.as_deref().filter(|d| d.starts_with("s:")) {
        return settings_callback(&bot, &q, data, &state).await;
    }
    let user = q.from.id.0;
    let Some(rest) = q.data.as_deref().and_then(|d| d.strip_prefix("ext:")) else {
        return Ok(());
    };
    let Some((answer, id)) = rest.split_once(':') else {
        return Ok(());
    };
    let Some(msg) = q.regular_message() else {
        return Ok(());
    };
    let (chat, mid) = (msg.chat.id, msg.id);
    let taken = {
        let mut p = state.pending.lock().expect("pending");
        if p.get(id).is_some_and(|x| x.user == user) {
            p.remove(id)
        } else {
            None
        }
    };
    let Some(pending) = taken else {
        bot.edit_message_text(chat, mid, "Cette demande a expiré.")
            .await?;
        return Ok(());
    };
    if answer != "y" {
        bot.edit_message_text(chat, mid, "Envoi annulé.").await?;
        return Ok(());
    }
    tokio::spawn(async move {
        // Capture tout `pending` : sinon seul `path` est capturé et le JobDir (le fichier) est supprimé.
        let pending = pending;
        let mut cx = Cx {
            bot,
            state: state.clone(),
            chat,
            user,
            platform: Platform::Generic,
            progress: Some(mid),
        };
        cx.status("📤 Envoi vers l'hébergement externe…").await;
        let _permit = state.semaphore.acquire().await;
        match egress::upload_external(&state.config, &pending.path).await {
            Ok(link) => cx.status(&format!("Voilà ton fichier : {link}")).await,
            Err(e) => cx.fail(app(e)).await,
        }
    });
    Ok(())
}

/// Traite une commande reconnue.
pub async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Command,
    state: Arc<AppState>,
) -> ResponseResult<()> {
    let Some(user) = msg.from.as_ref().map(|u| u.id.0) else {
        return Ok(());
    };
    let chat = msg.chat.id;
    match cmd {
        Command::Start => {
            bot.send_message(chat, START_TEXT).await?;
        }
        Command::Help => {
            bot.send_message(chat, Command::descriptions().to_string())
                .await?;
        }
        Command::Stats => {
            // Réservé aux admins, jugé sur l'ID numérique (jamais sur le pseudo).
            if state.access.is_admin(user) {
                bot.send_message(chat, stats_text(&state).await)
                    .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                    .await?;
            } else {
                bot.send_message(chat, not_admin()).await?;
            }
        }
        Command::Settings => {
            // Même règle que /stats : ID numérique, jamais le pseudo ; refus sans aucun menu.
            if state.access.is_admin(user) {
                let (text, kb) = menu::root();
                bot.send_message(chat, text).reply_markup(kb).await?;
            } else {
                bot.send_message(chat, not_admin()).await?;
            }
        }
        Command::Download(arg) | Command::Music(arg) if arg.trim().is_empty() => {
            bot.send_message(
                chat,
                "Il manque le lien : /download <lien> ou /music <lien>.",
            )
            .await?;
        }
        Command::Download(arg) => spawn_job(bot, state, chat, user, arg.trim().into(), Mode::Video),
        Command::Music(arg) => spawn_job(bot, state, chat, user, arg.trim().into(), Mode::Audio),
    }
    Ok(())
}

/// Traite un message texte libre, édité ou non (téléchargement automatique du premier lien).
pub async fn handle_text(bot: Bot, msg: Message, state: Arc<AppState>) -> ResponseResult<()> {
    let (Some(user), Some(text)) = (msg.from.as_ref().map(|u| u.id.0), msg.text()) else {
        return Ok(());
    };
    // Réponse attendue à un bouton « saisir » de /settings (même admin, même chat).
    let prompt = state.prompts.lock().ok().and_then(|mut p| {
        let hit = p.get(&user).is_some_and(|(_, c)| *c == msg.chat.id);
        hit.then(|| p.remove(&user)).flatten()
    });
    if let Some((key, chat)) = prompt {
        if state.access.is_admin(user) {
            let (t, kb) = apply_setting(&state, key, text.trim(), user);
            bot.send_message(chat, t).reply_markup(kb).await?;
            return Ok(());
        }
    }
    if let Some(url) = providers::extract_urls(text).into_iter().next() {
        spawn_job(bot, state, msg.chat.id, user, url, Mode::Video);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refused_user_never_reaches_rate_check() {
        let r = admit(false, || panic!("débit consommé pour un refusé"));
        assert!(matches!(r, Err(AccessError::NotAllowed)));
        assert!(matches!(
            admit(true, || Err(AccessError::RateLimited { retry_after_s: 7 })),
            Err(AccessError::RateLimited { retry_after_s: 7 })
        ));
        assert!(admit(true, || Ok(())).is_ok());
    }

    #[test]
    fn cache_hit_skips_download() {
        assert_eq!(route(Some("F1".into())), Route::CacheHit("F1".into()));
        assert_eq!(route(None), Route::Fetch);
    }

    #[test]
    fn oversize_needs_consent_only_when_external_configured() {
        let mb = 1024 * 1024;
        assert_eq!(fit(50 * mb, 50 * mb, true), Fit::Send);
        assert_eq!(fit(51 * mb, 50 * mb, true), Fit::AskConsent { size_mb: 51 });
        assert_eq!(
            fit(51 * mb + 1, 50 * mb, true),
            Fit::AskConsent { size_mb: 52 }
        );
        assert_eq!(fit(51 * mb, 50 * mb, false), Fit::TooBig);
    }

    #[test]
    fn commands_parse() {
        assert_eq!(
            Command::parse("/download https://x.y/z", "bot").unwrap(),
            Command::Download("https://x.y/z".into())
        );
        assert_eq!(Command::parse("/stats", "bot").unwrap(), Command::Stats);
    }
}
