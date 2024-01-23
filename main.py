import subprocess
import os
from telegram import InputFile
from telegram.ext import Updater, CommandHandler, MessageHandler, Filters

# Fonction pour gérer la commande /start
def start(update, context):
    context.bot.send_message(chat_id=update.message.chat_id, text="Salut! Je suis un bot simple.")

# Fonction pour gérer la commande /help
def help(update, context):
    context.bot.send_message(chat_id=update.message.chat_id, text="Je suis un bot simple. Voici quelques commandes que je comprends:\n/start - Pour commencer\n/help - Pour obtenir de l'aide\n/sendvideo - Pour envoyer une vidéo\n/download [LIEN] - Pour télécharger une vidéo avec yt-dlp")

# Fonction pour gérer les messages textuels
def handle_text_messages(update, context):
    text = update.message.text
    context.bot.send_message(chat_id=update.message.chat_id, text=f"Tu as dit: {text}")

# Fonction pour gérer la commande /download
def download(update, context):
    # Récupérer le lien depuis la commande
    link = " ".join(context.args)

    if not link:
        context.bot.send_message(chat_id=update.message.chat_id, text="Utilisation: /download [LIEN]")
        return

    # Exécuter la commande ./yt-dlp avec le lien
    try:
        result = subprocess.run(["./yt-dlp", "--format", "best", "-o", "downloaded_video.mp4", link], capture_output=True, text=True)
        output = result.stdout.strip() if result.stdout else result.stderr.strip()
        context.bot.send_message(chat_id=update.message.chat_id, text=output)

        # Envoyer la vidéo téléchargée
        video_path = "downloaded_video.mp4"
        if os.path.exists(video_path):
            video = open(video_path, "rb")
            context.bot.send_video(chat_id=update.message.chat_id, video=InputFile(video), caption="Voici votre vidéo!")
            video.close()
            os.remove(video_path)  # Supprimer le fichier après l'envoi
        else:
            context.bot.send_message(chat_id=update.message.chat_id, text="Erreur: La vidéo téléchargée n'a pas été trouvée.")
    except Exception as e:
        context.bot.send_message(chat_id=update.message.chat_id, text=f"Erreur lors de l'exécution de la commande: {str(e)}")

def main():
    # Token de votre bot Telegram
    token = "6729543306:AAFUULf3ERW3ygJ3uxIHc73dFh_mNnYCnAU"

    # Initialisation de l'updater avec le token du bot
    updater = Updater(token=token, use_context=True)

    # Obtention du gestionnaire des commandes
    dp = updater.dispatcher

    # Ajout des gestionnaires de commandes
    dp.add_handler(CommandHandler("start", start))
    dp.add_handler(CommandHandler("help", help))
    dp.add_handler(CommandHandler("download", download, pass_args=True))

    # Ajout du gestionnaire pour les messages textuels
    dp.add_handler(MessageHandler(Filters.text & ~Filters.command, handle_text_messages))

    # Démarrage du bot
    updater.start_polling()

    # Arrêt du bot lorsqu'on appuie sur Ctrl+C
    updater.idle()

if __name__ == "__main__":
    main()
