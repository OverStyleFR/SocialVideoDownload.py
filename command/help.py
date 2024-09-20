# Modifie la fonction help
def help(update, context):
    paratext = f"Version {BOT_VERSION}\nDéveloppé par {YOUR_NAME}"

    buttons = [
        [KeyboardButton(text="/start"), KeyboardButton(text="/help")],
    ]

    markup = ReplyKeyboardMarkup(buttons, one_time_keyboard=True, resize_keyboard=True)

    context.bot.send_message(
        chat_id=update.message.chat_id,
        text=f"Je suis un bot qui télécharge des vidéos/musiques via des liens de réseaux sociaux. Voici les commandes qui me sont associées:\n/start - Pour commencer\n/help - Pour obtenir de l'aide\n/download [LIEN] - Pour télécharger une vidéo avec yt-dlp\n/music [LIEN] - Pour télécharger de la musique avec yt-dlp\n\n"
             f"Si tu m'envoies un lien directement, je tenterai automatiquement de télécharger la vidéo associée.\n\n<code>{paratext}</code>",
        parse_mode=ParseMode.HTML,
        reply_markup=markup,
    )

    # Log de l'action
    console_logger.info(f"Command /help execute from {update.message.from_user.username}")