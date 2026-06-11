import os
import shutil
from config import MIN_FREE_SPACE_MB
from utils.logger import console_logger

DOWNLOADS_DIR = "downloads"


def get_free_space_mb() -> float:
    """Retourne l'espace disque libre en Mo sur la partition du dossier downloads."""
    stat = shutil.disk_usage(DOWNLOADS_DIR if os.path.exists(DOWNLOADS_DIR) else ".")
    return stat.free / (1024 * 1024)


def clear_downloads():
    """Vide le dossier downloads et recrée sa structure (conserve hashes.txt)."""
    hash_file = os.path.join(DOWNLOADS_DIR, "hashes.txt")
    hashes_backup = None

    # Sauvegarde des hashes avant suppression pour éviter les re-téléchargements
    if os.path.exists(hash_file):
        with open(hash_file, "r") as f:
            hashes_backup = f.read()

    if os.path.exists(DOWNLOADS_DIR):
        shutil.rmtree(DOWNLOADS_DIR)
        console_logger.info("[DISK_MANAGER] Dossier downloads vidé.")

    os.makedirs(DOWNLOADS_DIR)

    if hashes_backup is not None:
        with open(hash_file, "w") as f:
            f.write(hashes_backup)
        console_logger.info("[DISK_MANAGER] Fichier hashes.txt restauré après nettoyage.")


def check_and_clean_if_needed():
    """Vérifie l'espace libre et vide le dossier downloads si le seuil est atteint."""
    free_mb = get_free_space_mb()
    console_logger.debug(f"[DISK_MANAGER] Espace libre : {free_mb:.1f} Mo (seuil : {MIN_FREE_SPACE_MB} Mo)")

    if free_mb < MIN_FREE_SPACE_MB:
        console_logger.warning(
            f"[DISK_MANAGER] Espace libre insuffisant ({free_mb:.1f} Mo < {MIN_FREE_SPACE_MB} Mo). "
            "Nettoyage d'urgence du dossier downloads..."
        )
        clear_downloads()
        console_logger.info("[DISK_MANAGER] Nettoyage d'urgence terminé.")
        return True
    return False
