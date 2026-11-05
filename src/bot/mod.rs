//! Bot Telegram : état partagé et lancement du dispatcher.

pub mod handlers;
pub mod markdown;

use std::sync::Arc;

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
}

impl AppState {
    /// Construit l'état depuis la configuration (sans effet de bord).
    pub fn new(config: Arc<Config>) -> Arc<Self> {
        Arc::new(Self {
            downloader: Downloader::new(config.clone()),
            cache: ResultCache::new(config.cache_ttl, CACHE_MAX_ENTRIES),
            access: AccessControl::new(&config),
            semaphore: Semaphore::new(config.max_concurrent_downloads),
            config,
        })
    }
}

/// Lance le dispatcher teloxide (API configurable via TELEGRAM_API_URL) jusqu'à l'arrêt.
pub async fn run(state: Arc<AppState>) {
    let _ = state;
    todo!("bot::run")
}
