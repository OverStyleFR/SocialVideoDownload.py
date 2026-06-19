# commands/music.py
import os
import yt_dlp
import ffmpeg
from utils.logger import console_logger
from utils.file_manager import is_already_downloaded, save_download
from utils.retention import set_retention
from utils.cache import add_to_cache, record_cache_hit
from utils.upload import upload_file
from config import FFMPEG_PATH


def _edit_progress(bot, chat_id, msg_id, text):
    try:
        bot.edit_message_text(chat_id=chat_id, message_id=msg_id, text=text)
    except Exception:
        pass


def music(update, context):
    args = context.args
    if not args:
        update.message.reply_text("Veuillez fournir un lien pour télécharger l'audio. Ex: /music <LIEN>")
        console_logger.info(f"[MUSIC] Aucun lien fourni par {update.message.from_user.username}.")
        return

    url = args[0]
    chat_id = update.message.chat_id
    bot = context.bot

    console_logger.info(f"[MUSIC] Traitement de l'URL: {url} par {update.message.from_user.username}")

    progress_msg = update.message.reply_text(
        "⏳ Téléchargement vidéo en cours...",
        reply_to_message_id=update.message.message_id
    )
    progress_msg_id = progress_msg.message_id
    ydl_opts = {'outtmpl': 'downloads/%(title)s.%(ext)s'}

    should_download = True
    from_cache = False
    video_file = None

    if is_already_downloaded(url):
        console_logger.info(f"[MUSIC] Vidéo déjà téléchargée pour l'URL: {url} par {update.message.from_user.username}. Vérification du fichier...")
        try:
            with yt_dlp.YoutubeDL(ydl_opts) as ydl:
                info = ydl.extract_info(url, download=False)
                video_file = ydl.prepare_filename(info)
            if os.path.exists(video_file):
                should_download = False
                from_cache = True
                set_retention(video_file)
                add_to_cache(url, os.path.getsize(video_file))
                record_cache_hit(url)
                _edit_progress(bot, chat_id, progress_msg_id, "📦 Utilisation du cache...")
            else:
                console_logger.warning(f"[MUSIC] Vidéo manquante malgré hash pour l'URL: {url}. Retéléchargement...")
        except Exception as e:
            console_logger.error(f"[MUSIC] Erreur récupération infos pour l'URL: {url} - {str(e)}")
            _edit_progress(bot, chat_id, progress_msg_id, "❌ Erreur lors de la récupération de la vidéo.")
            return

    if should_download:
        max_attempts = 3
        attempts = 0
        while attempts < max_attempts:
            try:
                console_logger.info(f"[MUSIC] Tentative {attempts + 1} de téléchargement pour l'URL: {url} par {update.message.from_user.username}")
                with yt_dlp.YoutubeDL(ydl_opts) as ydl:
                    info = ydl.extract_info(url, download=True)
                    video_file = ydl.prepare_filename(info)
                save_download(url)
                set_retention(video_file)
                add_to_cache(url, os.path.getsize(video_file))
                break
            except Exception as e:
                attempts += 1
                console_logger.error(f"[MUSIC] Tentative {attempts} échouée pour l'URL: {url} par {update.message.from_user.username} - {str(e)}")
                if attempts >= max_attempts:
                    _edit_progress(bot, chat_id, progress_msg_id, "❌ Échec du téléchargement après plusieurs tentatives.")
                    return

    _edit_progress(bot, chat_id, progress_msg_id, "🔄 Conversion audio...")

    audio_file = os.path.splitext(video_file)[0] + ".mp3"
    if os.path.exists(audio_file):
        console_logger.info(f"[MUSIC] Fichier audio déjà converti: {audio_file}")
    else:
        try:
            stream = ffmpeg.input(video_file)
            stream = ffmpeg.output(stream, audio_file, format='mp3', acodec='libmp3lame', audio_bitrate='192k')
            ffmpeg.run(stream, cmd=FFMPEG_PATH, quiet=True)
            set_retention(audio_file)
            add_to_cache(url + "#audio", os.path.getsize(audio_file))
            console_logger.info(f"[MUSIC] Conversion terminée: {audio_file} pour {update.message.from_user.username}")
        except Exception as e:
            _edit_progress(bot, chat_id, progress_msg_id, "❌ Erreur lors de la conversion en audio.")
            console_logger.error(f"[MUSIC] Erreur conversion en audio pour {video_file} par {update.message.from_user.username} - {str(e)}")
            return

    _edit_progress(bot, chat_id, progress_msg_id, "📤 Envoi en cours... 0%")
    upload_file(update, audio_file, context, progress_msg_id=progress_msg_id, from_cache=from_cache)
