# Fonction pour gérer les messages textuels
def handle_text_messages(update, context):
    print("handle_text_messages() a été appelé !")  # Debug
    text = update.message.text
    console_logger.info(f"Message reçu : {text}")
