import os
import shutil
from config import MIN_FREE_SPACE_MB
from utils.logger import console_logger
from utils.retention import is_file_expired

DOWNLOADS_DIR = "downloads"
HASH_FILE = os.path.join(DOWNLOADS_DIR, "hashes.txt")


def get_free_space_mb() -> float:
    stat = shutil.disk_usage(DOWNLOADS_DIR if os.path.exists(DOWNLOADS_DIR) else ".")
    return stat.free / (1024 * 1024)


def clear_downloads():
    """Vidage complet du dossier downloads (fichiers + hashes.txt). Démarrage frais."""
    if os.path.exists(DOWNLOADS_DIR):
        shutil.rmtree(DOWNLOADS_DIR)
        console_logger.info("[DISK_MANAGER] Dossier downloads entièrement supprimé.")
    os.makedirs(DOWNLOADS_DIR, exist_ok=True)


def cleanup_by_retention():
    """Supprime les fichiers dont la rétention est expirée et nettoie hashes.txt."""
    if not os.path.exists(DOWNLOADS_DIR):
        os.makedirs(DOWNLOADS_DIR, exist_ok=True)
        return

    removed = 0
    for entry in os.listdir(DOWNLOADS_DIR):
        file_path = os.path.join(DOWNLOADS_DIR, entry)
        if entry == "hashes.txt" or not os.path.isfile(file_path):
            continue
        if is_file_expired(file_path):
            try:
                os.remove(file_path)
                console_logger.info(f"[DISK_MANAGER] Fichier expiré supprimé : {file_path}")
                removed += 1
            except Exception as e:
                console_logger.error(f"[DISK_MANAGER] Erreur suppression {file_path}: {e}")

    if removed:
        console_logger.info(f"[DISK_MANAGER] Nettoyage par rétention terminé — {removed} fichier(s) supprimé(s).")
    else:
        console_logger.debug("[DISK_MANAGER] Aucun fichier expiré trouvé.")


def check_and_clean_if_needed():
    """Vérifie l'espace libre. Nettoie par rétention d'abord, sinon vidage complet."""
    free_mb = get_free_space_mb()
    console_logger.debug(f"[DISK_MANAGER] Espace libre : {free_mb:.1f} Mo (seuil : {MIN_FREE_SPACE_MB} Mo)")

    if free_mb < MIN_FREE_SPACE_MB:
        console_logger.warning(
            f"[DISK_MANAGER] Espace libre insuffisant ({free_mb:.1f} Mo < {MIN_FREE_SPACE_MB} Mo). "
            "Nettoyage par rétention..."
        )
        cleanup_by_retention()
        free_mb = get_free_space_mb()
        if free_mb < MIN_FREE_SPACE_MB:
            console_logger.warning(
                f"[DISK_MANAGER] Toujours insuffisant après rétention ({free_mb:.1f} Mo). "
                "Vidage complet du dossier downloads..."
            )
            clear_downloads()
        console_logger.info("[DISK_MANAGER] Nettoyage d'urgence terminé.")
        return True
    return False
