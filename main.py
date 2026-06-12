# main.py
import os
import threading
import time
from dotenv import load_dotenv
from telegram.ext import Updater, CommandHandler, MessageHandler, Filters
from telegram import BotCommand
from commands.start import start
from commands.help import help_command
from commands.download import download
from commands.music import music
from commands.stats import stats
from commands.auto_download import auto_download
from utils.cache import load_cache
from utils.token_loader import get_token
from config import CLEANUP_INTERVAL_HOURS
from utils.disk_manager import clear_downloads, check_and_clean_if_needed
from utils.logger import console_logger

load_dotenv(".env", override=True)


def scheduled_cleanup():
    """Thread de nettoyage périodique du dossier downloads."""
    interval_seconds = CLEANUP_INTERVAL_HOURS * 3600
    console_logger.info(
        f"[CLEANUP] Rotation planifiée activée — nettoyage toutes les {CLEANUP_INTERVAL_HOURS}h."
    )
    while True:
        time.sleep(interval_seconds)
        console_logger.info("[CLEANUP] Nettoyage périodique du dossier downloads...")
        clear_downloads()
        console_logger.info("[CLEANUP] Nettoyage périodique terminé.")


def main():
    console_logger.info("[INIT] Début de la réinitialisation des dossiers...")
    load_cache()

    # Vérification de l'espace disque au démarrage
    check_and_clean_if_needed()

    token = get_token()
    updater = Updater(token, use_context=True)
    dp = updater.dispatcher

    # Enregistrement des handlers pour les commandes du bot
    dp.add_handler(CommandHandler("start", start))
    dp.add_handler(CommandHandler("help", help_command))
    dp.add_handler(CommandHandler("download", download))
    dp.add_handler(CommandHandler("music", music))
    dp.add_handler(CommandHandler("stats", stats))

    # Handler pour les messages contenant des liens (auto-download)
    dp.add_handler(MessageHandler(Filters.text & ~Filters.command, auto_download))

    # Configuration du menu des commandes pour Telegram
    bot = updater.bot
    bot.set_my_commands([
        BotCommand("start", "Pour commencer"),
        BotCommand("help", "Pour obtenir de l'aide"),
        BotCommand("download", "Télécharger une vidéo avec yt-dlp"),
    ])
    console_logger.info("[INIT] Menu des commandes configuré.")

    # Démarrage du thread de nettoyage périodique (daemon = s'arrête avec le bot)
    cleanup_thread = threading.Thread(target=scheduled_cleanup, daemon=True)
    cleanup_thread.start()

    console_logger.info("[INIT] Démarrage du bot et lancement du polling...")
    updater.start_polling()
    updater.idle()


if __name__ == '__main__':
    main()
