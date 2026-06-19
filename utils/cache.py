import os
import json
import time
import hashlib
from utils.logger import console_logger

SMALL_FILE_THRESHOLD = 5 * 1024 * 1024
LONG_TTL = 24 * 3600
STANDARD_TTL = 1 * 3600
CACHE_FILE = "download_temp/cache_metadata.json"

download_cache = {}

def load_cache():
    global download_cache
    try:
        with open(CACHE_FILE, 'r') as f:
            data = json.load(f)
        download_cache.clear()
        download_cache.update(data)
        console_logger.info(f'Cache loaded from {CACHE_FILE}')
    except FileNotFoundError:
        console_logger.warning(f'Cache file {CACHE_FILE} not found. Initializing empty cache.')
        download_cache.clear()
    except json.JSONDecodeError:
        console_logger.error(f'Error decoding JSON from {CACHE_FILE}. Initializing empty cache.')
        download_cache.clear()
    except Exception as e:
        console_logger.error(f'An unexpected error occurred loading cache: {e}')
        download_cache.clear()

def save_cache():
    try:
        cache_dir = os.path.dirname(CACHE_FILE)
        if cache_dir and not os.path.exists(cache_dir):
            os.makedirs(cache_dir)
            console_logger.info(f'Created cache directory: {cache_dir}')
        with open(CACHE_FILE, 'w') as f:
            json.dump(download_cache, f, indent=4)
        console_logger.info(f'Cache saved to {CACHE_FILE}')
    except Exception as e:
        console_logger.error(f'An error occurred saving cache to {CACHE_FILE}: {e}')

def get_ttl(file_size):
    return LONG_TTL if file_size <= SMALL_FILE_THRESHOLD else STANDARD_TTL

def is_cache_valid(link_hash):
    if link_hash not in download_cache:
        return False
    timestamp, size = download_cache[link_hash]
    ttl = get_ttl(size)
    return (time.time() - timestamp) < ttl

def add_to_cache(link, file_size):
    link_hash = hashlib.sha256(link.encode()).hexdigest()
    download_cache[link_hash] = (time.time(), file_size)
    save_cache()
    return link_hash

