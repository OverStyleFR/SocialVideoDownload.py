# Auto-download video. Direct Link
def download_and_send_video(bot, chat_id, text, update):
    link_hash = hashlib.md5(text.encode()).hexdigest()  # Générer un identifiant unique basé sur le hachage du lien
    video_path = os.path.join(download_temp_folder, f"{link_hash}.mp4")

    if os.path.exists(video_path):
        # Si le fichier existe déjà, envoyer directement la vidéo
        video = open(video_path, "rb")
        bot.send_video(chat_id=chat_id, video=InputFile(video), caption="Here's your video (already downloaded)")

        # Log lorsque la vidéo est envoyée sans téléchargement
        console_logger.info(f"Video sent directly from cache to {update.message.from_user.username}")

        video.close()
    else:
        # Sinon, procéder au téléchargement
        max_retries = 3  # Nombre maximum de réessais
        current_retry = 0

        while current_retry < max_retries:
            try:
                # Log pour enregistrer que le téléchargement est en cours
                console_logger.info(f"Downloading in progress with the link : {text} || from {update.message.from_user.username} #{current_retry}")

                result = subprocess.run(["./yt-dlp", "--format", "best", "-o", video_path, "--abort-on-unavailable-fragment", "-N 10", text], capture_output=True, text=True, check=True)

                # Log pour indiquer que le téléchargement s'est bien terminé
                console_logger.info(f"Video download completed successfully: {text}")

                if os.path.exists(video_path):
                    # Vérifier la taille du fichier
                    file_size = os.path.getsize(video_path)
                    max_size_bytes = 50 * 1024 * 1024  # Limite de 50 Mo

                    if file_size <= max_size_bytes:
                        video = open(video_path, "rb")
                        bot.send_video(chat_id=chat_id, video=InputFile(video), caption="Here's your video")

                        # Log lorsque la vidéo est envoyée (uploadée)
                        console_logger.info(f"Video successfully sent to {update.message.from_user.username} ⇒ Auto-Download")
                        
                        video.close()
                        break  # Sortir de la boucle en cas de succès
                    else:
                        bot.send_message(chat_id=chat_id, text="Erreur: La vidéo est trop grande pour être envoyée.")
                        # Log pour indiquer que la vidéo est trop grande
                        console_logger.warning(f"Video size exceeds the limit: {text}")
                        break  # Sortir de la boucle en cas d'erreur de taille
                else:
                    bot.send_message(chat_id=chat_id, text="Erreur: La vidéo téléchargée n'a pas été trouvée.")
                    current_retry += 1
                    bot.send_message(chat_id=chat_id, text=f"Réessai {current_retry}/{max_retries}...")
            except subprocess.CalledProcessError as e:
                bot.send_message(chat_id=chat_id, text=f"Erreur lors du téléchargement de la vidéo: {e}")
                current_retry += 1
                bot.send_message(chat_id=chat_id, text=f"Réessai {current_retry}/{max_retries}...")
                
                # Log pour indiquer qu'il y a eu une erreur lors du téléchargement
                console_logger.error(f"Error during video download: {text} - {str(e)}")
            except urllib3.exceptions.HTTPError as http_error:
                bot.send_message(chat_id=chat_id, text=f"Erreur HTTP lors du téléchargement de la vidéo. Tentative {current_retry + 1}/{max_retries}.")
                current_retry += 1
                bot.send_message(chat_id=chat_id, text=f"Réessai {current_retry}/{max_retries}...")
                
                # Log pour indiquer qu'il y a eu une erreur HTTP lors du téléchargement
                console_logger.error(f"HTTP error during video download: {text} - {str(http_error)}")
            except Exception as e:
                bot.send_message(chat_id=chat_id, text=f"Erreur lors de l'exécution de la commande: {str(e)}")
                current_retry += 1
                bot.send_message(chat_id=chat_id, text=f"Réessai {current_retry}/{max_retries}...")
                
                # Log général pour indiquer qu'il y a eu une autre erreur lors du téléchargement
                console_logger.error(f"Error during video download: {text} - {str(e)}")
                break  # Sortir de la boucle en cas d'erreur

        # Log général pour indiquer que le téléchargement a échoué
        if current_retry == max_retries:
            console_logger.error(f"Video download failed after {max_retries} retries: {text}")