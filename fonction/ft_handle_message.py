# Fonction pour gérer les messages textuels
def handle_text_messages(update, context):
    text = update.message.text

    # Log de l'action
    console_logger.info(f"Message recieve : {text}")

    if text.startswith("https"):
        reply_message = context.bot.send_message(chat_id=update.message.chat_id, text="Téléchargement en cours. Veuillez patienter...", reply_to_message_id=update.message.message_id)
        
        download_and_send_video(context.bot, update.message.chat_id, text, update)