# config.py
import os
from dotenv import load_dotenv

load_dotenv(".env")

VERSION = os.getenv("VERSION", "V.8-7")
DEVELOPED_BY = os.getenv("DEVELOPED_BY", "Tom V. | OverStyleFR")
FFMPEG_PATH = os.getenv("FFMPEG_PATH", "ffmpeg/ffmpeg-7.0.2-amd64-static/ffmpeg")  # Change cette valeur si nécessaire (chemin complet vers l'exécutable ffmpeg)

# Disk Management Configuration
MIN_FREE_SPACE_MB = int(os.getenv("MIN_FREE_SPACE_MB", 500))
CLEANUP_INTERVAL_HOURS = int(os.getenv("CLEANUP_INTERVAL_HOURS", 24))

# Retention Policy Configuration
SMALL_FILE_SIZE_BYTES = int(os.getenv("SMALL_FILE_SIZE_BYTES", 5 * 1024 * 1024)) # Default 5 MB
RETENTION_SMALL_MINUTES = int(os.getenv("RETENTION_SMALL_MINUTES", 60)) # Default 1 hour
RETENTION_LARGE_MINUTES = int(os.getenv("RETENTION_LARGE_MINUTES", 10)) # Default 10 minutes
