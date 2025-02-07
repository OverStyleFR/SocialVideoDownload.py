import os
import yt_dlp
import ffmpeg
from utils.logger import console_logger
from utils.file_manager import is_already_downloaded, save_download
from utils.helpers import get_default_keyboard
from utils.upload import upload_file
from config import FFMPEG_PATH  # Utilisation du chemin ffmpeg défini dans config.py

def music(update, context):
    args = context.args
    if not args:
        update.message.reply_text("Veuillez fournir un lien pour télécharger l'audio. Ex: /music <LIEN>", reply_markup=get_default_keyboard())
        console_logger.info("[MUSIC] Aucun lien fourni.")
        return

    url = args[0]
    console_logger.info(f"[MUSIC] Traitement de l'URL: {url}")

    # Options de téléchargement : on télécharge la vidéo en mp4
    ydl_opts = {
        'format': 'bestvideo+bestaudio/best',
        'outtmpl': 'downloads/%(title)s.%(ext)s',
    }

    # Vérifier si la vidéo a déjà été téléchargée via le hash
    if is_already_downloaded(url):
        console_logger.info(f"[MUSIC] Vidéo déjà téléchargée pour l'URL: {url}. Récupération du fichier...")
        try:
            with yt_dlp.YoutubeDL(ydl_opts) as ydl:
                info = ydl.extract_info(url, download=False)
                video_file = ydl.prepare_filename(info)
            console_logger.info(f"[MUSIC] Vidéo trouvée: {video_file}")
        except Exception as e:
            update.message.reply_text("Erreur lors de la récupération du fichier vidéo.", reply_markup=get_default_keyboard())
            console_logger.error(f"[MUSIC] Erreur récupération vidéo pour l'URL: {url} - {str(e)}")
            return
    else:
        try:
            console_logger.info(f"[MUSIC] Téléchargement de la vidéo pour l'URL: {url}")
            with yt_dlp.YoutubeDL(ydl_opts) as ydl:
                info = ydl.extract_info(url, download=True)
                video_file = ydl.prepare_filename(info)
            save_download(url)
            console_logger.info(f"[MUSIC] Téléchargement vidéo terminé: {video_file}")
        except Exception as e:
            update.message.reply_text("Erreur lors du téléchargement de la vidéo.", reply_markup=get_default_keyboard())
            console_logger.error(f"[MUSIC] Erreur téléchargement vidéo pour l'URL: {url} - {str(e)}")
            return

    # Définir le nom du fichier audio à générer (extension .mp3)
    audio_file = os.path.splitext(video_file)[0] + ".mp3"
    if os.path.exists(audio_file):
        console_logger.info(f"[MUSIC] Fichier audio déjà converti: {audio_file}")
    else:
        try:
            console_logger.info(f"[MUSIC] Conversion de {video_file} en audio {audio_file} via ffmpeg...")
            stream = ffmpeg.input(video_file)
            stream = ffmpeg.output(stream, audio_file, format='mp3', acodec='libmp3lame', audio_bitrate='192k')
            ffmpeg.run(stream, cmd=FFMPEG_PATH, quiet=True)
            console_logger.info(f"[MUSIC] Conversion terminée: {audio_file}")
        except Exception as e:
            update.message.reply_text("Erreur lors de la conversion en audio.", reply_markup=get_default_keyboard())
            console_logger.error(f"[MUSIC] Erreur conversion en audio pour {video_file} - {str(e)}")
            return

    try:
        console_logger.info(f"[MUSIC] Envoi du fichier audio: {audio_file}")
        upload_file(update, audio_file)
    except Exception as e:
        update.message.reply_text("Erreur lors de l'envoi du fichier audio.", reply_markup=get_default_keyboard())
        console_logger.error(f"[MUSIC] Erreur envoi audio pour {audio_file} - {str(e)}")
