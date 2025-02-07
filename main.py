import logging
from telegram.ext import Updater, CommandHandler, MessageHandler, Filters
from config import *
from fonction.ft_token import get_token
from commands import start, help
from command.auto_download import download
from command.music_download import music
from command.start import start
from command.help import help
from command.download import download_and_send_video
from command.auto_download import download
from fonction.ft_save import save_result_to_file
from fonction.ft_handle_message import handle_text_messages
from logger import console_logger

def main():
    token = get_token()
    if token is None:
        console_logger.error("Token introuvable. Vérifiez token.txt.")
        return

    updater = Updater(token=token, use_context=True)
    dp = updater.dispatcher

    dp.add_handler(CommandHandler("start", start))
    dp.add_handler(CommandHandler("help", help))
    dp.add_handler(CommandHandler("download", download, pass_args=True))
    dp.add_handler(CommandHandler("music", music, pass_args=True))


    dp.add_handler(MessageHandler(Filters.text & ~Filters.command, handle_text_messages))

    updater.start_polling()
    
    print("Le bot est démarré et écoute les commandes...")
    console_logger.info("Le bot a démarré avec succès!")
    
    updater.idle()

if __name__ == "__main__":
    main()
