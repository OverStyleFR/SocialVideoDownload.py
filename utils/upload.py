import os
from utils.logger import console_logger

def upload_file(update, file_path):
    if not os.path.exists(file_path):
        update.message.reply_text("Erreur : Fichier non trouvé.")
        console_logger.error(f"[UPLOAD] Fichier non trouvé: {file_path}")
        return
    
    ext = os.path.splitext(file_path)[1].lower()
    try:
        with open(file_path, "rb") as f:
            if ext in [".mp4", ".mkv", ".avi"]:
                update.message.reply_video(video=f, reply_to_message_id=update.message.message_id)
                console_logger.info(f"[UPLOAD] Vidéo envoyée: {file_path}")
            elif ext in [".mp3", ".wav"]:
                update.message.reply_audio(audio=f, reply_to_message_id=update.message.message_id)
                console_logger.info(f"[UPLOAD] Audio envoyé: {file_path}")
            else:
                update.message.reply_document(document=f, reply_to_message_id=update.message.message_id)
                console_logger.info(f"[UPLOAD] Document envoyé: {file_path}")
    except Exception as e:
        update.message.reply_text("Erreur lors de l'envoi du fichier.")
        console_logger.error(f"[UPLOAD] Erreur lors de l'envoi du fichier {file_path} - {str(e)}")
