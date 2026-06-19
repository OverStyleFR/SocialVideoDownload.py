import os
from dotenv import load_dotenv

def get_token():
    # Charge .env s'il existe (sans override : Docker -e garde la priorité)
    if os.path.exists(".env"):
        load_dotenv(".env")

    token = os.getenv("BOT_TOKEN", "").strip()

    if token and token != "YOUR_TELEGRAM_BOT_TOKEN_HERE":
        return token

    # Aucun token valide trouvé → créer le template ou signaler l'erreur
    if not os.path.exists(".env"):
        with open(".env", "w") as f:
            f.write("# === Configuration du bot Telegram ===\n")
            f.write("BOT_TOKEN=YOUR_TELEGRAM_BOT_TOKEN_HERE\n\n")
            f.write("# === Configuration générale ===\n")
            f.write("# VERSION est definie dans .env.example (source de verite)\n")
            f.write("DEVELOPED_BY=Tom V. | OverStyleFR\n")
            f.write("FFMPEG_PATH=ffmpeg/ffmpeg-7.0.2-amd64-static/ffmpeg\n")
        print(f"Le fichier .env a été créé. Veuillez y renseigner votre token Telegram (BOT_TOKEN).")
        exit(1)

    print("Le fichier .env est vide ou contient une valeur par défaut. Veuillez y renseigner votre token Telegram (BOT_TOKEN).")
    exit(1)
