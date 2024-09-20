def save_result_to_file(result, link):
    # Créer le répertoire s'il n'existe pas
    result_folder = os.path.join(download_temp_folder, "download_result")
    os.makedirs(result_folder, exist_ok=True)

    # Créer le nom de fichier basé sur la date actuelle
    current_date = datetime.now().strftime("%Y%m%d_%H%M%S")
    file_name = f"result_{current_date}.txt"

    # Chemin complet du fichier
    file_path = os.path.join(result_folder, file_name)

    # Enregistrer le résultat dans le fichier
    with open(file_path, "w", encoding="utf-8") as file:
        file.write(result)

    # Log pour indiquer que le résultat a été enregistré
    console_logger.info(f"Download result saved to file: {file_path}")