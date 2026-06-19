import os
import time
from datetime import datetime, timedelta
from utils.logger import console_logger

from config import (
    SMALL_FILE_SIZE_BYTES, SMALL_FILE_SIZE_MB,
    RETENTION_SMALL_MINUTES, RETENTION_SMALL_HOURS,
    RETENTION_LARGE_MINUTES, RETENTION_LARGE_HOURS,
)


def get_retention_minutes(file_path: str) -> int:
    """Return retention in minutes based on file size and type.
    Music files (mp3) are considered small.
    """
    if not os.path.exists(file_path):
        return 0
    size = os.path.getsize(file_path)
    _, ext = os.path.splitext(file_path)
    if ext.lower() == ".mp3":
        return RETENTION_SMALL_MINUTES
    if size < SMALL_FILE_SIZE_BYTES:
        return RETENTION_SMALL_MINUTES
    return RETENTION_LARGE_MINUTES

def set_retention(file_path: str):
    """Set the file's modification time to the future according to retention.
    This allows later cleanup to respect retention when removing old files.
    """
    minutes = get_retention_minutes(file_path)
    if minutes <= 0:
        return
    future_time = datetime.now() + timedelta(minutes=minutes)
    ts = future_time.timestamp()
    try:
        os.utime(file_path, (ts, ts))
        console_logger.info(f"[RETENTION] Set future mtime for {file_path} ({minutes} min)")
    except Exception as e:
        console_logger.error(f"[RETENTION] Failed to set mtime for {file_path}: {e}")

def is_file_expired(file_path: str) -> bool:
    """Check if a file's retention period has expired.
    The file's mtime was set to (now + retention) by set_retention(),
    so if mtime < now, the retention has elapsed.
    """
    if not os.path.exists(file_path):
        return True
    mtime = os.path.getmtime(file_path)
    return mtime < time.time()


def retention_stats() -> dict:
    """Return retention statistics for /stats command.

    Returns:
        dict with keys: total_files, total_size, next_file, next_remaining_min,
                        freed_2h, freed_24h, count_2h, count_24h
    """
    DOWNLOADS_DIR = "downloads"
    now = time.time()

    files_info = []
    total_size = 0
    total_files = 0

    if os.path.exists(DOWNLOADS_DIR):
        for entry in os.listdir(DOWNLOADS_DIR):
            file_path = os.path.join(DOWNLOADS_DIR, entry)
            if entry == "hashes.txt" or not os.path.isfile(file_path):
                continue
            mtime = os.path.getmtime(file_path)
            size = os.path.getsize(file_path)
            remaining = mtime - now
            files_info.append((entry, size, remaining))
            total_size += size
            total_files += 1

    if not files_info:
        return {
            "total_files": 0,
            "total_size": 0,
            "next_file": None,
            "next_remaining_min": None,
            "freed_2h": 0,
            "freed_24h": 0,
            "count_2h": 0,
            "count_24h": 0,
        }

    files_info.sort(key=lambda x: x[2])
    next_file = files_info[0][0]
    next_remaining_min = max(0, int(files_info[0][2] / 60))

    freed_2h = sum(f[1] for f in files_info if f[2] <= 7200)
    freed_24h = sum(f[1] for f in files_info if f[2] <= 86400)
    count_2h = sum(1 for f in files_info if f[2] <= 7200)
    count_24h = sum(1 for f in files_info if f[2] <= 86400)

    return {
        "total_files": total_files,
        "total_size": total_size,
        "next_file": next_file,
        "next_remaining_min": next_remaining_min,
        "freed_2h": freed_2h,
        "freed_24h": freed_24h,
        "count_2h": count_2h,
        "count_24h": count_24h,
    }
