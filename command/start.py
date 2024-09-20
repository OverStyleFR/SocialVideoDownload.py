# Fonction pour gérer la commande /start
def start(update, context):
    user_name = update.message.from_user.first_name
    welcome_message = f"Bonjour {user_name} 👋\n\n Je suis un bot qui permet de télécharger des vidéos/musiques via des liens de réseaux sociaux (principalement YouTube & TikTok)"

    buttons = [
        [KeyboardButton(text="/start"), KeyboardButton(text="/help")],
    ]

    markup = ReplyKeyboardMarkup(buttons, one_time_keyboard=True, resize_keyboard=True)

    update.message.reply_text(welcome_message, parse_mode=ParseMode.HTML, reply_markup=markup, reply_to_message_id=update.message.message_id)

    # Log de l'action
    console_logger.info(f"Command /start execute from {update.message.from_user.username}")