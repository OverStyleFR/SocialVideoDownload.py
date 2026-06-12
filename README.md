# SocialVideoDownload.py

Bot Telegram permettant de télécharger des vidéos et musiques depuis des liens de réseaux sociaux via `yt-dlp`.

Le bot peut fonctionner de trois façons :

- **Docker Compose classique**
- **Standalone Python**
- **Egg Pelican / Pterodactyl**

La configuration est centralisée dans un seul fichier : `.env`.

## Fonctionnalités

- Téléchargement vidéo via `/download <lien>`
- Extraction audio MP3 via `/music <lien>`
- Téléchargement automatique lorsqu'un lien est envoyé directement au bot
- Envoi Telegram direct pour les petits fichiers
- Upload externe via `curl.libriciel.fr` pour les fichiers trop volumineux
- Cache et déduplication des URLs téléchargées
- Nettoyage périodique du dossier `downloads/`
- Gestion de rétention selon la taille et le type de fichier

## Commandes Telegram

| Commande | Description |
|---|---|
| `/start` | Affiche le message d'accueil |
| `/help` | Affiche l'aide du bot |
| `/download <lien>` | Télécharge une vidéo |
| `/music <lien>` | Télécharge et convertit l'audio en MP3 |
| Lien direct | Le bot tente automatiquement le téléchargement vidéo |

## Prérequis

- Un token Telegram Bot obtenu via [@BotFather](https://t.me/BotFather)
- Python 3.11 si utilisation standalone
- Docker / Docker Compose si utilisation containerisée
- FFmpeg disponible dans l'environnement d'exécution

## Configuration

Copier l'exemple :

```bash
cp .env.example .env
```

Puis éditer au minimum :

```env
BOT_TOKEN=YOUR_TELEGRAM_BOT_TOKEN_HERE
```

Le même fichier `.env` est utilisé pour :

- Docker Compose
- Pelican / Pterodactyl
- Lancement Python standalone

## Utilisation avec Docker Compose

Construire et lancer le bot :

```bash
docker compose up -d --build
```

Voir les logs :

```bash
docker compose logs -f
```

Arrêter le bot :

```bash
docker compose down
```

Les dossiers suivants sont montés en volumes locaux :

- `logs/`
- `downloads/`
- `download_temp/`

## Utilisation standalone Python

Installer les dépendances :

```bash
python3.11 -m venv venv
venv/bin/pip install --upgrade pip setuptools wheel
venv/bin/pip install -r requirements.txt
```

Lancer le bot :

```bash
venv/bin/python main.py
```

## Utilisation avec Pelican / Pterodactyl

Le dépôt fournit un egg :

```text
egg-socialvideodownload.json
```

Principe :

1. Importer l'egg dans Pelican / Pterodactyl
2. Créer un serveur avec l'image `ghcr.io/overstylefr/socialvideodownload.py:pelican`
3. L'installation clone le dépôt dans le volume du serveur
4. Le serveur crée un virtualenv Python dans `venv/`
5. Le bot lit la configuration depuis `.env`

Après installation, éditer le fichier `.env` depuis le File Manager, puis démarrer le serveur.

## Variables principales

| Variable | Description | Exemple |
|---|---|---|
| `BOT_TOKEN` | Token Telegram du bot | `123456:ABC...` |
| `VERSION` | Version affichée dans `/help` | `V9.1` |
| `DEVELOPED_BY` | Auteur affiché dans `/help` | `Tom V. \| OverStyleFR` |
| `FFMPEG_PATH` | Chemin vers le binaire FFmpeg | `/usr/bin/ffmpeg` |
| `CLEANUP_INTERVAL_HOURS` | Intervalle de nettoyage périodique | `48` |
| `MIN_FREE_SPACE_MB` | Seuil d'espace disque libre minimal | `500` |
| `SMALL_FILE_SIZE_MB` | Taille max d'un petit fichier | `4` |
| `RETENTION_SMALL_HOURS` | Rétention des petits fichiers | `24` |
| `RETENTION_LARGE_HOURS` | Rétention des gros fichiers | `2` |

## Images Docker

| Usage | Image |
|---|---|
| Docker classique | Build local depuis `Dockerfile.compose` |
| Pelican / Pterodactyl | `ghcr.io/overstylefr/socialvideodownload.py:pelican` |
| Image applicative CI/CD | `ghcr.io/overstylefr/socialvideodownload.py:latest` |

## Documentation technique

Voir [`DOCS.md`](DOCS.md).

## Licence

Projet personnel développé par Tom V. | OverStyleFR.
