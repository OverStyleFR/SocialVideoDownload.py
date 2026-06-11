import os
from datetime import datetime, timedelta
from utils.logger import console_logger

# Size threshold to consider a file 'small' (bytes). Example 5 MB.
SMALL_FILE_SIZE_BYTES = 5 * 1024 * 1024

# Retention durations
RETENTION_SMALL_MINUTES = 60  # 1h for small files
RETENTION_LARGE_MINUTES = 10  # 10m for large files (example)

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
        console_logger.debug(f"[RETENTION] Set future mtime for {file_path} ({minutes} min)")
    except Exception as e:
        console_logger.error(f"[RETENTION] Failed to set mtime for {file_path}: {e}")
