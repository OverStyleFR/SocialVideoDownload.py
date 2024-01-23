import subprocess
import os
from telegram import InputFile, ReplyKeyboardMarkup, KeyboardButton, ParseMode
from telegram.ext import Updater, CommandHandler, MessageHandler, Filters

BOT_VERSION = "V0.3"
YOUR_NAME = "Tom V. | OverStyleFR"

# Fonction pour gérer la commande /start
def start(update, context):
    user_name = update.message.from_user.first_name  # Récupère le prénom de l'utilisateur
    welcome_message = f"Bonjour {user_name} 👋\n\n Je suis un bot qui permet de télécharger des vidéos/musiques via des liens de réseaux sociaux (principalement YouTube & TikTok)"
    
    # Envoie la réponse en utilisant la fonction "reply_text" avec "reply_to_message_id"
    update.message.reply_text(welcome_message, reply_to_message_id=update.message.message_id)
    


# Modifie la fonction help
def help(update, context):
    # Créer un paratexte en haut à droite
    paratext = f"Version {BOT_VERSION}\nDéveloppé par {YOUR_NAME}"

    # Créer une matrice de boutons pour les commandes disponibles
    buttons = [
        [KeyboardButton(text="/start"), KeyboardButton(text="/help")],
    ]

    # Ajouter des boutons supplémentaires au besoin

    # Créer un ReplyKeyboardMarkup avec les boutons
    markup = ReplyKeyboardMarkup(buttons, one_time_keyboard=True, resize_keyboard=True)

    # Envoyer le message avec le paratexte en haut à droite, les suggestions de commandes et le clavier pour les suggestions
    context.bot.send_message(
        chat_id=update.message.chat_id,
        text=f"Je suis un bot qui télécharge des vidéos/musiques via des liens de réseaux sociaux. Voici les commandes qui me sont associées:\n/start - Pour commencer\n/help - Pour obtenir de l'aide\n/download [LIEN] - Pour télécharger une vidéo avec yt-dlp\n/music [LIEN] - Pour télécharger de la musique avec yt-dlp\n\n"
             f"Si tu m'envoies un lien directement, je tenterai automatiquement de télécharger la vidéo associée.\n\n<code>{paratext}</code>",
        parse_mode=ParseMode.HTML,
        reply_markup=markup,
    )

# Fonction pour gérer les messages textuels
def handle_text_messages(update, context):
    text = update.message.text

    # Vérifier si le texte est un lien commencant par "https"
    if text.startswith("https"):
        video_path = "downloaded_video.mp4"

        # Supprimer le fichier existant s'il y en a un
        if os.path.exists(video_path):
            os.remove(video_path)

        # Exécuter la commande ./yt-dlp avec le lien et l'option -w pour écraser le fichier existant
        try:
            result = subprocess.run(["./yt-dlp", "--format", "best", "-o", "downloaded_video.mp4", text], capture_output=True, text=True)
            output = result.stdout.strip() if result.stdout else result.stderr.strip()

            # Envoyer la vidéo téléchargée
            if os.path.exists(video_path):
                video = open(video_path, "rb")
                context.bot.send_video(chat_id=update.message.chat_id, video=InputFile(video), caption="Voici votre vidéo!", reply_to_message_id=update.message.message_id)
                video.close()
            else:
                context.bot.send_message(chat_id=update.message.chat_id, text="Erreur: La vidéo téléchargée n'a pas été trouvée.")
        except Exception as e:
            context.bot.send_message(chat_id=update.message.chat_id, text=f"Erreur lors de l'exécution de la commande: {str(e)}", reply_to_message_id=update.message.message_id)
    else:
        context.bot.send_message(chat_id=update.message.chat_id, text=f"Je ne peux télécharger que des liens commencant par 'https'.")


# Fonction pour gérer la commande /download
def download(update, context):
    # Récupérer le lien depuis la commande ou directement depuis le texte du message
    link = " ".join(context.args) if context.args else update.message.text

    if not link:
        context.bot.send_message(chat_id=update.message.chat_id, text="Utilisation: /download [LIEN]")
        return

    # Vérifier si le texte du message est un lien
    if not link.startswith("http"):
        context.bot.send_message(chat_id=update.message.chat_id, text="Erreur: Le texte n'est pas un lien.")
        return

    # Exécuter la commande ./yt-dlp avec le lien
    try:
        result = subprocess.run(["./yt-dlp", "--format", "best", "-o", "downloaded_video.mp4", link], capture_output=True, text=True)
        output = result.stdout.strip() if result.stdout else result.stderr.strip()
        context.bot.send_message(chat_id=update.message.chat_id, text=output)

        # Envoyer la vidéo téléchargée
        video_path = "downloaded_video.mp4"
        if os.path.exists(video_path):
            video = open(video_path, "rb")
            context.bot.send_video(chat_id=update.message.chat_id, video=InputFile(video), caption="Voici votre vidéo!")
            video.close()
            os.remove(video_path)  # Supprimer le fichier après l'envoi
        else:
            context.bot.send_message(chat_id=update.message.chat_id, text="Erreur: La vidéo téléchargée n'a pas été trouvée.")
    except Exception as e:
        context.bot.send_message(chat_id=update.message.chat_id, text=f"Erreur lors de l'exécution de la commande: {str(e)}")


# Fonction pour gérer la commande /music
def music(update, context):
    # Récupérer le lien depuis la commande
    link = " ".join(context.args)

    if not link:
        context.bot.send_message(chat_id=update.message.chat_id, text="Utilisation: /music [LIEN]")
        return

    # Vérifier si le texte du message est un lien
    if not link.startswith("http"):
        context.bot.send_message(chat_id=update.message.chat_id, text="Erreur: Le texte n'est pas un lien.")
        return

    # Exécuter la commande ./yt-dlp avec le lien pour télécharger la musique
    try:
        # Spécifier le chemin vers ffmpeg et ffprobe
        ffmpeg_location = "ffmpeg-6.1-amd64-static/ffmpeg"  # Remplace avec le chemin correct
        result = subprocess.run(["./yt-dlp", "--extract-audio", "--audio-format", "mp3", "--ffmpeg-location", ffmpeg_location, "-o", "downloaded_music.%(ext)s", link], capture_output=True, text=True)
        output = result.stdout.strip() if result.stdout else result.stderr.strip()
        context.bot.send_message(chat_id=update.message.chat_id, text=output)

        # Envoyer la musique téléchargée
        music_path = "downloaded_music.mp3"
        if os.path.exists(music_path):
            music = open(music_path, "rb")
            context.bot.send_audio(chat_id=update.message.chat_id, audio=InputFile(music), caption="Voici votre musique!")
            music.close()
            os.remove(music_path)  # Supprimer le fichier après l'envoi
        else:
            context.bot.send_message(chat_id=update.message.chat_id, text="Erreur: La musique téléchargée n'a pas été trouvée.")
    except Exception as e:
        context.bot.send_message(chat_id=update.message.chat_id, text=f"Erreur lors de l'exécution de la commande: {str(e)}")





def main():
    # Token de votre bot Telegram
    token = "6977266339:AAHNxnhQn6pU_d0g7KioCOG7QclsUF0PBWk"

    # Initialisation de l'updater avec le token du bot
    updater = Updater(token=token, use_context=True)

    # Obtention du gestionnaire des commandes
    dp = updater.dispatcher

    # Ajout des gestionnaires de commandes
    dp.add_handler(CommandHandler("start", start))
    dp.add_handler(CommandHandler("help", help))
    dp.add_handler(CommandHandler("download", download, pass_args=True))
    dp.add_handler(CommandHandler("music", music, pass_args=True))

    # Ajout du gestionnaire pour les messages textuels
    dp.add_handler(MessageHandler(Filters.text & ~Filters.command, handle_text_messages))

    # Démarrage du bot
    updater.start_polling()
    
    # Indication dans la console
    print("Le bot a démarré avec succès!")

    # Arrêt du bot lorsqu'on appuie sur Ctrl+C
    updater.idle()

if __name__ == "__main__":
    main()
