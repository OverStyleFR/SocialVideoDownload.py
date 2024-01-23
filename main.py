import subprocess
import os
import logging

from datetime import datetime
from telegram import InputFile, ReplyKeyboardMarkup, KeyboardButton, ParseMode
from telegram.ext import Updater, CommandHandler, MessageHandler, Filters

# Créer le dossier "logs" s'il n'existe pas
if not os.path.exists("logs"):
    os.makedirs("logs")

# Configuration du logging
log_file_name = f"logs/bot_{datetime.now().strftime('%Y%m%d_%H%M%S')}.log"
file_handler = logging.FileHandler(log_file_name)
file_handler.setLevel(logging.INFO)

# Créer un logger pour la console
console_logger = logging.getLogger("console_logger")
console_logger.setLevel(logging.INFO)

# Ajouter un gestionnaire de fichier au logger principal
logging.basicConfig(level=logging.INFO, handlers=[file_handler])

BOT_VERSION = "V0.3"
YOUR_NAME = "Tom V. | OverStyleFR"

# Créer un gestionnaire de console et l'ajouter uniquement au logger de la console
console_handler = logging.StreamHandler()
console_handler.setLevel(logging.INFO)
console_logger.addHandler(console_handler)

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
    logging.info(f"Commande /start exécutée par {update.message.from_user.username}")
    console_logger.info(f"Commande /start exécutée par {update.message.from_user.username}")

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
    logging.info(f"Commande /help exécutée par {update.message.from_user.username}")
    console_logger.info(f"Commande /help exécutée par {update.message.from_user.username}")

# Fonction pour gérer les messages textuels
def handle_text_messages(update, context):
    text = update.message.text

    # Log de l'action
    logging.info(f"Message texte reçu: {text}")
    console_logger.info(f"Message texte reçu: {text}")

    if text.startswith("https"):
        video_path = "downloaded_video.mp4"

        # Log de l'action
        logging.info(f"Tentative de téléchargement de la vidéo depuis le lien: {text}")
        console_logger.info(f"Tentative de téléchargement de la vidéo depuis le lien: {text}")

        if os.path.exists(video_path):
            os.remove(video_path)

        try:
            result = subprocess.run(["./yt-dlp", "--format", "best", "-o", "downloaded_video.mp4", text], capture_output=True, text=True)
            output = result.stdout.strip() if result.stdout else result.stderr.strip()

            if os.path.exists(video_path):
                video = open(video_path, "rb")
                context.bot.send_video(chat_id=update.message.chat_id, video=InputFile(video), caption="Voici votre vidéo!", reply_to_message_id=update.message.message_id)
                video.close()
            else:
                context.bot.send_message(chat_id=update.message.chat_id, text="Erreur: La vidéo téléchargée n'a pas été trouvée.")
        except Exception as e:
            context.bot.send_message(chat_id=update.message.chat_id, text=f"Erreur lors de l'exécution de la commande: {str(e)}", reply_to_message_id=update.message.message_id)

# Fonction pour gérer la commande /download
def download(update, context):
    link = " ".join(context.args) if context.args else update.message.text

    if not link:
        context.bot.send_message(chat_id=update.message.chat_id, text="Utilisation: /download [LIEN]")
        return

    # Log de l'action
    logging.info(f"Tentative de téléchargement depuis la commande /download avec le lien: {link}")
    console_logger.info(f"Tentative de téléchargement depuis la commande /download avec le lien: {link}")

    if not link.startswith("http"):
        context.bot.send_message(chat_id=update.message.chat_id, text="Erreur: Le texte n'est pas un lien.")
        return

    try:
        result = subprocess.run(["./yt-dlp", "--format", "best", "-o", "downloaded_video.mp4", link], capture_output=True, text=True)
        output = result.stdout.strip() if result.stdout else result.stderr.strip()
        context.bot.send_message(chat_id=update.message.chat_id, text=output)

        video_path = "downloaded_video.mp4"
        if os.path.exists(video_path):
            video = open(video_path, "rb")
            context.bot.send_video(chat_id=update.message.chat_id, video=InputFile(video), caption="Voici votre vidéo!")
            video.close()
            os.remove(video_path)
        else:
            context.bot.send_message(chat_id=update.message.chat_id, text="Erreur: La vidéo téléchargée n'a pas été trouvée.")
    except Exception as e:
        context.bot.send_message(chat_id=update.message.chat_id, text=f"Erreur lors de l'exécution de la commande: {str(e)}")

    # Log de l'action
    logging.info(f"Commande /download exécutée par {update.message.from_user.username}")
    console_logger.info(f"Commande /download exécutée par {update.message.from_user.username}")

# Fonction pour gérer la commande /music
def music(update, context):
    link = " ".join(context.args)

    if not link:
        context.bot.send_message(chat_id=update.message.chat_id, text="Utilisation: /music [LIEN]")
        return

    # Log de l'action
    logging.info(f"Tentative de téléchargement de la musique depuis la commande /music avec le lien: {link}")
    console_logger.info(f"Tentative de téléchargement de la musique depuis la commande /music avec le lien: {link}")

    if not link.startswith("http"):
        context.bot.send_message(chat_id=update.message.chat_id, text="Erreur: Le texte n'est pas un lien.")
        return

    try:
        ffmpeg_location = "ffmpeg-6.1-amd64-static/ffmpeg"
        result = subprocess.run(["./yt-dlp", "--extract-audio", "--audio-format", "mp3", "--ffmpeg-location", ffmpeg_location, "-o", "downloaded_music.%(ext)s", link], capture_output=True, text=True)
        output = result.stdout.strip() if result.stdout else result.stderr.strip()
        context.bot.send_message(chat_id=update.message.chat_id, text=output)

        music_path = "downloaded_music.mp3"
        if os.path.exists(music_path):
            music = open(music_path, "rb")
            context.bot.send_audio(chat_id=update.message.chat_id, audio=InputFile(music), caption="Voici votre musique!")
            music.close()
            os.remove(music_path)
        else:
            context.bot.send_message(chat_id=update.message.chat_id, text="Erreur: La musique téléchargée n'a pas été trouvée.")
    except Exception as e:
        context.bot.send_message(chat_id=update.message.chat_id, text=f"Erreur lors de l'exécution de la commande: {str(e)}")

    # Log de l'action
    logging.info(f"Commande /music exécutée par {update.message.from_user.username}")
    console_logger.info(f"Commande /music exécutée par {update.message.from_user.username}")

def main():
    token = "6977266339:AAHNxnhQn6pU_d0g7KioCOG7QclsUF0PBWk"
    updater = Updater(token=token, use_context=True)
    dp = updater.dispatcher

    dp.add_handler(CommandHandler("start", start))
    dp.add_handler(CommandHandler("help", help))
    dp.add_handler(CommandHandler("download", download, pass_args=True))
    dp.add_handler(CommandHandler("music", music, pass_args=True))

    dp.add_handler(MessageHandler(Filters.text & ~Filters.command, handle_text_messages))

    updater.start_polling()

    console_logger.info("Le bot a démarré avec succès!3")
    updater.idle()

if __name__ == "__main__":
    main()
