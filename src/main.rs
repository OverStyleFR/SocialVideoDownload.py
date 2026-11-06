//! Point d'entrée : `--healthcheck` ou lancement du bot.

use std::process::ExitCode;
use std::sync::Arc;

use socialvideodownload::config::{Config, ConfigError};
use socialvideodownload::{bot, telemetry};

/// Modèle `.env` écrit par `token_loader.py` du bot Python.
const ENV_TEMPLATE: &str = "# === Configuration du bot Telegram ===
BOT_TOKEN=YOUR_TELEGRAM_BOT_TOKEN_HERE

# === Configuration générale ===
# VERSION est definie dans .env.example (source de verite)
DEVELOPED_BY=Tom V. | OverStyleFR
FFMPEG_PATH=ffmpeg/ffmpeg-7.0.2-amd64-static/ffmpeg
";

/// Comme `token_loader.py` : sans jeton, crée le modèle `.env` s'il manque et dit quoi faire.
fn missing_token() {
    let env = std::path::Path::new(".env");
    let msg = if env.exists() {
        "Le fichier .env est vide ou contient une valeur par défaut. Veuillez y renseigner votre token Telegram (BOT_TOKEN)."
    } else if let Err(e) = std::fs::write(env, ENV_TEMPLATE) {
        tracing::error!(error = %e, "impossible de créer .env");
        "Veuillez renseigner votre token Telegram (BOT_TOKEN) dans le .env ou l'environnement."
    } else {
        "Le fichier .env a été créé. Veuillez y renseigner votre token Telegram (BOT_TOKEN)."
    };
    tracing::error!("{msg}");
}

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
            if matches!(e, ConfigError::Missing("BOT_TOKEN")) {
                missing_token();
            }
            return ExitCode::FAILURE;
        }
    };
    // Jeton connu : toute ligne de journal passe par le masquage.
    telemetry::init_with_files(&config.bot_token, &config.logs_dir);
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
    tracing::info!("[INIT] Nettoyage du dossier downloads...");
    let cleared = socialvideodownload::retention::clear_downloads(&state.config.download_dir);
    tracing::info!(
        "[DISK_MANAGER] Contenu du dossier downloads entièrement vidé ({cleared} entrée(s))."
    );
    tracing::info!("Cache initialisé (session en mémoire).");
    let _background = bot::spawn_background(&state);
    if bot::run(state).await {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
