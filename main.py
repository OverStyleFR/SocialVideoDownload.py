import subprocess
import os
import logging
import urllib3
import hashlib

from datetime import datetime
from telegram import InputFile, ReplyKeyboardMarkup, KeyboardButton, ParseMode
from telegram.ext import Updater, CommandHandler, MessageHandler, Filters

# Dossier de téléchargement temporaire
download_temp_folder = "download_temp"

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

# Ajouter un formatteur pour inclure la date, le niveau de danger, et le message dans les logs
log_formatter = logging.Formatter('%(asctime)s - %(levelname)s - %(message)s')

# Ajouter le formatteur au gestionnaire de fichier
file_handler.setFormatter(log_formatter)

# Ajouter un gestionnaire de fichier au logger principal
logging.basicConfig(level=logging.INFO, handlers=[file_handler])

# Créer un gestionnaire de console et l'ajouter uniquement au logger de la console
console_handler = logging.StreamHandler()
console_handler.setLevel(logging.INFO)
console_handler.setFormatter(log_formatter)
console_logger.addHandler(console_handler)

BOT_VERSION = "V0.7-3"
YOUR_NAME = "Tom V. | OverStyleFR"

def main():
    token = read_token()

    if token is None:
        print("Le token n'a pas été trouvé. Assurez-vous que le fichier token.txt contient le token.")
        return


    # Vérifier si le dossier 'download_temp' existe
    folder_path = 'download_temp'
    if os.path.exists(folder_path):
        # Supprimer le contenu du dossier s'il existe
        for file_name in os.listdir(folder_path):
            file_path = os.path.join(folder_path, file_name)
            try:
                if os.path.isfile(file_path):
                    os.unlink(file_path)
            except Exception as e:
                print(e)
        console_logger.info('Folder exist. All files as been deleted.')
    else:
        # Créer le dossier s'il n'existe pas
        os.makedirs(folder_path)
        console_logger.info("Folder Create")


    updater = Updater(token=token, use_context=True)
    dp = updater.dispatcher

    dp.add_handler(CommandHandler("start", start))
    dp.add_handler(CommandHandler("help", help))
    dp.add_handler(CommandHandler("download", download, pass_args=True))
    dp.add_handler(CommandHandler("music", music, pass_args=True))

    dp.add_handler(MessageHandler(Filters.text & ~Filters.command, handle_text_messages))

    updater.start_polling()

    console_logger.info("Le bot a démarré avec succès!")
    updater.idle()

if __name__ == "__main__":
    main()