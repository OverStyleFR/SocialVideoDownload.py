# utils/logger.py
import logging
import os
from datetime import datetime

try:
    os.makedirs("logs", exist_ok=True)
except OSError:
    pass
log_filename = os.path.join("logs", f"{datetime.now().strftime('%Y-%m-%d')}.log")

class ColoredFormatter(logging.Formatter):
    # Couleurs spécifiques pour des catégories
    CATEGORY_COLORS = {
        "[DOWNLOAD]": "\x1b[34m",          # bleu
        "[MUSIC]": "\x1b[35m",             # magenta
        "[AUTO_DOWNLOAD]": "\x1b[32m",       # vert
        "[UPLOAD]": "\x1b[33m",            # jaune
        "[CURL UPLOAD]": "\x1b[36m",       # cyan
        "[CURL UPLOAD PROGRESS]": "\x1b[31m" # rouge
    }
    # Couleurs par niveau si aucune catégorie n'est présente dans le préfixe
    LEVEL_COLORS = {
        "DEBUG": "\x1b[34m",   # bleu
        "INFO": "\x1b[32m",    # vert
        "WARNING": "\x1b[33m", # jaune
        "ERROR": "\x1b[31m",   # rouge
        "CRITICAL": "\x1b[41m" # fond rouge
    }
    RESET = "\x1b[0m"
    
    def format(self, record):
        original_msg = record.getMessage()
        category = None
        remainder = original_msg
        # Vérifier si le message commence par une catégorie connue
        for key, cat_color in self.CATEGORY_COLORS.items():
            if original_msg.startswith(key):
                category = key
                remainder = original_msg[len(key):].lstrip()
                break
        level_color = self.LEVEL_COLORS.get(record.levelname, "")
        if category:
            colored_category = f"{self.CATEGORY_COLORS[category]}{category}{self.RESET}"
            colored_remainder = f"{level_color}{remainder}{self.RESET}"
            record.msg = f"{colored_category} {colored_remainder}"
        else:
            record.msg = f"{level_color}{original_msg}{self.RESET}"
        return super().format(record)

console_logger = logging.getLogger("TelegramBot")
console_logger.setLevel(logging.DEBUG)

console_handler = logging.StreamHandler()
console_handler.setLevel(logging.DEBUG)
formatter = ColoredFormatter("%(asctime)s - %(levelname)s - %(message)s")
console_handler.setFormatter(formatter)
console_logger.addHandler(console_handler)

try:
    file_handler = logging.FileHandler(log_filename, encoding="utf-8", mode="a")
    file_handler.setLevel(logging.DEBUG)
    file_handler.setFormatter(formatter)
    console_logger.addHandler(file_handler)
except OSError:
    pass
