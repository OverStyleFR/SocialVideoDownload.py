from telegram import ParseMode
from utils.logger import console_logger

def start(update, context):
    user_name = update.message.from_user.first_name
    welcome_message = (
        f"Bonjour {user_name} 👋\n\n"
        "Je suis un bot qui permet de télécharger des vidéos/musiques via des liens de réseaux sociaux (principalement YouTube & TikTok)."
    )
    
    update.message.reply_text(welcome_message, parse_mode=ParseMode.HTML)
    console_logger.info(f"[START] Commande /start exécutée par {update.message.from_user.username}")
