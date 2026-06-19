from utils.logger import console_logger
from utils.cache import cache_stats
from utils.disk_manager import get_free_space_mb
from utils.retention import retention_stats
from config import (
    VERSION, DEVELOPED_BY,
    SMALL_FILE_SIZE_MB, RETENTION_SMALL_HOURS, RETENTION_LARGE_HOURS,
)
import os
import time
import psutil

AUTHORIZED_USER = "overstylefr"
AUTHORIZED_IDS = {5092023723}


def _fmt_fr(value, decimals=2):
    return f"{value:.{decimals}f}".replace(".", ",")


def stats(update, context):
    user = update.message.from_user
    username = user.username or ""
    user_id = user.id

    # Autorise soit par username (insensible à la casse), soit par ID Telegram
    if username.lower() != AUTHORIZED_USER.lower() and user_id not in AUTHORIZED_IDS:
        update.message.reply_text("Accès refusé. Cette commande est réservée au développeur.")
        console_logger.warning(f"[STATS] Tentative non autorisée par @{username} (id={user_id})")
        return

    # --- Cache Stats ---
    cs = cache_stats()

    # --- Disk Stats ---
    downloads_dir = "downloads"
    total_downloads_size = 0
    total_downloads_files = 0

    if os.path.exists(downloads_dir):
        for root, dirs, files in os.walk(downloads_dir):
            for f in files:
                fp = os.path.join(root, f)
                total_downloads_size += os.path.getsize(fp)
                total_downloads_files += 1

    free_space_gb = get_free_space_mb() / 1024

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
    memory_used_gb = memory.used / (1024 ** 3)
    memory_total_gb = memory.total / (1024 ** 3)
    memory_percent = memory.percent

    # --- Hash Stats ---
    hashes_file = os.path.join(downloads_dir, "hashes.txt")
    total_hashes = 0
    if os.path.exists(hashes_file):
        with open(hashes_file, "r") as f:
            total_hashes = len(f.read().strip().splitlines())

    # --- Retention Stats ---
    rs = retention_stats()
    if rs["next_file"]:
        next_expire = f"`Prochain:` {rs['next_file']} dans {rs['next_remaining_min']} min"
    else:
        next_expire = "`Prochain:` aucun fichier"

    freed_2h_mb = rs["freed_2h"] / (1024 * 1024)
    freed_24h_mb = rs["freed_24h"] / (1024 * 1024)
    retention_block = (
        f"\n⏳ *Rétention*\n"
        f"`Politique:` petits ≤{SMALL_FILE_SIZE_MB} Mo → {RETENTION_SMALL_HOURS}h, gros → {RETENTION_LARGE_HOURS}h\n"
        f"`Fichiers:` {rs['total_files']} ({_fmt_fr(rs['total_size'] / (1024 * 1024))} Mo)\n"
        f"{next_expire}\n"
        f"`Libérable 2h:` {_fmt_fr(freed_2h_mb)} Mo ({rs['count_2h']} fichiers)\n"
        f"`Libérable 24h:` {_fmt_fr(freed_24h_mb)} Mo ({rs['count_24h']} fichiers)\n"
    )

    # --- Build Message ---
    msg = (
        f"📊 *Statistiques du Bot*\n"
        f"`Version:` {VERSION}\n"
        f"`Développé par:` {DEVELOPED_BY}\n\n"

        f"🗂️ *Cache (session)*\n"
        f"`Entrées totales:` {cs['total_entries']}\n"
        f"`Entrées valides:` {cs['valid']}\n"
        f"`Entrées expirées:` {cs['expired']}\n"
        f"`Petits fichiers (≤5Mo):` {cs['small']}\n"
        f"`Gros fichiers (>5Mo):` {cs['large']}\n"
        f"`Taille totale cache:` {cs['total_size'] / (1024 * 1024):.2f} Mo\n"
        f"`Hits cache:` {cs['total_hits']} ({cs['bytes_saved'] / (1024 * 1024):.2f} Mo économisés)\n\n"

        f"💾 *Disque*\n"
        f"`Espace libre:` {_fmt_fr(free_space_gb)} Go\n"
        f"`Fichiers downloads:` {total_downloads_files} ({total_downloads_size / (1024 * 1024):.2f} Mo)\n"
        f"`URLs enregistrées:` {total_hashes}\n"
        f"{retention_block}"

        f"📝 *Logs*\n"
        f"`Fichiers logs:` {total_log_files}\n"
        f"`Taille totale logs:` {total_log_size / (1024 * 1024):.2f} Mo\n\n"

        f"🖥️ *Système*\n"
        f"`Uptime:` {uptime_str}\n"
        f"`CPU:` {cpu_percent:.1f}%\n"
        f"`RAM:` {_fmt_fr(memory_used_gb, 1)}/{_fmt_fr(memory_total_gb, 1)} Go ({memory_percent}%)\n"
    )

    update.message.reply_text(msg, parse_mode="Markdown")
    console_logger.info(f"[STATS] Commande /stats exécutée par @{username}")
