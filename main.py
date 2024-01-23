from telegram import InputFile
from telegram.ext import Updater, CommandHandler, MessageHandler, Filters

# Fonction pour gérer la commande /start
def start(update, context):
    context.bot.send_message(chat_id=update.message.chat_id, text="Salut! Je suis un bot simple.")

# Fonction pour gérer la commande /help
def help(update, context):
    context.bot.send_message(chat_id=update.message.chat_id, text="Je suis un bot simple. Voici quelques commandes que je comprends:\n/start - Pour commencer\n/help - Pour obtenir de l'aide")

# Fonction pour gérer les messages textuels
def handle_text_messages(update, context):
    text = update.message.text
    context.bot.send_message(chat_id=update.message.chat_id, text=f"Tu as dit: {text}")

# Fonction pour gérer la commande /sendvideo
def send_video(update, context):
    video_path = "video.mp4"  # Remplacez "video.mp4" par le nom de votre fichier vidéo
    video = open(video_path, "rb")
    context.bot.send_video(chat_id=update.message.chat_id, video=video, caption="Voici votre vidéo!")
    video.close()

def main():
    # Token de votre bot Telegram
    token = "TOKEN"

    # Initialisation de l'updater avec le token du bot
    updater = Updater(token=token, use_context=True)

    # Obtention du gestionnaire des commandes
    dp = updater.dispatcher

    # Ajout des gestionnaires de commandes
    dp.add_handler(CommandHandler("start", start))
    dp.add_handler(CommandHandler("help", help))
    dp.add_handler(CommandHandler("sendvideo", send_video))

    # Ajout du gestionnaire pour les messages textuels
    dp.add_handler(MessageHandler(Filters.text & ~Filters.command, handle_text_messages))

    # Démarrage du bot
    updater.start_polling()

    # Arrêt du bot lorsqu'on appuie sur Ctrl+C
    updater.idle()

if __name__ == "__main__":
    main()