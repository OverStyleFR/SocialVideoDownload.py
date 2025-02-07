import re
from utils.logger import console_logger
from commands.download import download as download_command

def auto_download(update, context):
    message_text = update.message.text
    url_pattern = r'(https?://[^\s]+)'
    urls = re.findall(url_pattern, message_text)
    if urls:
        context.args = [urls[0]]
        console_logger.info(f"URL détectée dans le message : {urls[0]}, lancement du téléchargement automatique.")
        download_command(update, context)
