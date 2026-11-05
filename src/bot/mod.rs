//! Bot Telegram : état partagé et lancement du dispatcher.

pub mod handlers;
pub mod markdown;
pub mod stats;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use teloxide::prelude::*;
use tokio::sync::Semaphore;

use crate::access::AccessControl;
use crate::cache::ResultCache;
use crate::config::Config;
use crate::download::Downloader;
use crate::settings::{SettingsError, SettingsStore};

/// Borne du cache de résultats.
pub const CACHE_MAX_ENTRIES: usize = 10_000;

/// État partagé entre tous les gestionnaires.
pub struct AppState {
    /// Configuration.
    pub config: Arc<Config>,
    /// Téléchargeur yt-dlp.
    pub downloader: Arc<Downloader>,
    /// Cache des file_id.
    pub cache: ResultCache,
    /// Contrôle d'accès et débit.
    pub access: AccessControl,
    /// Borne des téléchargements simultanés.
    pub semaphore: Semaphore,
    /// Fichiers trop gros en attente du « Oui » de l'utilisateur (clé = id du JobDir).
    pub pending: Mutex<HashMap<String, handlers::Pending>>,
    /// Réglages modifiables par /settings (SETTINGS_FILE).
    pub settings: SettingsStore,
    /// Saisie attendue après un bouton « Prompt » : admin -> (clé, chat).
    pub prompts: Mutex<HashMap<u64, (&'static str, ChatId)>>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("config", &self.config)
            .field("access", &self.access)
            .finish_non_exhaustive()
    }
}

impl AppState {
    /// Construit l'état ; charge les réglages depuis `config.settings_file` (absent = défauts).
    pub fn try_new(config: Arc<Config>) -> Result<Arc<Self>, SettingsError> {
        if let Some(dir) = config.settings_file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let settings = SettingsStore::load(&config.settings_file, &config)?;
        Ok(Arc::new(Self {
            downloader: Arc::new(Downloader::new(config.clone()).with_settings(settings.clone())),
            cache: ResultCache::new(config.cache_ttl, CACHE_MAX_ENTRIES),
            access: AccessControl::new(&config).with_settings(settings.clone()),
            semaphore: Semaphore::new(config.max_concurrent_downloads),
            pending: Mutex::new(HashMap::new()),
            settings,
            prompts: Mutex::new(HashMap::new()),
            config,
        }))
    }

    /// Comme [`AppState::try_new`], panique si les réglages sont illisibles (tests).
    pub fn new(config: Arc<Config>) -> Arc<Self> {
        Self::try_new(config).expect("réglages illisibles")
    }
}

/// Lance les tâches de fond du démarrage : nettoyage périodique et mise à jour de yt-dlp.
/// Les handles doivent rester en vie tant que le bot tourne.
pub fn spawn_background(state: &Arc<AppState>) -> Vec<tokio::task::JoinHandle<()>> {
    vec![
        crate::retention::spawn_janitor(&state.config, Some(state.settings.clone())),
        crate::download::spawn_updater(state.downloader.clone()),
    ]
}

/// Construit le client Bot sur l'API configurée (faux Telegram des tests compris).
pub fn build_bot(cfg: &Config) -> Bot {
    Bot::new(cfg.bot_token.clone()).set_api_url(cfg.telegram_api_url.clone())
}

/// Arbre des gestionnaires : commandes, texte libre, messages ÉDITÉS, boutons.
pub fn schema() -> teloxide::dispatching::UpdateHandler<teloxide::RequestError> {
    use handlers::{handle_callback, handle_command, handle_text, Command};
    let has_text = |m: Message| m.text().is_some();
    dptree::entry()
        .branch(
            Update::filter_message()
                .branch(
                    dptree::entry()
                        .filter_command::<Command>()
                        .endpoint(handle_command),
                )
                .branch(dptree::filter(has_text).endpoint(handle_text)),
        )
        .branch(
            Update::filter_edited_message().branch(dptree::filter(has_text).endpoint(handle_text)),
        )
        .branch(Update::filter_callback_query().endpoint(handle_callback))
}

/// Lance le dispatcher teloxide (API configurable via TELEGRAM_API_URL) jusqu'à l'arrêt.
pub async fn run(state: Arc<AppState>) {
    let bot = build_bot(&state.config);
    Dispatcher::builder(bot, schema())
        .dependencies(dptree::deps![state])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
