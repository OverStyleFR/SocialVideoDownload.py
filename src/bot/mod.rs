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

/// Borne du cache de résultats.
pub const CACHE_MAX_ENTRIES: usize = 10_000;

/// État partagé entre tous les gestionnaires.
#[derive(Debug)]
pub struct AppState {
    /// Configuration.
    pub config: Arc<Config>,
    /// Téléchargeur yt-dlp.
    pub downloader: Downloader,
    /// Cache des file_id.
    pub cache: ResultCache,
    /// Contrôle d'accès et débit.
    pub access: AccessControl,
    /// Borne des téléchargements simultanés.
    pub semaphore: Semaphore,
    /// Fichiers trop gros en attente du « Oui » de l'utilisateur (clé = id du JobDir).
    pub pending: Mutex<HashMap<String, handlers::Pending>>,
}

impl AppState {
    /// Construit l'état depuis la configuration (sans effet de bord).
    pub fn new(config: Arc<Config>) -> Arc<Self> {
        Arc::new(Self {
            downloader: Downloader::new(config.clone()),
            cache: ResultCache::new(config.cache_ttl, CACHE_MAX_ENTRIES),
            access: AccessControl::new(&config),
            semaphore: Semaphore::new(config.max_concurrent_downloads),
            pending: Mutex::new(HashMap::new()),
            config,
        })
    }
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
