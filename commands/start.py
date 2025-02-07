from telegram import ParseMode
from utils.logger import console_logger
from utils.helpers import get_default_keyboard

def start(update, context):
    user_name = update.message.from_user.first_name
    welcome_message = (
        f"Bonjour {user_name} 👋\n\n"
        "Je suis un bot qui permet de télécharger des vidéos/musiques via des liens de réseaux sociaux (principalement YouTube & TikTok)."
    )
    
    update.message.reply_text(
        welcome_message,
        parse_mode=ParseMode.HTML,
        reply_markup=get_default_keyboard(),
        reply_to_message_id=update.message.message_id
    )
    console_logger.info(f"Commande /start exécutée par {update.message.from_user.username}")
