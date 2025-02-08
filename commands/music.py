import os
import yt_dlp
import ffmpeg
from utils.logger import console_logger
from utils.file_manager import is_already_downloaded, save_download
from utils.upload import upload_file
from config import FFMPEG_PATH

def music(update, context):
    args = context.args
    if not args:
        update.message.reply_text("Veuillez fournir un lien pour télécharger l'audio. Ex: /music <LIEN>")
        console_logger.info(f"[MUSIC] Aucun lien fourni par {update.message.from_user.username}.")
        return

    url = args[0]
    console_logger.info(f"[MUSIC] Traitement de l'URL: {url} par {update.message.from_user.username}")

    ydl_opts = {
        'format': 'bestvideo+bestaudio/best',
        'outtmpl': 'downloads/%(title)s.%(ext)s',
    }

    if is_already_downloaded(url):
        console_logger.info(f"[MUSIC] Vidéo déjà téléchargée pour l'URL: {url} par {update.message.from_user.username}. Récupération du fichier...")
        try:
            with yt_dlp.YoutubeDL(ydl_opts) as ydl:
                info = ydl.extract_info(url, download=False)
                video_file = ydl.prepare_filename(info)
            console_logger.info(f"[MUSIC] Vidéo trouvée: {video_file}")
        except Exception as e:
            update.message.reply_text("Erreur lors de la récupération du fichier vidéo.")
            console_logger.error(f"[MUSIC] Erreur récupération vidéo pour l'URL: {url} par {update.message.from_user.username} - {str(e)}")
            return
    else:
        try:
            console_logger.info(f"[MUSIC] Téléchargement de la vidéo pour l'URL: {url} par {update.message.from_user.username}")
            with yt_dlp.YoutubeDL(ydl_opts) as ydl:
                info = ydl.extract_info(url, download=True)
                video_file = ydl.prepare_filename(info)
            save_download(url)
            console_logger.info(f"[MUSIC] Téléchargement terminé: {video_file} par {update.message.from_user.username}")
        except Exception as e:
            update.message.reply_text("Erreur lors du téléchargement de la vidéo.")
            console_logger.error(f"[MUSIC] Erreur téléchargement vidéo pour l'URL: {url} par {update.message.from_user.username} - {str(e)}")
            return

    audio_file = os.path.splitext(video_file)[0] + ".mp3"
    if os.path.exists(audio_file):
        console_logger.info(f"[MUSIC] Fichier audio déjà converti: {audio_file}")
    else:
        try:
            console_logger.info(f"[MUSIC] Conversion de {video_file} en audio {audio_file} via ffmpeg pour {update.message.from_user.username}...")
            stream = ffmpeg.input(video_file)
            stream = ffmpeg.output(stream, audio_file, format='mp3', acodec='libmp3lame', audio_bitrate='192k')
            ffmpeg.run(stream, cmd=FFMPEG_PATH, quiet=True)
            console_logger.info(f"[MUSIC] Conversion terminée: {audio_file} pour {update.message.from_user.username}")
        except Exception as e:
            update.message.reply_text("Erreur lors de la conversion en audio.")
            console_logger.error(f"[MUSIC] Erreur conversion en audio pour {video_file} par {update.message.from_user.username} - {str(e)}")
            return

    try:
        console_logger.info(f"[MUSIC] Envoi du fichier audio: {audio_file} pour {update.message.from_user.username}")
        upload_file(update, audio_file)
    except Exception as e:
        update.message.reply_text("Erreur lors de l'envoi du fichier audio.")
        console_logger.error(f"[MUSIC] Erreur envoi audio pour {audio_file} par {update.message.from_user.username} - {str(e)}")
