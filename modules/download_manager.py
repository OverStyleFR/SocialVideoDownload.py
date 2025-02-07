import os
import hashlib
import subprocess
from telegram import InputFile
from logger import console_logger
from config import DOWNLOAD_TEMP_FOLDER

def download_video(bot, chat_id, link):
    link_hash = hashlib.md5(link.encode()).hexdigest()
    video_path = os.path.join(DOWNLOAD_TEMP_FOLDER, f"{link_hash}.mp4")

    if os.path.exists(video_path):
        bot.send_video(chat_id=chat_id, video=InputFile(video_path), caption="Vidéo déjà téléchargée.")
        console_logger.info(f"Vidéo envoyée depuis le cache: {link}")
        return

    try:
        result = subprocess.run(["./yt-dlp", "--format", "best", "-o", video_path, link], capture_output=True, text=True, check=True)

        if result.returncode == 0 and os.path.exists(video_path):
            bot.send_video(chat_id=chat_id, video=InputFile(video_path), caption="Téléchargement terminé.")
            console_logger.info(f"Vidéo téléchargée et envoyée: {link}")
        else:
            bot.send_message(chat_id=chat_id, text="Échec du téléchargement.")
            console_logger.error(f"Échec du téléchargement: {link}")

    except subprocess.CalledProcessError as e:
        bot.send_message(chat_id=chat_id, text=f"Erreur: {str(e)}")
        console_logger.error(f"Erreur yt-dlp: {str(e)}")
