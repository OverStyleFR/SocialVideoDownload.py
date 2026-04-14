import os
from dotenv import load_dotenv

load_dotenv(".env")

VERSION = os.getenv("VERSION", "V.8-7")
DEVELOPED_BY = os.getenv("DEVELOPED_BY", "Tom V. | OverStyleFR")
FFMPEG_PATH = os.getenv("FFMPEG_PATH", "ffmpeg/ffmpeg-7.0.2-amd64-static/ffmpeg")  # Change cette valeur si nécessaire (chemin complet vers l'exécutable ffmpeg)
