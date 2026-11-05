//! Point d'entrée : `--healthcheck` ou lancement du bot.

use std::process::ExitCode;
use std::sync::Arc;

use socialvideodownload::{bot, config::Config, telemetry};

#[tokio::main]
async fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("--healthcheck") {
        let path = std::env::var("HEARTBEAT_FILE").unwrap_or_else(|_| "/tmp/svd-heartbeat".into());
        let fresh = telemetry::heartbeat_fresh(path.as_ref(), telemetry::HEARTBEAT_MAX_AGE);
        return if fresh {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }

    telemetry::init();
    let config = match Config::from_env() {
        Ok(c) => Arc::new(c),
        Err(e) => {
            tracing::error!(error = %e, "configuration invalide");
            return ExitCode::FAILURE;
        }
    };
    if config.unsafe_allow_private_hosts {
        tracing::warn!(
            "UNSAFE_ALLOW_PRIVATE_HOSTS actif : protection SSRF désactivée (bancs/tests seulement)"
        );
    }
    if config.allowed_user_ids.is_empty() && !config.allow_all_users {
        tracing::warn!(
            "ALLOWED_USER_IDS vide et ALLOW_ALL_USERS absent : tout le monde sera refusé"
        );
    }
    tracing::info!(?config, "démarrage");

    let _heartbeat = telemetry::spawn_heartbeat(config.heartbeat_file.clone());
    let state = bot::AppState::new(config);
    let _background = bot::spawn_background(&state);
    bot::run(state).await;
    ExitCode::SUCCESS
}
