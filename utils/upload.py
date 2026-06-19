# utils/upload.py
import os
from utils.logger import console_logger
from utils.progress_file import ProgressFile


def _edit_progress(bot, chat_id, msg_id, text):
    try:
        bot.edit_message_text(chat_id=chat_id, message_id=msg_id, text=text)
    except Exception:
        pass


def upload_file(update, file_path, context, progress_msg_id=None, from_cache=False):
    if not os.path.exists(file_path):
        update.message.reply_text("Erreur : Fichier non trouvé.")
        console_logger.error(f"[UPLOAD] Fichier non trouvé: {file_path}")
        return

    chat_id = update.message.chat_id
    bot = context.bot

    MAX_FILE_SIZE = 35 * 1024 * 1024
    file_size = os.path.getsize(file_path)
    caption = "📦 Envoyé depuis le cache" if from_cache else None

    if file_size > MAX_FILE_SIZE:
        console_logger.info(f"[UPLOAD] Fichier '{file_path}' trop volumineux ({file_size} octets). Upload externe via curl.libriciel.fr.")
        if progress_msg_id is None:
            progress_msg = update.message.reply_text("Upload externe en cours : 0% ⏳")
            progress_msg_id = progress_msg.message_id
        else:
            _edit_progress(bot, chat_id, progress_msg_id, "Upload externe en cours : 0% ⏳")

        def progress_callback(percent):
            _edit_progress(bot, chat_id, progress_msg_id,
                           f"Upload externe en cours : {percent}% ⏳")

        try:
            from utils.curl_uploader import upload_large_file_via_curl
            shareable_url = upload_large_file_via_curl(file_path, progress_callback=progress_callback)
            bot.delete_message(chat_id=chat_id, message_id=progress_msg_id)
            update.message.reply_text(
                f"Le fichier est trop volumineux pour être envoyé directement par Telegram.\n"
                f"Veuillez le télécharger ici : {shareable_url}"
            )
            console_logger.info(f"[UPLOAD] Upload externe réussi pour '{file_path}' -> {shareable_url}")
        except Exception as e:
            bot.delete_message(chat_id=chat_id, message_id=progress_msg_id)
            update.message.reply_text(
                "Erreur lors de l'upload externe du fichier.\nVeuillez uploader manuellement via https://curl.libriciel.fr/"
            )
            console_logger.error(f"[UPLOAD] Erreur upload externe pour '{file_path}': {str(e)}")
        return

    # Envoi direct via Telegram avec progression
    ext = os.path.splitext(file_path)[1].lower()
    if progress_msg_id is not None:
        _edit_progress(bot, chat_id, progress_msg_id, "📤 Envoi en cours... 0%")

    try:
        progress_file = ProgressFile(
            file_path,
            progress_interval=10,
            callback=lambda p: _edit_progress(bot, chat_id, progress_msg_id,
                                               f"📤 Envoi en cours... {p}%") if progress_msg_id else None
        )
        if ext in [".mp4", ".mkv", ".avi"]:
            update.message.reply_video(video=progress_file,
                                       caption=caption,
                                       reply_to_message_id=update.message.message_id)
            console_logger.info(f"[UPLOAD] Vidéo envoyée : {file_path}")
        elif ext in [".mp3", ".wav"]:
            update.message.reply_audio(audio=progress_file,
                                       caption=caption,
                                       reply_to_message_id=update.message.message_id)
            console_logger.info(f"[UPLOAD] Audio envoyé : {file_path}")
        else:
            update.message.reply_document(document=progress_file,
                                          caption=caption,
                                          reply_to_message_id=update.message.message_id)
            console_logger.info(f"[UPLOAD] Document envoyé : {file_path}")
        if progress_msg_id is not None:
            bot.delete_message(chat_id=chat_id, message_id=progress_msg_id)
    except Exception as e:
        if progress_msg_id is not None:
            bot.delete_message(chat_id=chat_id, message_id=progress_msg_id)
        update.message.reply_text("Erreur lors de l'envoi du fichier.")
        console_logger.error(f"[UPLOAD] Erreur lors de l'envoi du fichier '{file_path}': {str(e)}")
