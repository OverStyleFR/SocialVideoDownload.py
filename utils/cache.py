import time
import hashlib
from utils.logger import console_logger

SMALL_FILE_THRESHOLD = 5 * 1024 * 1024
LONG_TTL = 24 * 3600
STANDARD_TTL = 1 * 3600

download_cache = {}


def load_cache():
    global download_cache
    download_cache.clear()
    console_logger.info("Cache initialisé (session en mémoire).")


def get_ttl(file_size):
    return LONG_TTL if file_size <= SMALL_FILE_THRESHOLD else STANDARD_TTL


def is_cache_valid(link_hash):
    if link_hash not in download_cache:
        return False
    timestamp, size, _hits = download_cache[link_hash]
    return (time.time() - timestamp) < get_ttl(size)


def add_to_cache(link, file_size):
    link_hash = hashlib.sha256(link.encode()).hexdigest()
    download_cache[link_hash] = [time.time(), file_size, 0]
    return link_hash


def record_cache_hit(link):
    link_hash = hashlib.sha256(link.encode()).hexdigest()
    if link_hash in download_cache:
        download_cache[link_hash][2] += 1
        return True
    return False


def cache_stats():
    total_entries = len(download_cache)
    hits = 0
    expired = 0
    total_size = 0
    small = 0
    large = 0
    total_hits = 0
    bytes_saved = 0

    for timestamp, file_size, hit_count in download_cache.values():
        age = time.time() - timestamp
        total_hits += hit_count
        bytes_saved += file_size * hit_count
        if age < get_ttl(file_size):
            hits += 1
            total_size += file_size
            if file_size <= SMALL_FILE_THRESHOLD:
                small += 1
            else:
                large += 1
        else:
            expired += 1

    return {
        "total_entries": total_entries,
        "valid": hits,
        "expired": expired,
        "small": small,
        "large": large,
        "total_size": total_size,
        "total_hits": total_hits,
        "bytes_saved": bytes_saved,
    }