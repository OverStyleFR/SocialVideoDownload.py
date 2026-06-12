import os
from dotenv import load_dotenv

def get_token():
    # Priorité : variable d'environnement directe (Docker, Pelican, CI…)
    token = os.getenv("BOT_TOKEN", "").strip()
    if token and token != "YOUR_TELEGRAM_BOT_TOKEN_HERE":
        return token

    env_file = ".env"

    # Génération automatique du fichier .env s'il n'existe pas
    if not os.path.exists(env_file):
        try:
            with open(env_file, "w") as f:
                f.write("# === Configuration du bot Telegram ===\n")
                f.write("BOT_TOKEN=YOUR_TELEGRAM_BOT_TOKEN_HERE\n\n")
                f.write("# === Configuration générale ===\n")
                f.write("VERSION=V.8-7\n")
                f.write("DEVELOPED_BY=Tom V. | OverStyleFR\n")
                f.write("FFMPEG_PATH=ffmpeg/ffmpeg-7.0.2-amd64-static/ffmpeg\n")
            print(f"Le fichier {env_file} a été créé. Veuillez y renseigner votre token Telegram (BOT_TOKEN).")
        except OSError:
            print("Impossible de créer le fichier .env (système de fichiers en lecture seule). Veuillez définir la variable d'environnement BOT_TOKEN.")
        exit(1)

    load_dotenv(env_file, override=True)

    token = os.getenv("BOT_TOKEN", "").strip()

    if not token or token == "YOUR_TELEGRAM_BOT_TOKEN_HERE":
        print("Le fichier .env est vide ou contient une valeur par défaut. Veuillez y renseigner votre token Telegram (BOT_TOKEN).")
        exit(1)

    return token
