import os
import hashlib
import shutil
from utils.logger import console_logger

DOWNLOADS_DIR = "downloads"
HASH_FILE = os.path.join(DOWNLOADS_DIR, "hashes.txt")

def create_folders():
    console_logger.info("[FILE_MANAGER] Réinitialisation du dossier downloads...")
    if os.path.exists(DOWNLOADS_DIR):
        shutil.rmtree(DOWNLOADS_DIR)
        console_logger.info("[FILE_MANAGER] Dossier downloads supprimé.")
    os.makedirs(DOWNLOADS_DIR)
    console_logger.info("[FILE_MANAGER] Dossier downloads recréé.")
    
    if not os.path.exists("logs"):
        os.makedirs("logs")
        console_logger.info("[FILE_MANAGER] Dossier logs créé.")

def compute_hash(url):
    hash_value = hashlib.sha256(url.encode('utf-8')).hexdigest()
    console_logger.debug(f"[FILE_MANAGER] Hash calculé pour l'URL: {url} -> {hash_value}")
    return hash_value

def is_already_downloaded(url):
    hash_url = compute_hash(url)
    if not os.path.exists(HASH_FILE):
        console_logger.debug("[FILE_MANAGER] Aucun fichier hash trouvé. Pas de téléchargement précédent.")
        return False
    with open(HASH_FILE, "r") as f:
        hashes = f.read().splitlines()
    exists = hash_url in hashes
    console_logger.debug(f"[FILE_MANAGER] Vérification du hash pour l'URL: {url} - Existe: {exists}")
    return exists

def save_download(url):
    hash_url = compute_hash(url)
    with open(HASH_FILE, "a") as f:
        f.write(hash_url + "\n")
    console_logger.info(f"[FILE_MANAGER] Hash enregistré pour l'URL: {url}")
