import yt_dlp
from utils.logger import console_logger
from utils.file_manager import is_already_downloaded, save_download
from utils.helpers import get_default_keyboard
from utils.upload import upload_file

def download(update, context):
    args = context.args
    if not args:
        update.message.reply_text("Veuillez fournir un lien pour le téléchargement. Ex: /download <lien>", reply_markup=get_default_keyboard())
        console_logger.info("[DOWNLOAD] Aucun lien fourni.")
        return

    url = args[0]
    console_logger.info(f"[DOWNLOAD] Traitement de l'URL: {url}")
    ydl_opts = {
        'outtmpl': 'downloads/%(title)s.%(ext)s',
    }
    
    if is_already_downloaded(url):
        console_logger.info(f"[DOWNLOAD] Fichier déjà téléchargé pour l'URL: {url}. Récupération du fichier...")
        try:
            with yt_dlp.YoutubeDL(ydl_opts) as ydl:
                info = ydl.extract_info(url, download=False)
                filename = ydl.prepare_filename(info)
            upload_file(update, filename)
            console_logger.info(f"[DOWNLOAD] Fichier envoyé pour l'URL: {url}")
        except Exception as e:
            update.message.reply_text("Erreur lors de la récupération du fichier.", reply_markup=get_default_keyboard())
            console_logger.error(f"[DOWNLOAD] Erreur récupération fichier pour l'URL: {url} - {str(e)}")
        return

    try:
        console_logger.info(f"[DOWNLOAD] Téléchargement de la vidéo pour l'URL: {url}")
        with yt_dlp.YoutubeDL(ydl_opts) as ydl:
            info = ydl.extract_info(url, download=True)
            filename = ydl.prepare_filename(info)
        save_download(url)
        console_logger.info(f"[DOWNLOAD] Téléchargement vidéo terminé pour l'URL: {url}. Envoi du fichier...")
        upload_file(update, filename)
    except Exception as e:
        update.message.reply_text("Erreur lors du téléchargement.", reply_markup=get_default_keyboard())
        console_logger.error(f"[DOWNLOAD] Erreur téléchargement vidéo pour l'URL: {url} - {str(e)}")
