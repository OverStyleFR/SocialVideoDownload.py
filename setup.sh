#!/bin/bash
set -e

echo "=== SocialVideoDownload.py — Installation autonome ==="
echo ""

# Création de l'environnement virtuel
python3 -m venv .venv
source .venv/bin/activate

# Installation des dépendances
pip install -r requirements.txt

# Création du fichier .env
if [ ! -f .env ]; then
    cp .env.example .env
    echo "Fichier .env créé à partir de .env.example."
else
    echo "Fichier .env déjà existant, aucun changement."
fi

echo ""
echo "Installation terminée."
echo ""
echo "Configurez votre token Telegram dans .env (BOT_TOKEN), puis lancez :"
echo "  source .venv/bin/activate && python main.py"
echo ""
echo "Assurez-vous que ffmpeg est accessible dans votre PATH ou défini via FFMPEG_PATH dans .env."
