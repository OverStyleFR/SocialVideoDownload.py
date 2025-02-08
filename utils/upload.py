# utils/upload.py
import os
from utils.logger import console_logger

def upload_file(update, file_path, context):
    """
    Envoie le fichier via Telegram si sa taille est < 35 Mo.
    Sinon, le fichier est uploadé via curl.libriciel.fr à l'aide
    de upload_large_file_via_curl() et l'URL de téléchargement est renvoyée à l'utilisateur.
    Un callback de progression met à jour un message Telegram tous les 10%.
    """
    if not os.path.exists(file_path):
        update.message.reply_text("Erreur : Fichier non trouvé.")
        console_logger.error(f"[UPLOAD] Fichier non trouvé: {file_path}")
        return

    MAX_FILE_SIZE = 35 * 1024 * 1024  # 35 Mo en octets
    file_size = os.path.getsize(file_path)
    if file_size > MAX_FILE_SIZE:
        console_logger.info(f"[UPLOAD] Fichier '{file_path}' trop volumineux ({file_size} octets). Upload externe via curl.libriciel.fr.")
        progress_msg = update.message.reply_text("Upload externe en cours : 0% ⏳")
        
        def progress_callback(percent):
            try:
                context.bot.edit_message_text(
                    chat_id=update.message.chat_id,
                    message_id=progress_msg.message_id,
                    text=f"Upload externe en cours : {percent}% ⏳"
                )
            except Exception:
                pass

        try:
            from utils.curl_uploader import upload_large_file_via_curl
            shareable_url = upload_large_file_via_curl(file_path, progress_callback=progress_callback)
            context.bot.delete_message(chat_id=update.message.chat_id,
                                       message_id=progress_msg.message_id)
            update.message.reply_text(
                f"Le fichier est trop volumineux pour être envoyé directement par Telegram.\n"
                f"Veuillez le télécharger ici : {shareable_url}"
            )
            console_logger.info(f"[UPLOAD] Upload externe réussi pour '{file_path}' -> {shareable_url}")
        except Exception as e:
            context.bot.delete_message(chat_id=update.message.chat_id,
                                       message_id=progress_msg.message_id)
            update.message.reply_text("Erreur lors de l'upload externe du fichier.")
            console_logger.error(f"[UPLOAD] Erreur upload externe pour '{file_path}': {str(e)}")
        return

    # Sinon, envoyer le fichier via l'API Telegram
    ext = os.path.splitext(file_path)[1].lower()
    try:
        with open(file_path, "rb") as f:
            if ext in [".mp4", ".mkv", ".avi"]:
                update.message.reply_video(video=f, reply_to_message_id=update.message.message_id)
                console_logger.info(f"[UPLOAD] Vidéo envoyée : {file_path}")
            elif ext in [".mp3", ".wav"]:
                update.message.reply_audio(audio=f, reply_to_message_id=update.message.message_id)
                console_logger.info(f"[UPLOAD] Audio envoyé : {file_path}")
            else:
                update.message.reply_document(document=f, reply_to_message_id=update.message.message_id)
                console_logger.info(f"[UPLOAD] Document envoyé : {file_path}")
    except Exception as e:
        update.message.reply_text("Erreur lors de l'envoi du fichier.")
        console_logger.error(f"[UPLOAD] Erreur lors de l'envoi du fichier '{file_path}': {str(e)}")
