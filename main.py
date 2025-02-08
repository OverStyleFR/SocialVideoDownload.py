from telegram.ext import Updater, CommandHandler, MessageHandler, Filters
from telegram import BotCommand
from commands.start import start
from commands.help import help_command
from commands.download import download
from commands.music import music  # Assure-toi que le fichier s'appelle bien music.py
from commands.auto_download import auto_download
from utils.file_manager import create_folders
from utils.token_loader import get_token
from utils.logger import console_logger

def main():
    console_logger.info("[INIT] Début de la réinitialisation des dossiers...")
    create_folders()
    
    token = get_token()
    updater = Updater(token, use_context=True)
    dp = updater.dispatcher

    dp.add_handler(CommandHandler("start", start))
    dp.add_handler(CommandHandler("help", help_command))
    dp.add_handler(CommandHandler("download", download))
    dp.add_handler(CommandHandler("music", music))
    dp.add_handler(MessageHandler(Filters.text & ~Filters.command, auto_download))
    
    # Définir le menu des commandes pour Telegram
    bot = updater.bot
    bot.set_my_commands([
        BotCommand("start", "Pour commencer"),
        BotCommand("help", "Pour obtenir de l'aide"),
        BotCommand("download", "Télécharger une vidéo avec yt-dlp"),
        BotCommand("music", "Télécharger de la musique avec yt-dlp")
    ])
    console_logger.info("[INIT] Menu des commandes configuré.")
    
    console_logger.info("[INIT] Démarrage du bot et lancement du polling...")
    updater.start_polling()
    updater.idle()

if __name__ == '__main__':
    main()
