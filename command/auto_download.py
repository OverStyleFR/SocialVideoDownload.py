# Fonction pour gérer la commande /download
def download(update, context):
    link = context.args[0] if context.args else None

    if not link:
        # Si aucun lien n'est fourni, attendre le lien dans une réponse
        context.bot.send_message(chat_id=update.message.chat_id, text="Veuillez fournir un lien. Attendant votre réponse...")
        context.user_data["waiting_for_link"] = True
        return

    # Log de l'action
    console_logger.info(f"Download request with /download and the link : {link} || from {update.message.from_user.username}")

    if not link.startswith("http"):
        context.bot.send_message(chat_id=update.message.chat_id, text="Erreur: Le texte n'est pas un lien.")
        return

    max_retries = 3  # Nombre maximum de réessais
    current_retry = 0
    video_path = None  # Initialiser le chemin du fichier vidéo

    reply_message = context.bot.send_message(chat_id=update.message.chat_id, text="Downloading. processing...", reply_to_message_id=update.message.message_id)

    try:
        link_hash = hashlib.md5(link.encode()).hexdigest()  # Générer un identifiant unique basé sur le hachage du lien
        video_path = os.path.join(download_temp_folder, f"{link_hash}.mp4")

        if os.path.exists(video_path):
            # Si le fichier existe déjà, envoyer directement la vidéo
            video = open(video_path, "rb")
            context.bot.send_video(chat_id=update.message.chat_id, video=InputFile(video), caption="Here's your video (already downloaded)", reply_to_message_id=reply_message.message_id)

            # Log lorsque la vidéo est envoyée sans téléchargement
            console_logger.info(f"Video sent directly from cache to {update.message.from_user.username}")

            video.close()
        else:
            # Sinon, procéder au téléchargement
            while current_retry < max_retries:
                try:
                    # Log pour enregistrer que le téléchargement est en cours
                    console_logger.info(f"Downloading with /download and the link :  {link} || from {update.message.from_user.username} #{current_retry}")

                    result = subprocess.run(["./yt-dlp", "--format", "best", "-o", video_path, "--abort-on-unavailable-fragment", "-N 10", link], capture_output=True, text=True, check=True)

                    if result.returncode == 0:
                        # La commande s'est exécutée avec succès
                        output = result.stdout.strip()

                        # ... (le reste du code reste inchangé)

                        # Limiter la taille du message envoyé
                        output_message = output[:4000]  # Choisissez une longueur appropriée
                        context.bot.send_message(chat_id=update.message.chat_id, text=output_message, reply_to_message_id=reply_message.message_id)

                        # Enregistrer le résultat dans un fichier
                        save_result_to_file(output, link)

                        if os.path.exists(video_path):
                            # Vérifier la taille du fichier
                            file_size = os.path.getsize(video_path)
                            max_size_bytes = 50 * 1024 * 1024  # Limite de 50 Mo

                            if file_size <= max_size_bytes:
                                video = open(video_path, "rb")
                                context.bot.send_video(chat_id=update.message.chat_id, video=InputFile(video), caption="Here's your video", reply_to_message_id=reply_message.message_id)

                                # Log lorsque la vidéo est envoyée (uploadée)
                                console_logger.info(f"Video successfully sent to {update.message.from_user.username} ⇒ /download")

                                video.close()
                                break  # Sortir de la boucle en cas de succès
                            else:
                                context.bot.send_message(chat_id=update.message.chat_id, text="Erreur: La vidéo est trop grande pour être envoyée.", reply_to_message_id=reply_message.message_id)
                                # Log pour indiquer que la vidéo est trop grande
                                console_logger.warning(f"Video size exceeds the limit: {link}")
                                break  # Sortir de la boucle en cas d'erreur de taille
                        else:
                            context.bot.send_message(chat_id=update.message.chat_id, text="Erreur: La vidéo téléchargée n'a pas été trouvée.", reply_to_message_id=reply_message.message_id)
                            # Log en cas d'échec de l'upload de la vidéo
                            console_logger.error(f"Échec de l'upload de la vidéo à {update.message.from_user.username}")
                            current_retry += 1
                            context.bot.send_message(chat_id=update.message.chat_id, text=f"Réessai {current_retry}/{max_retries}...", reply_to_message_id=reply_message.message_id)
                    else:
                        # La commande a retourné une erreur
                        error_message = result.stderr.strip()
                        context.bot.send_message(chat_id=update.message.chat_id, text=f"Erreur lors du téléchargement de la vidéo: {error_message}", reply_to_message_id=reply_message.message_id)

                        # Log pour indiquer qu'il y a eu une erreur lors du téléchargement
                        console_logger.error(f"Error during video download: {link} - {error_message}")
                        current_retry += 1
                        context.bot.send_message(chat_id=update.message.chat_id, text=f"Réessai {current_retry}/{max_retries}...", reply_to_message_id=reply_message.message_id)
                except urllib3.exceptions.HTTPError as http_error:
                    context.bot.send_message(chat_id=update.message.chat_id, text=f"Erreur HTTP lors du téléchargement de la vidéo. Tentative {current_retry + 1}/{max_retries}.", reply_to_message_id=reply_message.message_id)
                    current_retry += 1
                    context.bot.send_message(chat_id=update.message.chat_id, text=f"Réessai {current_retry}/{max_retries}...", reply_to_message_id=reply_message.message_id)

                    # Log pour indiquer qu'il y a eu une erreur HTTP lors du téléchargement
                    console_logger.error(f"HTTP error during video download: {link} - {str(http_error)}")
                except Exception as e:
                    context.bot.send_message(chat_id=update.message.chat_id, text=f"Erreur lors de l'exécution de la commande: {str(e)}", reply_to_message_id=reply_message.message_id)
                    current_retry += 1
                    context.bot.send_message(chat_id=update.message.chat_id, text=f"Réessai {current_retry}/{max_retries}...", reply_to_message_id=reply_message.message_id)

                    # Log général pour indiquer qu'il y a eu une autre erreur lors du téléchargement
                    console_logger.error(f"Error during video download: {link} - {str(e)}")
                    break  # Sortir de la boucle en cas d'erreur
    except Exception as e:
        # Log en cas d'erreur pendant l'exécution de la commande
        console_logger.error(f"Erreur pendant l'exécution de la commande /download : {str(e)}")
    finally:
        # Supprimer le fichier temporaire uniquement en cas d'échec
        if video_path and not os.path.exists(video_path):
            os.remove(video_path)