from utils.logger import console_logger
from utils.helpers import get_default_keyboard

def help_command(update, context):
    help_message = (
        "Voici les commandes disponibles :\n"
        "/start - Démarrer le bot et afficher le menu\n"
        "/download <lien> - Télécharger la vidéo\n"
        "/music_download <lien> - Télécharger uniquement l'audio\n"
        "Envoyer directement un lien (http/https) déclenche un téléchargement automatique."
    )
    update.message.reply_text(help_message, reply_markup=get_default_keyboard())
    console_logger.info(f"Commande /help exécutée par {update.message.from_user.username}")
