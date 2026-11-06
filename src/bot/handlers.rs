//! Gestionnaires des commandes, messages (édités compris) et boutons Telegram.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use teloxide::prelude::*;
use teloxide::types::{
    ChatId, InlineKeyboardButton, InlineKeyboardMarkup, InputFile, MessageId, ReplyParameters,
};
use teloxide::utils::command::BotCommands;

use super::progress::ProgressRead;
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

/// Message de /start, identique au bot Python d'origine.
pub fn start_text(first_name: &str) -> String {
    format!(
        "Bonjour {first_name} 👋\n\nJe suis un bot qui permet de télécharger des vidéos/musiques via des liens de réseaux sociaux (principalement YouTube & TikTok)."
    )
}

/// Message de /help (Markdown historique de Telegram), identique au bot Python d'origine.
pub fn help_text(version: &str, developed_by: &str) -> String {
    format!(
        "Je suis un bot qui télécharge des vidéos/musiques via des liens de réseaux sociaux. Voici les commandes qui me sont associées:\n\
/start - Pour commencer\n\
/help - Pour obtenir de l'aide\n\
/download [LIEN] - Pour télécharger une vidéo avec yt-dlp\n\
/music [LIEN] - Pour télécharger de la musique avec yt-dlp\n\n\
Si tu m'envoies un lien directement, je tenterai automatiquement de télécharger la vidéo associée.\n\n\
`Version {version}`\n`Développé par {developed_by}`"
    )
}

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
    /// Envoi direct chez l'hébergeur externe, comme utils/upload.py.
    External,
    /// Demander le consentement avant d'envoyer chez le tiers (EXTERNAL_UPLOAD_CONSENT).
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

/// Décide l'envoi Telegram, l'envoi externe (direct ou après consentement) ou le refus.
pub fn fit(size: u64, limit: u64, external: bool, consent: bool) -> Fit {
    if size <= limit {
        Fit::Send
    } else if !external {
        Fit::TooBig
    } else if consent {
        Fit::AskConsent {
            size_mb: size.div_ceil(1024 * 1024),
        }
    } else {
        Fit::External
    }
}

/// Erreur d'une tâche : message pour l'utilisateur + détail pour les journaux.
struct Failure {
    user: String,
    detail: String,
    /// Comme le bot Python après un envoi raté : le message de progression est supprimé et
    /// l'erreur part dans un nouveau message (sinon la progression est éditée).
    fresh: bool,
}

fn app(e: impl Into<AppError>) -> Failure {
    let e = e.into();
    Failure {
        user: e.user_message(),
        detail: e.to_string(),
        fresh: false,
    }
}

fn tg(e: teloxide::RequestError) -> Failure {
    Failure {
        user: "Erreur lors de l'envoi du fichier.".into(),
        detail: e.to_string(),
        fresh: true,
    }
}

fn internal(what: &str) -> Failure {
    Failure {
        user: "Erreur interne, réessaie plus tard.".into(),
        detail: what.into(),
        fresh: false,
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
    /// Message de l'utilisateur auquel le bot répond (comme `reply_to_message_id` du bot Python).
    reply_to: Option<MessageId>,
}

impl Cx {
    async fn status(&mut self, text: &str) {
        let r = match self.progress {
            Some(id) => self
                .bot
                .edit_message_text(self.chat, id, text)
                .await
                .map(|_| ()),
            None => {
                let mut req = self.bot.send_message(self.chat, text);
                if let Some(id) = self.reply_to {
                    req = req
                        .reply_parameters(ReplyParameters::new(id).allow_sending_without_reply());
                }
                req.await.map(|m| self.progress = Some(m.id))
            }
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
        if f.fresh {
            self.clear().await;
            self.say(&f.user).await;
        } else {
            self.status(&f.user).await;
        }
    }

    /// Nouveau message, comme `update.message.reply_text` de python-telegram-bot 13.
    async fn say(&self, text: &str) {
        let mut req = self.bot.send_message(self.chat, text);
        if let Some(id) = self.reply_to.filter(|_| !self.chat.is_user()) {
            req = req.reply_parameters(ReplyParameters::new(id).allow_sending_without_reply());
        }
        if let Err(e) = req.await {
            let err = self.redact(e.to_string());
            tracing::warn!(user_id = self.user, error = %err, "message non envoyé");
        }
    }

    /// Envoi chez l'hébergeur externe avec progression puis lien (textes de utils/upload.py).
    async fn external(&mut self, path: &std::path::Path) -> Result<(), Failure> {
        let state = self.state.clone();
        let cfg = &state.config;
        let base = cfg
            .external_upload_url
            .as_ref()
            .map_or_else(String::new, |u| u.to_string());
        self.status("Upload externe en cours : 0% ⏳").await;
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<u8>();
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let editor = self.progress.map(|id| {
            let (bot, chat) = (self.bot.clone(), self.chat);
            tokio::spawn(async move {
                while let Some(p) = rx.recv().await {
                    tracing::info!("[CURL UPLOAD PROGRESS] Upload progress: {p}% ⏳ pour '{name}'");
                    let _ = bot
                        .edit_message_text(chat, id, format!("Upload externe en cours : {p}% ⏳"))
                        .await;
                }
            })
        });
        let r = egress::upload_external(cfg, path, Some(tx)).await;
        if let Some(h) = editor {
            h.abort();
        }
        match r {
            Ok(link) => {
                tracing::info!(
                    "[UPLOAD] Upload externe réussi pour '{}' -> {link}",
                    path.display()
                );
                state.metrics.note_sent(None);
                self.clear().await;
                self.say(&format!(
                    "Le fichier est trop volumineux pour être envoyé directement par Telegram.\nVeuillez le télécharger ici : {link}"
                ))
                .await;
                Ok(())
            }
            Err(e) => {
                tracing::error!(
                    "[UPLOAD] Erreur upload externe pour '{}': {e}",
                    path.display()
                );
                Err(Failure {
                    user: format!(
                        "Erreur lors de l'upload externe du fichier.\nVeuillez uploader manuellement via {base}"
                    ),
                    detail: e.to_string(),
                    fresh: true,
                })
            }
        }
    }

    /// Envoie en RÉPONSE au message d'origine, avec le type choisi comme le bot Python.
    async fn send_file(
        &self,
        input: InputFile,
        kind: Kind,
        caption: Option<&str>,
        title: Option<&str>,
    ) -> Result<String, Failure> {
        macro_rules! go {
            ($req:expr) => {{
                let mut r = $req;
                if let Some(c) = caption {
                    r = r.caption(c);
                }
                if let Some(id) = self.reply_to {
                    r = r.reply_parameters(ReplyParameters::new(id).allow_sending_without_reply());
                }
                r.await.map_err(tg)?
            }};
        }
        let id = match kind {
            Kind::Video => go!(self.bot.send_video(self.chat, input))
                .video()
                .map(|v| v.file.id.0.clone()),
            Kind::Audio => {
                // Titre du lecteur audio de Telegram, accents compris.
                let mut req = self.bot.send_audio(self.chat, input);
                if let Some(t) = title {
                    req = req.title(t);
                }
                go!(req).audio().map(|a| a.file.id.0.clone())
            }
            Kind::Document => go!(self.bot.send_document(self.chat, input))
                .document()
                .map(|d| d.file.id.0.clone()),
        };
        id.ok_or_else(|| internal("réponse Telegram sans média"))
    }

    /// Envoi d'un fichier local avec progression par paliers de 10 % dans le message de statut.
    async fn send_path(
        &self,
        path: &std::path::Path,
        kind: Kind,
        name: String,
        title: Option<&str>,
    ) -> Result<String, Failure> {
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|e| app(MediaError::Io(e)))?;
        let total = file
            .metadata()
            .await
            .map_err(|e| app(MediaError::Io(e)))?
            .len();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<u8>();
        let input = InputFile::read(ProgressRead::new(file, total, tx)).file_name(name);
        let shown = path.display().to_string();
        let editor = self.progress.map(|id| {
            let (bot, chat) = (self.bot.clone(), self.chat);
            tokio::spawn(async move {
                while let Some(p) = rx.recv().await {
                    tracing::info!("[UPLOAD] Envoi du fichier {shown} : {p}% complété.");
                    let _ = bot
                        .edit_message_text(chat, id, format!("📤 Envoi en cours... {p}%"))
                        .await;
                }
            })
        });
        let r = self.send_file(input, kind, None, title).await;
        if let Some(h) = editor {
            h.abort();
        }
        r
    }
}

/// Nom affiché du fichier : le titre, comme `%(title)s.%(ext)s` du bot Python, en ASCII sans espace.
// shortcut: reqwest envoie un nom non ASCII en `filename*=utf-8''…`, que l'API Bot ne lit peut-être
// pas ; passer au titre brut une fois vérifié sur le vrai Telegram.
pub fn display_name(title: &str, path: &std::path::Path) -> String {
    let mut stem = String::new();
    for c in title.chars() {
        let c = match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'È' | 'É' | 'Ê' | 'Ë' => 'E',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'O',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
            'ç' => 'c',
            'Ç' => 'C',
            'ñ' => 'n',
            'Ñ' => 'N',
            c if c.is_ascii_alphanumeric() || "-.()[]!&+,;=@~'".contains(c) => c,
            _ => '_',
        };
        if !(c == '_' && stem.ends_with('_')) {
            stem.push(c);
        }
    }
    let stem: String = stem.chars().take(100).collect();
    let stem = stem.trim_matches(|c| c == '_' || c == '.');
    if stem.is_empty() {
        return path
            .file_name()
            .map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned());
    }
    match path.extension() {
        Some(ext) => format!("{stem}.{}", ext.to_string_lossy()),
        None => stem.to_string(),
    }
}

/// Type d'envoi Telegram, choisi par l'extension (mp4/mkv/avi vidéo, mp3/wav audio, sinon document).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Video,
    Audio,
    Document,
}

pub fn kind_of(path: &std::path::Path) -> Kind {
    match path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .as_deref()
    {
        Some("mp4" | "mkv" | "avi") => Kind::Video,
        Some("mp3" | "wav") => Kind::Audio,
        _ => Kind::Document,
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
fn spawn_job(
    bot: Bot,
    state: Arc<AppState>,
    chat: ChatId,
    reply_to: MessageId,
    user: u64,
    raw: String,
    mode: Mode,
) {
    tokio::spawn(async move {
        let mut cx = Cx {
            bot,
            state,
            chat,
            user,
            platform: Platform::Generic,
            progress: None,
            reply_to: Some(reply_to),
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
    state
        .metrics
        .note_request(cx.user, matches!(mode, Mode::Audio));
    cx.status(match mode {
        Mode::Video => "⏳ Téléchargement en cours...",
        Mode::Audio => "⏳ Téléchargement vidéo en cours...",
    })
    .await;
    let url = url_guard::validate(raw, cfg.unsafe_allow_private_hosts)
        .await
        .map_err(app)?;
    cx.platform = providers::detect(&url);
    let tag = match mode {
        Mode::Video => "DOWNLOAD",
        Mode::Audio => "MUSIC",
    };

    if let Route::CacheHit(file_id) = route(state.cache.get(&url, mode)) {
        tracing::info!(
            "[{tag}] Fichier déjà téléchargé pour l'URL: {url} par {}. Envoi depuis le cache.",
            cx.user
        );
        cx.status("📦 Utilisation du cache...").await;
        let kind = match mode {
            Mode::Video => Kind::Video,
            Mode::Audio => Kind::Audio,
        };
        match cx
            .send_file(
                InputFile::file_id(file_id.into()),
                kind,
                Some("📦 Envoyé depuis le cache"),
                None,
            )
            .await
        {
            Ok(_) => {
                cx.clear().await;
                return Ok(());
            }
            Err(f) => {
                tracing::warn!(user_id = cx.user, error = %f.detail, "file_id du cache refusé, retéléchargement");
                cx.status("⏳ Téléchargement en cours...").await;
            }
        }
    }

    let _permit = state
        .semaphore
        .acquire()
        .await
        .map_err(|_| internal("sémaphore fermé"))?;
    state.metrics.note_active(
        cfg.max_concurrent_downloads
            .saturating_sub(state.semaphore.available_permits()),
    );
    crate::retention::check_and_clean_if_needed(cfg, Some(&state.settings)).await;
    let job = JobDir::create(&cfg.download_dir, &job_id(cx.user))
        .map_err(|e| app(DownloadError::Io(e)))?;
    // Trois tentatives comme le bot Python ; texte d'échec identique.
    let mut last = None;
    let mut got = None;
    for attempt in 1..=3 {
        tracing::info!(
            "[{tag}] Tentative {attempt} de téléchargement pour l'URL: {url} par {}",
            cx.user
        );
        match state.downloader.fetch(&url, mode, &job).await {
            Ok(d) => {
                got = Some(d);
                break;
            }
            Err(e) => {
                tracing::error!(
                    "[{tag}] Tentative {attempt} échouée pour l'URL: {url} par {} - {e}",
                    cx.user
                );
                // Réessayer ne changerait rien : on le dit tout de suite, avec le message précis.
                if matches!(
                    e,
                    DownloadError::Unsupported
                        | DownloadError::TooLarge { .. }
                        | DownloadError::TooLong { .. }
                        | DownloadError::Timeout
                ) {
                    return Err(app(e));
                }
                last = Some(e);
            }
        }
    }
    let Some(dl) = got else {
        let detail = last.map_or_else(String::new, |e| e.to_string());
        return Err(Failure {
            user: "❌ Échec du téléchargement après plusieurs tentatives.".into(),
            detail,
            fresh: false,
        });
    };

    let limit = state
        .settings
        .get_u64("send.telegram_limit_mb")
        .map_or(cfg.upload_limit_bytes(), |mb| mb * 1024 * 1024);
    let external = cfg.external_upload_url.is_some()
        && state
            .settings
            .get_bool("send.external_fallback")
            .unwrap_or(true);
    // Contrôle audio pendant l'envoi : il ne retarde plus le fichier, et son échec n'empêche pas l'envoi.
    let audio_check = async {
        if !matches!(mode, Mode::Video) {
            return None;
        }
        media::audio_issue(cfg, &dl.path).await.unwrap_or_else(|e| {
            tracing::warn!(error = %e, "contrôle audio impossible, envoi sans avertissement");
            None
        })
    };
    // Option : le JobDir ne part dans `pending` que sur demande d'accord ; sinon il vit jusqu'à la
    // fin du contrôle audio qui lit encore le fichier.
    let jid = job.id().to_string();
    let mut job = Some(job);
    let deliver = async {
        let path = match mode {
            Mode::Video => {
                // Compression : option (désactivée par défaut), le bot Python n'en faisait aucune.
                if state.settings.get_bool("send.compress").unwrap_or(false) {
                    cx.status("🗜️ Préparation du fichier…").await;
                    match media::compress_under(cfg, &dl.path, limit).await {
                        Ok(p) => p,
                        Err(MediaError::CannotFit { .. }) => dl.path.clone(),
                        Err(e) => return Err(app(e)),
                    }
                } else {
                    dl.path.clone()
                }
            }
            Mode::Audio => {
                cx.status("🔄 Conversion audio...").await;
                let dst = dl.path.with_file_name(format!("{jid}.audio.mp3"));
                // libmp3lame à 192k comme commands/music.py ; réglable dans /settings (audio.bitrate_kbps).
                let kbps = state.settings.get_u64("audio.bitrate_kbps").unwrap_or(192);
                let mp3 = media::to_mp3_atomic(cfg, &dl.path, &dst, kbps)
                    .await
                    .map_err(|e| {
                        tracing::error!(
                            "[MUSIC] Erreur conversion en audio pour {} par {} - {e}",
                            dl.path.display(),
                            cx.user
                        );
                        Failure {
                            user: "❌ Erreur lors de la conversion en audio.".into(),
                            detail: e.to_string(),
                            fresh: false,
                        }
                    })?;
                tracing::info!(
                    "[MUSIC] Conversion terminée: {} pour {}",
                    mp3.display(),
                    cx.user
                );
                mp3
            }
        };

        let size = tokio::fs::metadata(&path)
            .await
            .map_err(|e| app(MediaError::Io(e)))?
            .len();
        let ask = !state
            .settings
            .get_bool("send.external_always_yes")
            .unwrap_or(!cfg.external_upload_consent);
        match fit(size, limit, external, ask) {
            Fit::TooBig => Err(app(EgressError::Disabled)),
            Fit::External => {
                tracing::info!(
                    "[UPLOAD] Fichier '{}' trop volumineux ({size} octets). Upload externe via {}.",
                    path.display(),
                    cfg.external_upload_url
                        .as_ref()
                        .and_then(|u| u.host_str())
                        .unwrap_or("?")
                );
                cx.external(&path).await
            }
            Fit::AskConsent { size_mb } => {
                let id = jid.clone();
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
                            _job: job.take().ok_or_else(|| internal("JobDir absent"))?,
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
                cx.status("📤 Envoi en cours... 0%").await;
                let kind = kind_of(&path);
                let title =
                    (kind == Kind::Audio && !dl.title.is_empty()).then_some(dl.title.as_str());
                let file_id = cx
                    .send_path(&path, kind, display_name(&dl.title, &path), title)
                    .await
                    .inspect_err(|f| {
                        tracing::error!(
                            "[UPLOAD] Erreur lors de l'envoi du fichier '{}': {}",
                            path.display(),
                            f.detail
                        )
                    })?;
                let what = match kind {
                    Kind::Video => "Vidéo envoyée",
                    Kind::Audio => "Audio envoyé",
                    Kind::Document => "Document envoyé",
                };
                tracing::info!("[UPLOAD] {what} : {}", path.display());
                // Un document (.webm, .mov…) ne se renvoie pas par send_video : pas de cache pour lui.
                if kind != Kind::Document {
                    state.cache.put(&url, mode, file_id, size);
                }
                state.metrics.note_sent(Some(size));
                cx.clear().await;
                Ok(())
            }
        }
    };
    let (r, issue) = tokio::join!(deliver, audio_check);
    if let (Ok(()), Some(issue)) = (&r, issue) {
        let what = match issue {
            media::AudioIssue::NoAudioTrack => "n'a pas de piste audio",
            media::AudioIssue::SilentTrack => "a une piste audio muette",
        };
        let _ = cx
            .bot
            .send_message(cx.chat, format!("⚠️ Cette vidéo {what} à la source."))
            .await;
    }
    r
}

/// Comme `update.message.reply_text` de python-telegram-bot 13 : cite le message de
/// l'utilisateur seulement hors conversation privée.
fn reply(bot: &Bot, msg: &Message, text: impl Into<String>) -> <Bot as Requester>::SendMessage {
    let req = bot.send_message(msg.chat.id, text);
    if msg.chat.is_private() {
        req
    } else {
        req.reply_parameters(ReplyParameters::new(msg.id).allow_sending_without_reply())
    }
}

/// Nom de l'utilisateur dans les journaux (pseudo, sinon ID), comme `from_user.username`.
fn who(msg: &Message) -> String {
    msg.from.as_ref().map_or_else(String::new, |u| {
        u.username
            .clone()
            .map_or_else(|| u.id.0.to_string(), |n| format!("@{n}"))
    })
}

/// Premier argument, comme `context.args[0]` du bot Python.
fn first_token(arg: &str) -> String {
    arg.split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string()
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
            reply_to: None,
        };
        let _permit = state.semaphore.acquire().await;
        if let Err(f) = cx.external(&pending.path).await {
            cx.fail(f).await;
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
            tracing::info!("[START] Commande /start exécutée par {}", who(&msg));
            let name = msg.from.as_ref().map_or("", |u| u.first_name.as_str());
            reply(&bot, &msg, start_text(name)).await?;
        }
        Command::Help => {
            tracing::info!("[HELP] Commande /help exécutée par {}", who(&msg));
            let text = help_text(&state.config.version, &state.config.developed_by);
            // Markdown historique voulu : rendu identique au bot Python (parse_mode="Markdown").
            #[allow(deprecated)]
            let mode = teloxide::types::ParseMode::Markdown;
            reply(&bot, &msg, text).parse_mode(mode).await?;
        }
        Command::Stats => {
            // Réservé aux admins, jugé sur l'ID numérique (jamais sur le pseudo).
            if state.access.is_admin(user) {
                tracing::info!("[STATS] Commande /stats exécutée par {}", who(&msg));
                // Markdown historique voulu : rendu identique au bot Python (parse_mode="Markdown").
                #[allow(deprecated)]
                let mode = teloxide::types::ParseMode::Markdown;
                reply(&bot, &msg, stats_text(&state).await)
                    .parse_mode(mode)
                    .await?;
            } else {
                tracing::warn!(
                    "[STATS] Tentative non autorisée par {} (id={user})",
                    who(&msg)
                );
                // Texte d'origine de commands/stats.py.
                reply(
                    &bot,
                    &msg,
                    "Accès refusé. Cette commande est réservée au développeur.",
                )
                .await?;
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
        Command::Download(arg) if arg.trim().is_empty() => {
            tracing::info!("[DOWNLOAD] Aucun lien fourni par {}.", who(&msg));
            reply(
                &bot,
                &msg,
                "Veuillez fournir un lien pour le téléchargement. Ex: /download <LIEN>",
            )
            .await?;
        }
        Command::Music(_)
            if !state
                .settings
                .get_bool("audio.music_enabled")
                .unwrap_or(true) =>
        {
            tracing::info!(
                "[MUSIC] Commande désactivée dans /settings, demandée par {}.",
                who(&msg)
            );
            reply(
                &bot,
                &msg,
                "La commande /music est désactivée par l'administrateur.",
            )
            .await?;
        }
        Command::Music(arg) if arg.trim().is_empty() => {
            tracing::info!("[MUSIC] Aucun lien fourni par {}.", who(&msg));
            reply(
                &bot,
                &msg,
                "Veuillez fournir un lien pour télécharger l'audio. Ex: /music <LIEN>",
            )
            .await?;
        }
        Command::Download(arg) => {
            tracing::info!(
                "[DOWNLOAD] Traitement de l'URL: {} par {}",
                first_token(&arg),
                who(&msg)
            );
            spawn_job(
                bot,
                state,
                chat,
                msg.id,
                user,
                first_token(&arg),
                Mode::Video,
            );
        }
        Command::Music(arg) => {
            tracing::info!(
                "[MUSIC] Traitement de l'URL: {} par {}",
                first_token(&arg),
                who(&msg)
            );
            spawn_job(
                bot,
                state,
                chat,
                msg.id,
                user,
                first_token(&arg),
                Mode::Audio,
            );
        }
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
    // Comme `Filters.text & ~Filters.command` : une commande inconnue n'est jamais téléchargée.
    if text.starts_with('/') {
        return Ok(());
    }
    if let Some(url) = providers::extract_urls(text).into_iter().next() {
        tracing::info!(
            "URL détectée dans le message : {url}, lancement du téléchargement automatique."
        );
        spawn_job(bot, state, msg.chat.id, msg.id, user, url, Mode::Video);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_and_start_are_the_python_bot_messages() {
        let h = help_text("V9.4-3", "Tom V. | OverStyleFR");
        assert!(h.starts_with("Je suis un bot qui télécharge des vidéos/musiques via des liens de réseaux sociaux. Voici les commandes qui me sont associées:\n/start - Pour commencer\n/help - Pour obtenir de l'aide\n/download [LIEN] - Pour télécharger une vidéo avec yt-dlp\n/music [LIEN] - Pour télécharger de la musique avec yt-dlp\n\nSi tu m'envoies"), "{h}");
        assert!(
            h.ends_with("\n\n`Version V9.4-3`\n`Développé par Tom V. | OverStyleFR`"),
            "{h}"
        );
        assert_eq!(
            start_text("Tom"),
            "Bonjour Tom 👋\n\nJe suis un bot qui permet de télécharger des vidéos/musiques via des liens de réseaux sociaux (principalement YouTube & TikTok)."
        );
    }

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
    fn oversize_goes_external_directly_or_after_consent() {
        let mb = 1024 * 1024;
        assert_eq!(fit(50 * mb, 50 * mb, true, false), Fit::Send);
        assert_eq!(fit(51 * mb, 50 * mb, true, false), Fit::External);
        assert_eq!(
            fit(51 * mb, 50 * mb, true, true),
            Fit::AskConsent { size_mb: 51 }
        );
        assert_eq!(
            fit(51 * mb + 1, 50 * mb, true, true),
            Fit::AskConsent { size_mb: 52 }
        );
        assert_eq!(fit(51 * mb, 50 * mb, false, false), Fit::TooBig);
        assert_eq!(fit(51 * mb, 50 * mb, false, true), Fit::TooBig);
    }

    #[test]
    fn nom_affiche_tire_du_titre() {
        let p = std::path::Path::new("/d/abc.audio.mp3");
        assert_eq!(
            display_name("Café de Flore (Live) / 2024 🎵", p),
            "Cafe_de_Flore_(Live)_2024.mp3"
        );
        assert_eq!(display_name("🎵 🎵", p), "abc.audio.mp3");
        assert_eq!(display_name("", std::path::Path::new("x.mp4")), "x.mp4");
        let long = display_name(&"a".repeat(300), std::path::Path::new("x.mp4"));
        assert_eq!(long.len(), 104);
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
