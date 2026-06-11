from utils.logger import console_logger
from utils.cache import download_cache
from utils.disk_manager import get_free_space_mb
from config import VERSION, DEVELOPED_BY
import os
import time
import hashlib
import psutil

AUTHORIZED_USER = "overstylefr"


def stats(update, context):
    username = update.message.from_user.username
    if username != AUTHORIZED_USER:
        update.message.reply_text("Accès refusé. Cette commande est réservée au développeur.")
        console_logger.warning(f"[STATS] Tentative non autorisée par @{username}")
        return

    # --- Cache Stats ---
    cache_entries = len(download_cache)
    cache_hits = 0
    cache_misses = 0
    cache_expired = 0
    cache_total_size = 0
    cache_small_files = 0
    cache_large_files = 0

    for link_hash, (timestamp, file_size) in download_cache.items():
        from utils.cache import get_ttl
        ttl = get_ttl(file_size)
        age = time.time() - timestamp
        if age < ttl:
            cache_hits += 1
            cache_total_size += file_size
            if file_size <= 5 * 1024 * 1024:
                cache_small_files += 1
            else:
                cache_large_files += 1
        else:
            cache_expired += 1

    # --- Disk Stats ---
    downloads_dir = "downloads"
    download_temp_dir = "download_temp"
    total_downloads_size = 0
    total_downloads_files = 0
    total_temp_size = 0
    total_temp_files = 0

    if os.path.exists(downloads_dir):
        for root, dirs, files in os.walk(downloads_dir):
            for f in files:
                fp = os.path.join(root, f)
                total_downloads_size += os.path.getsize(fp)
                total_downloads_files += 1

    if os.path.exists(download_temp_dir):
        for root, dirs, files in os.walk(download_temp_dir):
            for f in files:
                fp = os.path.join(root, f)
                total_temp_size += os.path.getsize(fp)
                total_temp_files += 1

    free_space_mb = get_free_space_mb()

    # --- Logs Stats ---
    logs_dir = "logs"
    total_log_size = 0
    total_log_files = 0
    if os.path.exists(logs_dir):
        for f in os.listdir(logs_dir):
            fp = os.path.join(logs_dir, f)
            if os.path.isfile(fp):
                total_log_size += os.path.getsize(fp)
                total_log_files += 1

    # --- System Stats ---
    uptime_seconds = time.time() - psutil.boot_time()
    uptime_str = f"{int(uptime_seconds // 3600)}h {int((uptime_seconds % 3600) // 60)}m"
    cpu_percent = psutil.cpu_percent(interval=0.1)
    memory = psutil.virtual_memory()
    memory_used_mb = memory.used / (1024 * 1024)
    memory_total_mb = memory.total / (1024 * 1024)
    memory_percent = memory.percent

    # --- Hash Stats ---
    hashes_file = os.path.join(downloads_dir, "hashes.txt")
    total_hashes = 0
    if os.path.exists(hashes_file):
        with open(hashes_file, "r") as f:
            total_hashes = len(f.read().strip().splitlines())

    # --- Build Message ---
    msg = (
        f"📊 *Statistiques du Bot*\n"
        f"`Version:` {VERSION}\n"
        f"`Développé par:` {DEVELOPED_BY}\n\n"

        f"🗂️ *Cache*\n"
        f"`Entrées totales:` {cache_entries}\n"
        f"`Entrées valides:` {cache_hits}\n"
        f"`Entrées expirées:` {cache_expired}\n"
        f"`Petits fichiers (≤5Mo):` {cache_small_files}\n"
        f"`Gros fichiers (>5Mo):` {cache_large_files}\n"
        f"`Taille totale cache:` {cache_total_size / (1024 * 1024):.2f} Mo\n\n"

        f"💾 *Disque*\n"
        f"`Espace libre:` {free_space_mb:.2f} Mo\n"
        f"`Fichiers downloads:` {total_downloads_files} ({total_downloads_size / (1024 * 1024):.2f} Mo)\n"
        f"`Fichiers temp:` {total_temp_files} ({total_temp_size / (1024 * 1024):.2f} Mo)\n"
        f"`URLs enregistrées:` {total_hashes}\n\n"

        f"📝 *Logs*\n"
        f"`Fichiers logs:` {total_log_files}\n"
        f"`Taille totale logs:` {total_log_size / (1024 * 1024):.2f} Mo\n\n"

        f"🖥️ *Système*\n"
        f"`Uptime:` {uptime_str}\n"
        f"`CPU:` {cpu_percent:.1f}%\n"
        f"`RAM:` {memory_used_mb:.1f}/{memory_total_mb:.1f} Mo ({memory_percent}%)\n"
    )

    update.message.reply_text(msg, parse_mode="Markdown")
    console_logger.info(f"[STATS] Commande /stats exécutée par @{username}")
