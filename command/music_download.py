# Fonction pour gérer la commande /music
def music(update, context):
    link = " ".join(context.args)

    if not link:
        context.bot.send_message(chat_id=update.message.chat_id, text="Utilisation: /music [LIEN]")
        return

    # Log de l'action
    console_logger.info(f"Download request with /music and the link : {link} || from {update.message.from_user.username}")

    if not link.startswith("http"):
        context.bot.send_message(chat_id=update.message.chat_id, text="Erreur: Le texte n'est pas un lien.")
        return

    max_retries = 3  # Nombre maximum de réessais
    current_retry = 0
    music_path = None  # Initialiser le chemin du fichier musique

    reply_message = context.bot.send_message(chat_id=update.message.chat_id, text="Downloading. processing...", reply_to_message_id=update.message.message_id)
    # Log pour enregistrer que le téléchargement est en cours
    console_logger.info(f"Downloading with /music and the link :  {link} || from {update.message.from_user.username}")

    output = None
    try:
        link_hash = hashlib.md5(link.encode()).hexdigest()  # Générer un identifiant unique basé sur le hachage du lien
        music_path = os.path.join(download_temp_folder, f"{link_hash}.mp3")

        if os.path.exists(music_path):
            # Si le fichier existe déjà, envoyer directement la musique
            music = open(music_path, "rb")
            context.bot.send_audio(chat_id=update.message.chat_id, audio=InputFile(music), caption="Here's your music (already downloaded)", reply_to_message_id=reply_message.message_id)

            # Log lorsque la musique est envoyée sans téléchargement
            console_logger.info(f"Music sent directly from cache to {update.message.from_user.username}")
            
            music.close()
        else:
            # Sinon, procéder au téléchargement
            while current_retry < max_retries:
                try:
                    ffmpeg_location = "ffmpeg-6.1-amd64-static/ffmpeg"
                    result = subprocess.run(["./yt-dlp", "--extract-audio", "--audio-format", "mp3", "--ffmpeg-location", ffmpeg_location, "-o", music_path, "--abort-on-unavailable-fragment", "-N 10", link], capture_output=True, text=True)
                    output = result.stdout.strip() if result.stdout else result.stderr.strip()
                    context.bot.send_message(chat_id=update.message.chat_id, text=output, reply_to_message_id=reply_message.message_id)

                    # Log pour indiquer que le téléchargement s'est bien terminé
                    console_logger.info(f"Music download and converting completed successfully: {link}")

                    if os.path.exists(music_path):
                        music = open(music_path, "rb")
                        context.bot.send_audio(chat_id=update.message.chat_id, audio=InputFile(music), caption="Here's your music", reply_to_message_id=reply_message.message_id)

                        # Log lorsque la musique est envoyée (uploadée)
                        console_logger.info(f"Music send successfully to {update.message.from_user.username} ⇒ /music")

                        music.close()
                        break  # Sortir de la boucle en cas de succès
                    else:
                        context.bot.send_message(chat_id=update.message.chat_id, text="Erreur: La musique téléchargée n'a pas été trouvée.", reply_to_message_id=reply_message.message_id)
                        # Log en cas d'échec de l'upload de la musique
                        console_logger.error(f"Échec de l'upload de la musique à {update.message.from_user.username}")
                        current_retry += 1
                        context.bot.send_message(chat_id=update.message.chat_id, text=f"Réessai {current_retry}/{max_retries}...", reply_to_message_id=reply_message.message_id)
                except urllib3.exceptions.HTTPError as http_error:
                    context.bot.send_message(chat_id=update.message.chat_id, text=f"Erreur HTTP lors du téléchargement de la musique. Tentative {current_retry + 1}/{max_retries}.", reply_to_message_id=reply_message.message_id)
                    console_logger.error(f"Erreur HTTP lors du téléchargement de la musique depuis le lien {link}: {str(http_error)}")
                    current_retry += 1
                    context.bot.send_message(chat_id=update.message.chat_id, text=f"Réessai {current_retry}/{max_retries}...", reply_to_message_id=reply_message.message_id)
                except Exception as e:
                    context.bot.send_message(chat_id=update.message.chat_id, text=f"Erreur lors de l'exécution de la commande: {str(e)}", reply_to_message_id=reply_message.message_id)
                    # Log en cas d'erreur lors du téléchargement de la musique
                    console_logger.error(f"Erreur lors du téléchargement de la musique depuis le lien {link}: {str(e)}")
                    current_retry += 1
                    context.bot.send_message(chat_id=update.message.chat_id, text=f"Réessai {current_retry}/{max_retries}...", reply_to_message_id=reply_message.message_id)
    except Exception as e:
        # Log en cas d'erreur pendant l'exécution de la commande
        console_logger.error(f"Erreur pendant l'exécution de la commande /music : {str(e)}")
    finally:
        # Utiliser la fonction save_result_to_file
        save_result_to_file(output, link)

        # Supprimer le fichier temporaire uniquement en cas d'échec
        if music_path and not os.path.exists(music_path):
            os.remove(music_path)