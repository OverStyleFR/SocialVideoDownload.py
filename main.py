import subprocess
import os
import logging
import hashlib

from datetime import datetime
from telegram import ReplyKeyboardMarkup, KeyboardButton, ParseMode
from telegram.ext import ApplicationBuilder, CommandHandler, MessageHandler, filters

# Importer le token depuis le sous-programme .command/token.py
from fonction.ft_token import get_token

# Dossier de téléchargement temporaire
download_temp_folder = "download_temp"

# Créer le dossier "logs" s'il n'existe pas
if not os.path.exists("logs"):
    os.makedirs("logs")

# Configuration du logging
log_file_name = f"logs/bot_{datetime.now().strftime('%Y%m%d_%H%M%S')}.log"
file_handler = logging.FileHandler(log_file_name)
file_handler.setLevel(logging.INFO)

log_formatter = logging.Formatter('%(asctime)s - %(levelname)s - %(message)s')
file_handler.setFormatter(log_formatter)

console_logger = logging.getLogger("console_logger")
console_logger.setLevel(logging.INFO)
console_handler = logging.StreamHandler()
console_handler.setFormatter(log_formatter)
console_logger.addHandler(console_handler)

# Ajouter le gestionnaire de fichier au logger principal
logging.basicConfig(level=logging.INFO, handlers=[file_handler])

BOT_VERSION = "V0.7-3"
YOUR_NAME = "Tom V. | OverStyleFR"

async def start(update, context):
    await update.message.reply_text("Bienvenue sur le bot!")

async def help(update, context):
    await update.message.reply_text("Voici les commandes disponibles...")

async def download(update, context):
    args = context.args
    await update.message.reply_text(f"Téléchargement en cours pour: {args}")

async def music(update, context):
    args = context.args
    await update.message.reply_text(f"Recherche de musique pour: {args}")

async def handle_text_messages(update, context):
    user_message = update.message.text
    await update.message.reply_text(f"Message reçu: {user_message}")

async def main():
    token = get_token()  # Récupérer le token depuis le sous-programme

    if not token:
        print("Le token n'a pas été trouvé. Assurez-vous que .command/token.py contient le token.")
        return

    if os.path.exists(download_temp_folder):
        for file_name in os.listdir(download_temp_folder):
            file_path = os.path.join(download_temp_folder, file_name)
            try:
                if os.path.isfile(file_path):
                    os.unlink(file_path)
            except Exception as e:
                console_logger.error(f"Erreur lors de la suppression de {file_name}: {e}")
        console_logger.info('Folder exists. All files have been deleted.')
    else:
        os.makedirs(download_temp_folder)
        console_logger.info("Folder created")

    application = ApplicationBuilder().token(token).build()

    application.add_handler(CommandHandler("start", start))
    application.add_handler(CommandHandler("help", help))
    application.add_handler(CommandHandler("download", download))
    application.add_handler(CommandHandler("music", music))
    application.add_handler(MessageHandler(filters.TEXT & ~filters.COMMAND, handle_text_messages))

    console_logger.info("Le bot a démarré avec succès!")
    await application.start()
    await application.idle()

if __name__ == "__main__":
    import asyncio
    asyncio.run(main())
