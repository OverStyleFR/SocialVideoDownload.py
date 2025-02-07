import os

def get_token():
    token_file = "token.txt"
    if not os.path.exists(token_file):
        with open(token_file, "w") as f:
            f.write("YOUR_TELEGRAM_BOT_TOKEN_HERE")
        print(f"Le fichier {token_file} a été créé. Veuillez y insérer votre token Telegram.")
        exit(1)
    
    with open(token_file, "r") as f:
        token = f.read().strip()
    
    if not token or token == "YOUR_TELEGRAM_BOT_TOKEN_HERE":
        print(f"Le fichier {token_file} est vide ou contient une valeur par défaut. Veuillez y insérer votre token Telegram.")
        exit(1)
    
    return token
