# utils/curl_uploader.py
import os
import pycurl
from io import BytesIO
from urllib.parse import quote
import hashlib
import shutil
from utils.logger import console_logger

def compute_file_hash(file_path):
    """Calcule le hash SHA256 du fichier."""
    sha256 = hashlib.sha256()
    with open(file_path, "rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            sha256.update(chunk)
    return sha256.hexdigest()

def upload_large_file_via_curl(file_path, progress_callback=None):
    """
    Upload un fichier via PycURL vers curl.libriciel.fr.
    Le fichier est renommé selon son hash SHA256 et son extension d'origine.
    Le nom est URL-encodé pour composer l'URL cible.
    Si progress_callback est fourni, il est appelé avec le pourcentage d'avancement tous les 10%.
    Renvoie l'URL de partage retournée par le serveur.
    """
    if not os.path.exists(file_path):
        console_logger.error(f"[CURL UPLOAD] Fichier introuvable : {file_path}")
        raise FileNotFoundError(f"Le fichier '{file_path}' n'existe pas.")
    
    # Renommage selon le hash
    file_hash = compute_file_hash(file_path)
    _, ext = os.path.splitext(file_path)
    new_file_name = file_hash + ext
    temp_file_path = os.path.join(os.path.dirname(file_path), new_file_name)
    if not os.path.exists(temp_file_path):
        shutil.copyfile(file_path, temp_file_path)
    
    encoded_file_name = quote(new_file_name)
    target_url = f"https://curl.libriciel.fr/{encoded_file_name}"
    console_logger.info(f"[CURL UPLOAD] Upload du fichier '{temp_file_path}' vers '{target_url}' via curl.")
    
    c = pycurl.Curl()
    c.setopt(c.URL, target_url)
    c.setopt(c.UPLOAD, 1)
    file_size = os.path.getsize(temp_file_path)
    c.setopt(c.INFILESIZE, file_size)
    f = open(temp_file_path, 'rb')
    c.setopt(c.READDATA, f)
    
    last_reported = [0]
    def progress(download_total, download_now, upload_total, upload_now):
        if upload_total > 0:
            percent = int((upload_now / upload_total) * 100)
            if (percent - last_reported[0] >= 10) or (percent == 100 and last_reported[0] < 100):
                last_reported[0] = percent
                msg = f"Upload progress: {percent}% ⏳"
                console_logger.info(f"[CURL UPLOAD PROGRESS] {msg} pour '{new_file_name}'")
                if progress_callback:
                    progress_callback(percent)
        return 0

    c.setopt(c.NOPROGRESS, False)
    c.setopt(c.XFERINFOFUNCTION, progress)
    c.setopt(c.VERBOSE, False)
    
    response_buffer = BytesIO()
    c.setopt(c.WRITEFUNCTION, response_buffer.write)
    
    try:
        c.perform()
        response = response_buffer.getvalue().decode('utf-8').strip()
        console_logger.info(f"[CURL UPLOAD] Upload terminé pour '{new_file_name}'. Réponse: {response}")
        return response
    except pycurl.error as e:
        console_logger.error(f"[CURL UPLOAD] Échec de l'upload pour '{file_path}' : {e}")
        raise Exception(f"Upload échoué pour '{file_path}': {e}")
    finally:
        c.close()
        f.close()
        # Optionnel : supprimer le fichier temporaire après upload
        # os.remove(temp_file_path)
