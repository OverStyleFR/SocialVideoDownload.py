import logging
import os

class ColoredFormatter(logging.Formatter):
    COLORS = {
        'DEBUG': '\x1b[34m',    # bleu
        'INFO': '\x1b[32m',     # vert
        'WARNING': '\x1b[33m',  # jaune
        'ERROR': '\x1b[31m',    # rouge
        'CRITICAL': '\x1b[41m', # fond rouge
    }
    RESET = '\x1b[0m'
    
    def format(self, record):
        color = self.COLORS.get(record.levelname, self.RESET)
        record.levelname = f"{color}{record.levelname}{self.RESET}"
        return super().format(record)

if not os.path.exists("logs"):
    os.makedirs("logs")

console_logger = logging.getLogger("TelegramBot")
console_logger.setLevel(logging.DEBUG)

console_handler = logging.StreamHandler()
file_handler = logging.FileHandler("logs/bot.log", encoding="utf-8")

console_handler.setLevel(logging.DEBUG)
file_handler.setLevel(logging.DEBUG)

formatter = ColoredFormatter("%(asctime)s - %(levelname)s - %(message)s")
console_handler.setFormatter(formatter)
file_handler.setFormatter(formatter)

console_logger.addHandler(console_handler)
console_logger.addHandler(file_handler)
