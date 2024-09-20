def read_token():
    token_file_path = "token.txt"
    try:
        with open(token_file_path, "r") as file:
            return file.read().strip()
    except FileNotFoundError:
        print(f"Le fichier {token_file_path} n'a pas été trouvé.")
        return None