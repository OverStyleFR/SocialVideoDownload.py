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
# The .env uses MB and HOURS, but the code needs BYTES and MINUTES for internal calculations
SMALL_FILE_SIZE_MB = int(os.getenv("SMALL_FILE_SIZE_MB", 4))  # Default 4 MB
RETENTION_SMALL_HOURS = int(os.getenv("RETENTION_SMALL_HOURS", 24))  # Default 24 hours
RETENTION_LARGE_HOURS = int(os.getenv("RETENTION_LARGE_HOURS", 2))  # Default 2 hours

# Internal conversion for use by retention.py
SMALL_FILE_SIZE_BYTES = SMALL_FILE_SIZE_MB * 1024 * 1024
RETENTION_SMALL_MINUTES = RETENTION_SMALL_HOURS * 60
RETENTION_LARGE_MINUTES = RETENTION_LARGE_HOURS * 60
