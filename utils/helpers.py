from telegram import KeyboardButton, ReplyKeyboardMarkup

def get_default_keyboard():
    buttons = [
        [KeyboardButton(text="/start"), KeyboardButton(text="/help")],
        [KeyboardButton(text="/download"), KeyboardButton(text="/music")]
    ]
    return ReplyKeyboardMarkup(buttons, one_time_keyboard=True, resize_keyboard=True)
