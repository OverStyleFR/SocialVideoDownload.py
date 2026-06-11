import os
import math
from utils.logger import console_logger

class ProgressFile:
    def __init__(self, filename, progress_interval=20):
        self.filename = filename
        self.f = open(filename, "rb")
        self.total = os.path.getsize(filename)
        self.read_bytes = 0
        self.last_percent = 0
        self.progress_interval = progress_interval  # Intervalle de 20% par défaut

    def read(self, size=-1):
        data = self.f.read(size)
        self.read_bytes += len(data)
        if self.total > 0:
            percent = math.floor((self.read_bytes / self.total) * 100)
            if percent - self.last_percent >= self.progress_interval:
                self.last_percent = percent
                console_logger.info(f"[UPLOAD] Envoi du fichier {self.filename} : {percent}% complété.")
        return data

    def close(self):
        self.f.close()

    def __getattr__(self, attr):
        # Permet d'accéder aux autres attributs du fichier
        return getattr(self.f, attr)
