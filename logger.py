import logging
import os
from datetime import datetime

# Création du dossier logs
if not os.path.exists("logs"):
    os.makedirs("logs")

# Configuration du logging
log_file_name = f"logs/bot_{datetime.now().strftime('%Y%m%d_%H%M%S')}.log"
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s',
    handlers=[
        logging.FileHandler(log_file_name),
        logging.StreamHandler()
    ]
)

console_logger = logging.getLogger("bot_logger")