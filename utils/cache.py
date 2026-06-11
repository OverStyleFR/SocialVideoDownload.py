import os
import json
import time
import hashlib
from utils.logger import console_logger

# Bot version related constants (assuming these are defined in main.py or a config file)
# For now, hardcoding as example:
BOT_VERSION = "V9.1" 

# Cache configuration
SMALL_FILE_THRESHOLD = 5 * 1024 * 1024  # 5 MB
LONG_TTL = 24 * 3600  # 24 hours for files ≤5MB
STANDARD_TTL = 1 * 3600  # 1 hour for files >5MB
AUTHORIZED_USER = "overstylefr"
CACHE_FILE = "download_temp/cache_metadata.json"

download_cache = {}

def load_cache():
    global download_cache
    try:
        with open(CACHE_FILE, 'r') as f:
            download_cache = json.load(f)
        console_logger.info(f'Cache loaded from {CACHE_FILE}')
    except FileNotFoundError:
        console_logger.warning(f'Cache file {CACHE_FILE} not found. Initializing empty cache.')
        download_cache = {}
    except json.JSONDecodeError:
        console_logger.error(f'Error decoding JSON from {CACHE_FILE}. Initializing empty cache.')
        download_cache = {}
    except Exception as e:
        console_logger.error(f'An unexpected error occurred loading cache: {e}')
        download_cache = {}

def save_cache():
    try:
        # Ensure the directory exists
        cache_dir = os.path.dirname(CACHE_FILE)
        if cache_dir and not os.path.exists(cache_dir):
            os.makedirs(cache_dir)
            console_logger.info(f'Created cache directory: {cache_dir}')
            
        with open(CACHE_FILE, 'w') as f:
            json.dump(download_cache, f, indent=4) # Use indent for readability
        console_logger.info(f'Cache saved to {CACHE_FILE}')
    except Exception as e:
        console_logger.error(f'An error occurred saving cache to {CACHE_FILE}: {e}')

def get_ttl(file_size):
    return LONG_TTL if file_size <= SMALL_FILE_THRESHOLD else STANDARD_TTL

def is_cache_valid(link_hash):
    if link_hash not in download_cache:
        return False
    timestamp, size = download_cache[link_hash]
    current_time = time.time()
    ttl = get_ttl(size)
    is_valid = (current_time - timestamp) < ttl
    # console_logger.debug(f"Cache check for hash {link_hash}: valid={is_valid}, age={(current_time - timestamp):.0f}s, ttl={ttl}s")
    return is_valid

def add_to_cache(link, file_size):
    """Adds or updates an entry in the cache."""
    link_hash = hashlib.md5(link.encode()).hexdigest()
    download_cache[link_hash] = (time.time(), file_size)
    save_cache()
    return link_hash

def get_cached_file_path(link_hash):
    """Tries to find the actual file path from the hash. Assumes a known file structure."""
    # This is a placeholder, a more robust solution might be needed if extensions vary wildly
    # or if files are stored with different naming conventions.
    base_path = os.path.join("download_temp", link_hash)
    for ext in ['.mp4', '.mkv', '.webm', '.avi', '.mov', '.mp3', '.m4a', '.ogg', '.wav']:
        if os.path.exists(base_path + ext):
            return base_path + ext
    return None

