//! Gestionnaires des commandes et messages Telegram.

use std::sync::Arc;

use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;

use super::AppState;

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
}

/// Traite une commande reconnue.
pub async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Command,
    state: Arc<AppState>,
) -> ResponseResult<()> {
    let _ = (bot, msg, cmd, state);
    todo!("handlers::handle_command")
}

/// Traite un message texte libre (téléchargement automatique des liens).
pub async fn handle_text(bot: Bot, msg: Message, state: Arc<AppState>) -> ResponseResult<()> {
    let _ = (bot, msg, state);
    todo!("handlers::handle_text")
}
