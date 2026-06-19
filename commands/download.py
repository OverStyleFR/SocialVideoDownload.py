# commands/download.py
import os
import yt_dlp
from utils.logger import console_logger
from utils.file_manager import is_already_downloaded, save_download
from utils.disk_manager import check_and_clean_if_needed
from utils.retention import set_retention
from utils.cache import add_to_cache
from utils.upload import upload_file


def _edit_progress(bot, chat_id, msg_id, text):
    try:
        bot.edit_message_text(chat_id=chat_id, message_id=msg_id, text=text)
    except Exception:
        pass


def download(update, context):
    args = context.args
    if not args:
        update.message.reply_text("Veuillez fournir un lien pour le téléchargement. Ex: /download <LIEN>")
        console_logger.info(f"[DOWNLOAD] Aucun lien fourni par {update.message.from_user.username}.")
        return

    url = args[0]
    chat_id = update.message.chat_id
    bot = context.bot

    check_and_clean_if_needed()

    console_logger.info(f"[DOWNLOAD] Traitement de l'URL: {url} par {update.message.from_user.username}")

    progress_msg = update.message.reply_text(
        "⏳ Téléchargement en cours...",
        reply_to_message_id=update.message.message_id
    )
    progress_msg_id = progress_msg.message_id
    ydl_opts = {'outtmpl': 'downloads/%(title)s.%(ext)s'}

    should_download = True
    filename = None

    if is_already_downloaded(url):
        console_logger.info(f"[DOWNLOAD] Fichier déjà téléchargé pour l'URL: {url} par {update.message.from_user.username}. Vérification du fichier...")
        try:
            with yt_dlp.YoutubeDL(ydl_opts) as ydl:
                info = ydl.extract_info(url, download=False)
                filename = ydl.prepare_filename(info)
            if os.path.exists(filename):
                should_download = False
                set_retention(filename)
            else:
                console_logger.warning(f"[DOWNLOAD] Fichier manquant malgré hash pour l'URL: {url}. Retéléchargement...")
        except Exception as e:
            console_logger.error(f"[DOWNLOAD] Erreur récupération infos pour l'URL: {url} - {str(e)}")

    if should_download:
        max_attempts = 3
        attempts = 0
        while attempts < max_attempts:
            try:
                console_logger.info(f"[DOWNLOAD] Tentative {attempts + 1} de téléchargement pour l'URL: {url} par {update.message.from_user.username}")
                with yt_dlp.YoutubeDL(ydl_opts) as ydl:
                    info = ydl.extract_info(url, download=True)
                    filename = ydl.prepare_filename(info)
                save_download(url)
                set_retention(filename)
                add_to_cache(url, os.path.getsize(filename))
                break
            except Exception as e:
                attempts += 1
                console_logger.error(f"[DOWNLOAD] Tentative {attempts} échouée pour l'URL: {url} par {update.message.from_user.username} - {str(e)}")
                if attempts >= max_attempts:
                    _edit_progress(bot, chat_id, progress_msg_id, "❌ Échec du téléchargement après plusieurs tentatives.")
                    return

    _edit_progress(bot, chat_id, progress_msg_id, "📤 Envoi en cours... 0%")
    upload_file(update, filename, context, progress_msg_id=progress_msg_id)
