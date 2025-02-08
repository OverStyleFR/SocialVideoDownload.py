import yt_dlp
from utils.logger import console_logger
from utils.file_manager import is_already_downloaded, save_download
from utils.upload import upload_file

def download(update, context):
    args = context.args
    if not args:
        update.message.reply_text("Veuillez fournir un lien pour le téléchargement. Ex: /download <LIEN>")
        console_logger.info(f"[DOWNLOAD] Aucun lien fourni par {update.message.from_user.username}.")
        return

    url = args[0]
    console_logger.info(f"[DOWNLOAD] Traitement de l'URL: {url} par {update.message.from_user.username}")
    ydl_opts = {
        'outtmpl': 'downloads/%(title)s.%(ext)s',
    }

    # Envoyer un message de confirmation que la demande a été prise en compte
    confirm_msg = update.message.reply_text("Téléchargement en cours...")

    if is_already_downloaded(url):
        console_logger.info(f"[DOWNLOAD] Fichier déjà téléchargé pour l'URL: {url} par {update.message.from_user.username}. Récupération du fichier...")
        try:
            with yt_dlp.YoutubeDL(ydl_opts) as ydl:
                info = ydl.extract_info(url, download=False)
                filename = ydl.prepare_filename(info)
            upload_file(update, filename)
            # Supprimer le message de confirmation une fois le fichier envoyé
            context.bot.delete_message(chat_id=update.message.chat_id, message_id=confirm_msg.message_id)
            console_logger.info(f"[DOWNLOAD] Fichier envoyé pour l'URL: {url} par {update.message.from_user.username}")
        except Exception as e:
            update.message.reply_text("Erreur lors de la récupération du fichier.")
            console_logger.error(f"[DOWNLOAD] Erreur récupération fichier pour l'URL: {url} par {update.message.from_user.username} - {str(e)}")
        return

    max_attempts = 3
    attempts = 0
    while attempts < max_attempts:
        try:
            console_logger.info(f"[DOWNLOAD] Tentative {attempts + 1} de téléchargement pour l'URL: {url} par {update.message.from_user.username}")
            with yt_dlp.YoutubeDL(ydl_opts) as ydl:
                info = ydl.extract_info(url, download=True)
                filename = ydl.prepare_filename(info)
            save_download(url)
            console_logger.info(f"[DOWNLOAD] Téléchargement terminé pour l'URL: {url} par {update.message.from_user.username}. Envoi du fichier...")
            upload_file(update, filename)
            context.bot.delete_message(chat_id=update.message.chat_id, message_id=confirm_msg.message_id)
            break  # Succès, sortir de la boucle
        except Exception as e:
            attempts += 1
            console_logger.error(f"[DOWNLOAD] Tentative {attempts} échouée pour l'URL: {url} par {update.message.from_user.username} - {str(e)}")
            if attempts >= max_attempts:
                update.message.reply_text("Erreur lors du téléchargement après plusieurs tentatives.")
                context.bot.delete_message(chat_id=update.message.chat_id, message_id=confirm_msg.message_id)
