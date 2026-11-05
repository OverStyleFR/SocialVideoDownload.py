//! Journalisation tracing et battement de cœur pour le HEALTHCHECK Docker.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use tracing_subscriber::EnvFilter;

/// Âge maximal du battement de cœur pour être déclaré sain.
pub const HEARTBEAT_MAX_AGE: Duration = Duration::from_secs(120);
/// Période d'écriture du battement de cœur.
pub const HEARTBEAT_PERIOD: Duration = Duration::from_secs(30);

/// Initialise tracing : LOG_FORMAT=json ou texte (défaut), filtre RUST_LOG (défaut info), sans ANSI.
pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false);
    if std::env::var("LOG_FORMAT").is_ok_and(|v| v.eq_ignore_ascii_case("json")) {
        builder.json().init();
    } else {
        builder.init();
    }
}

/// Lance la tâche qui réécrit périodiquement le fichier battement de cœur.
pub fn spawn_heartbeat(path: PathBuf) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(HEARTBEAT_PERIOD);
        loop {
            tick.tick().await;
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            if let Err(e) = tokio::fs::write(&path, now.to_string()).await {
                tracing::warn!(error = %e, "écriture du heartbeat impossible");
            }
        }
    })
}

/// Vrai si le fichier battement de cœur a été modifié il y a moins de `max_age`.
pub fn heartbeat_fresh(path: &Path, max_age: Duration) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age < max_age)
}
