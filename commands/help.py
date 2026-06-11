from utils.logger import console_logger
from config import VERSION, DEVELOPED_BY

def help_command(update, context):
    help_message = (
        "Je suis un bot qui télécharge des vidéos/musiques via des liens de réseaux sociaux. Voici les commandes qui me sont associées:\n"
        "/start - Pour commencer\n"
        "/help - Pour obtenir de l'aide\n"
        "/download [LIEN] - Pour télécharger une vidéo avec yt-dlp\n"
        "/music [LIEN] - Pour télécharger de la musique avec yt-dlp\n\n"
        "Si tu m'envoies un lien directement, je tenterai automatiquement de télécharger la vidéo associée.\n\n"
        "`Version {0}`\n`Développé par {1}`".format(VERSION, DEVELOPED_BY)
    )
    update.message.reply_text(help_message, parse_mode="Markdown")
    console_logger.info(f"[HELP] Commande /help exécutée par {update.message.from_user.username}")
