#!/bin/sh
# Lancement sous Pelican/Pterodactyl : Wings monte le serveur sur /home/container et passe
# la commande de démarrage de l'egg dans STARTUP ({{VAR}} = variable de l'egg).
set -eu
cd /home/container
# tmp/ (TMPDIR : extraction de yt-dlp) repart vide à chaque démarrage, comme un /tmp.
rm -rf tmp
mkdir -p bin tmp
# Copie inscriptible pour que yt-dlp se mette à jour lui-même ; une image plus récente la remplace.
cp -u /usr/local/bin/yt-dlp bin/yt-dlp
# Pas de .env sous Pelican : VERSION (affichée par /help) vient du modèle du dépôt, sauf variable de l'egg.
: "${VERSION:=$(sed -n 's/^VERSION=//p' /usr/local/share/socialvideodownload/env.example | tr -d '\r')}"
export VERSION
CMD=$(printf '%s' "${STARTUP:-socialvideodownload}" | sed -e 's/{{/${/g' -e 's/}}/}/g')
printf ':/home/container$ %s\n' "$CMD"
eval "exec $CMD"
