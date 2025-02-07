import yt_dlp
from utils.logger import console_logger
from utils.file_manager import is_already_downloaded, save_download
from utils.helpers import get_default_keyboard
from utils.upload import upload_file

def music(update, context):
    args = context.args
    if not args:
        update.message.reply_text("Veuillez fournir un lien pour télécharger l'audio. Ex: /music <lien>", reply_markup=get_default_keyboard())
        console_logger.info("[MUSIC] Aucun lien fourni.")
        return

    url = args[0]
    console_logger.info(f"[MUSIC] Traitement de l'URL: {url}")
    ydl_opts = {
        'format': 'bestaudio/best',
        'outtmpl': 'downloads/%(title)s.mp3',   # Force l'extension mp3
        'postprocessors': [{
            'key': 'FFmpegExtractAudio',
            'preferredcodec': 'mp3',
            'preferredquality': '192',
        }],
    }

    if is_already_downloaded(url):
        console_logger.info(f"[MUSIC] Audio déjà téléchargé pour l'URL: {url}. Récupération du fichier...")
        try:
            with yt_dlp.YoutubeDL(ydl_opts) as ydl:
                info = ydl.extract_info(url, download=False)
                filename = ydl.prepare_filename(info)
                # On s'assure que l'extension est .mp3
                filename = filename.rsplit('.', 1)[0] + ".mp3"
            upload_file(update, filename)
            console_logger.info(f"[MUSIC] Fichier audio envoyé pour l'URL: {url}")
        except Exception as e:
            update.message.reply_text("Erreur lors de la récupération du fichier audio.", reply_markup=get_default_keyboard())
            console_logger.error(f"[MUSIC] Erreur récupération audio pour l'URL: {url} - {str(e)}")
        return

    try:
        console_logger.info(f"[MUSIC] Téléchargement de l'audio pour l'URL: {url}")
        with yt_dlp.YoutubeDL(ydl_opts) as ydl:
            info = ydl.extract_info(url, download=True)
            filename = ydl.prepare_filename(info)
            filename = filename.rsplit('.', 1)[0] + ".mp3"
        save_download(url)
        console_logger.info(f"[MUSIC] Téléchargement audio terminé pour l'URL: {url}. Envoi du fichier...")
        upload_file(update, filename)
    except Exception as e:
        update.message.reply_text("Erreur lors du téléchargement audio.", reply_markup=get_default_keyboard())
        console_logger.error(f"[MUSIC] Erreur téléchargement audio pour l'URL: {url} - {str(e)}")
