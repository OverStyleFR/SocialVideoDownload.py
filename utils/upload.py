import os
from utils.logger import console_logger
from utils.progress_file import ProgressFile

def upload_file(update, file_path):
    if not os.path.exists(file_path):
        update.message.reply_text("Erreur : Fichier non trouvé.")
        console_logger.error(f"[UPLOAD] Fichier non trouvé: {file_path}")
        return

    ext = os.path.splitext(file_path)[1].lower()
    max_attempts = 3
    attempts = 0
    success = False
    while attempts < max_attempts and not success:
        try:
            progress_file = ProgressFile(file_path)
            # Selon le type de fichier, on envoie via la bonne méthode
            if ext in [".mp4", ".mkv", ".avi"]:
                update.message.reply_video(video=progress_file, reply_to_message_id=update.message.message_id)
                console_logger.info(f"[UPLOAD] Vidéo envoyée: {file_path}")
            elif ext in [".mp3", ".wav"]:
                update.message.reply_audio(audio=progress_file, reply_to_message_id=update.message.message_id)
                console_logger.info(f"[UPLOAD] Audio envoyé: {file_path}")
            else:
                update.message.reply_document(document=progress_file, reply_to_message_id=update.message.message_id)
                console_logger.info(f"[UPLOAD] Document envoyé: {file_path}")
            progress_file.close()
            success = True
        except Exception as e:
            attempts += 1
            console_logger.error(f"[UPLOAD] Tentative {attempts} échouée pour l'envoi du fichier {file_path} - {str(e)}")
            if attempts >= max_attempts:
                update.message.reply_text("Erreur lors de l'envoi du fichier après plusieurs tentatives.")
