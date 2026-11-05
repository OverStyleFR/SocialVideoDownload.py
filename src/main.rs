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

    let config = match Config::from_env() {
        Ok(c) => Arc::new(c),
        Err(e) => {
            telemetry::init();
            tracing::error!(error = %e, "configuration invalide");
            return ExitCode::FAILURE;
        }
    };
    // Jeton connu : toute ligne de journal passe par le masquage.
    telemetry::init_redacted(&config.bot_token);
    if config.unsafe_allow_private_hosts {
        tracing::warn!(
            "UNSAFE_ALLOW_PRIVATE_HOSTS actif : protection SSRF désactivée (bancs/tests seulement)"
        );
    }
    if config.allowed_user_ids.is_empty() && !config.allow_all_users {
        tracing::info!(
            "ALLOWED_USER_IDS vide : le bot est ouvert à tous, comme l'ancien bot Python (ADMIN_USER_IDS ne réserve que /stats et /settings)"
        );
    }
    tracing::info!(?config, "démarrage");

    let _heartbeat = telemetry::spawn_heartbeat(config.heartbeat_file.clone());
    let state = match bot::AppState::try_new(config) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "réglages illisibles (SETTINGS_FILE)");
            return ExitCode::FAILURE;
        }
    };
    if let Some(level) = state.settings.get_choice("sys.log_level") {
        if let Err(e) = telemetry::set_level(&level) {
            tracing::warn!(error = %e, "niveau de journaux ignoré");
        }
    }
    let _background = bot::spawn_background(&state);
    bot::run(state).await;
    ExitCode::SUCCESS
}
