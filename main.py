import subprocess
import os
import logging
import urllib3
import hashlib

from datetime import datetime
from telegram import InputFile, ReplyKeyboardMarkup, KeyboardButton, ParseMode
from telegram.ext import Updater, CommandHandler, MessageHandler, Filters

# Dossier de téléchargement temporaire
download_temp_folder = "download_temp"

# Créer le dossier "logs" s'il n'existe pas
if not os.path.exists("logs"):
    os.makedirs("logs")

# Configuration du logging
log_file_name = f"logs/bot_{datetime.now().strftime('%Y%m%d_%H%M%S')}.log"
file_handler = logging.FileHandler(log_file_name)
file_handler.setLevel(logging.INFO)

# Créer un logger pour la console
console_logger = logging.getLogger("console_logger")
console_logger.setLevel(logging.INFO)

# Ajouter un formatteur pour inclure la date, le niveau de danger, et le message dans les logs
log_formatter = logging.Formatter('%(asctime)s - %(levelname)s - %(message)s')

# Ajouter le formatteur au gestionnaire de fichier
file_handler.setFormatter(log_formatter)

# Ajouter un gestionnaire de fichier au logger principal
logging.basicConfig(level=logging.INFO, handlers=[file_handler])

# Créer un gestionnaire de console et l'ajouter uniquement au logger de la console
console_handler = logging.StreamHandler()
console_handler.setLevel(logging.INFO)
console_handler.setFormatter(log_formatter)
console_logger.addHandler(console_handler)

BOT_VERSION = "V0.7-1"
YOUR_NAME = "Tom V. | OverStyleFR"

# Fonction pour gérer la commande /start
def start(update, context):
    user_name = update.message.from_user.first_name
    welcome_message = f"Bonjour {user_name} 👋\n\n Je suis un bot qui permet de télécharger des vidéos/musiques via des liens de réseaux sociaux (principalement YouTube & TikTok)"

    buttons = [
        [KeyboardButton(text="/start"), KeyboardButton(text="/help")],
    ]

    markup = ReplyKeyboardMarkup(buttons, one_time_keyboard=True, resize_keyboard=True)

    update.message.reply_text(welcome_message, parse_mode=ParseMode.HTML, reply_markup=markup, reply_to_message_id=update.message.message_id)

    # Log de l'action
    console_logger.info(f"Command /start execute from {update.message.from_user.username}")

# Modifie la fonction help
def help(update, context):
    paratext = f"Version {BOT_VERSION}\nDéveloppé par {YOUR_NAME}"

    buttons = [
        [KeyboardButton(text="/start"), KeyboardButton(text="/help")],
    ]

    markup = ReplyKeyboardMarkup(buttons, one_time_keyboard=True, resize_keyboard=True)

    context.bot.send_message(
        chat_id=update.message.chat_id,
        text=f"Je suis un bot qui télécharge des vidéos/musiques via des liens de réseaux sociaux. Voici les commandes qui me sont associées:\n/start - Pour commencer\n/help - Pour obtenir de l'aide\n/download [LIEN] - Pour télécharger une vidéo avec yt-dlp\n/music [LIEN] - Pour télécharger de la musique avec yt-dlp\n\n"
             f"Si tu m'envoies un lien directement, je tenterai automatiquement de télécharger la vidéo associée.\n\n<code>{paratext}</code>",
        parse_mode=ParseMode.HTML,
        reply_markup=markup,
    )

    # Log de l'action
    console_logger.info(f"Command /help execute from {update.message.from_user.username}")

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

                result = subprocess.run(["./yt-dlp", "--format", "best", "-o", video_path, text], capture_output=True, text=True, check=True)

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

# Fonction pour gérer les messages textuels
def handle_text_messages(update, context):
    text = update.message.text

    # Log de l'action
    console_logger.info(f"Message recieve : {text}")

    if text.startswith("https"):
        reply_message = context.bot.send_message(chat_id=update.message.chat_id, text="Téléchargement en cours. Veuillez patienter...", reply_to_message_id=update.message.message_id)
        
        download_and_send_video(context.bot, update.message.chat_id, text, update)


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

                    result = subprocess.run(["./yt-dlp", "--format", "best", "-o", video_path, link], capture_output=True, text=True, check=True)
                    output = result.stdout.strip() if result.stdout else result.stderr.strip()

                    # Log pour indiquer que le téléchargement s'est bien terminé
                    console_logger.info(f"Video download completed successfully: {link}")

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
                except subprocess.CalledProcessError as e:
                    context.bot.send_message(chat_id=update.message.chat_id, text=f"Erreur lors du téléchargement de la vidéo: {e}", reply_to_message_id=reply_message.message_id)
                    current_retry += 1
                    context.bot.send_message(chat_id=update.message.chat_id, text=f"Réessai {current_retry}/{max_retries}...", reply_to_message_id=reply_message.message_id)
                    
                    # Log pour indiquer qu'il y a eu une erreur lors du téléchargement
                    console_logger.error(f"Error during video download: {link} - {str(e)}")
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
                    result = subprocess.run(["./yt-dlp", "--extract-audio", "--audio-format", "mp3", "--ffmpeg-location", ffmpeg_location, "-o", music_path, link], capture_output=True, text=True)
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


def read_token():
    token_file_path = "token.txt"
    try:
        with open(token_file_path, "r") as file:
            return file.read().strip()
    except FileNotFoundError:
        print(f"Le fichier {token_file_path} n'a pas été trouvé.")
        return None

def main():
    token = read_token()

    if token is None:
        print("Le token n'a pas été trouvé. Assurez-vous que le fichier token.txt contient le token.")
        return


    # Vérifier si le dossier 'download_temp' existe
    folder_path = 'download_temp'
    if os.path.exists(folder_path):
        # Supprimer le contenu du dossier s'il existe
        for file_name in os.listdir(folder_path):
            file_path = os.path.join(folder_path, file_name)
            try:
                if os.path.isfile(file_path):
                    os.unlink(file_path)
            except Exception as e:
                print(e)
        console_logger.info('Folder exist. All files as been deleted.')
    else:
        # Créer le dossier s'il n'existe pas
        os.makedirs(folder_path)
        console_logger.info("Folder Create")


    updater = Updater(token=token, use_context=True)
    dp = updater.dispatcher

    dp.add_handler(CommandHandler("start", start))
    dp.add_handler(CommandHandler("help", help))
    dp.add_handler(CommandHandler("download", download, pass_args=True))
    dp.add_handler(CommandHandler("music", music, pass_args=True))

    dp.add_handler(MessageHandler(Filters.text & ~Filters.command, handle_text_messages))

    updater.start_polling()

    console_logger.info("Le bot a démarré avec succès!")
    updater.idle()

if __name__ == "__main__":
    main()