from telegram import Update, KeyboardButton, ReplyKeyboardMarkup, ParseMode
from telegram.ext import CallbackContext
from config import BOT_VERSION, YOUR_NAME
from logger import console_logger

def start(update: Update, context: CallbackContext):
    user_name = update.message.from_user.first_name
    welcome_message = f"Bonjour {user_name} 👋\n\n Je suis un bot qui télécharge des vidéos/musiques."

    buttons = [[KeyboardButton(text="/start"), KeyboardButton(text="/help")]]
    markup = ReplyKeyboardMarkup(buttons, one_time_keyboard=True, resize_keyboard=True)

    update.message.reply_text(welcome_message, parse_mode=ParseMode.HTML, reply_markup=markup)
    console_logger.info(f"Command /start executed from {update.message.from_user.username}")

def help(update: Update, context: CallbackContext):
    paratext = f"Version {BOT_VERSION}\nDéveloppé par {YOUR_NAME}"

    buttons = [[KeyboardButton(text="/start"), KeyboardButton(text="/help")]]
    markup = ReplyKeyboardMarkup(buttons, one_time_keyboard=True, resize_keyboard=True)

    update.message.reply_text(
        f"Voici les commandes :\n/start - Démarrer\n/help - Aide\n/download [LIEN] - Télécharger vidéo\n/music [LIEN] - Télécharger musique\n\n<code>{paratext}</code>",
        parse_mode=ParseMode.HTML, reply_markup=markup
    )

    console_logger.info(f"Command /help executed from {update.message.from_user.username}")