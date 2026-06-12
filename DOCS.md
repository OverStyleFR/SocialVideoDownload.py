# Documentation technique

## Vue d'ensemble

SocialVideoDownload.py est un bot Telegram synchrone basé sur `python-telegram-bot==13.7`.

Il utilise :

- `python-telegram-bot` v13 pour la réception et l'envoi de messages Telegram
- `yt-dlp` pour l'extraction et le téléchargement média
- `ffmpeg` pour la conversion audio MP3
- `python-dotenv` pour charger la configuration depuis `.env`
- `pycurl` pour l'upload externe de fichiers volumineux

Le projet cible **Python 3.11**. Les versions modernes de `python-telegram-bot` ne sont pas utilisées : le code est basé sur l'ancienne API synchrone v13 (`Updater`, `CommandHandler`, `Filters`).

## Structure du projet

```text
.
├── main.py
├── config.py
├── requirements.txt
├── Dockerfile
├── Dockerfile.compose
├── Dockerfile.pelican
├── docker-compose.yml
├── egg-socialvideodownload.json
├── .env.example
├── commands/
│   ├── start.py
│   ├── help.py
│   ├── download.py
│   ├── music.py
│   ├── stats.py
│   └── auto_download.py
└── utils/
    ├── token_loader.py
    ├── logger.py
    ├── file_manager.py
    ├── disk_manager.py
    ├── cache.py
    ├── retention.py
    ├── upload.py
    ├── curl_uploader.py
    └── progress_file.py
```

## Démarrage applicatif

Le point d'entrée est `main.py`.

Séquence principale :

1. Chargement du `.env` avec `override=True`
2. Chargement du cache local
3. Vérification de l'espace disque
4. Chargement du token Telegram
5. Création de l'`Updater`
6. Enregistrement des handlers Telegram
7. Configuration du menu Telegram
8. Démarrage du thread de nettoyage périodique
9. Lancement du polling Telegram

## Chargement de configuration

La configuration est lue depuis `.env` grâce à `python-dotenv`.

Les appels utilisent :

```python
load_dotenv(".env", override=True)
```

Cela signifie que le `.env` est prioritaire sur les variables d'environnement injectées par Docker, Pelican ou le système.

Fichiers concernés :

- `main.py`
- `config.py`
- `utils/token_loader.py`

## Variables d'environnement

| Variable | Utilisation |
|---|---|
| `BOT_TOKEN` | Token Telegram obligatoire |
| `VERSION` | Version affichée dans `/help` |
| `DEVELOPED_BY` | Auteur affiché dans `/help` |
| `FFMPEG_PATH` | Chemin vers FFmpeg pour `/music` |
| `CLEANUP_INTERVAL_HOURS` | Fréquence du nettoyage périodique |
| `MIN_FREE_SPACE_MB` | Seuil d'espace disque minimal |
| `SMALL_FILE_SIZE_MB` | Seuil de petit fichier |
| `RETENTION_SMALL_HOURS` | Rétention des petits fichiers et MP3 |
| `RETENTION_LARGE_HOURS` | Rétention des gros fichiers |

## Commandes Telegram

### `/download <url>`

Implémenté dans `commands/download.py`.

Flux :

1. Vérifie qu'une URL est fournie
2. Vérifie l'espace disque disponible
3. Vérifie si l'URL a déjà été téléchargée
4. Télécharge via `yt_dlp.YoutubeDL`
5. Sauvegarde le hash de l'URL
6. Applique la politique de rétention
7. Envoie le fichier via `utils/upload.py`

### `/music <url>`

Implémenté dans `commands/music.py`.

Flux :

1. Télécharge la vidéo avec `yt-dlp`
2. Convertit le fichier en MP3 via FFmpeg
3. Applique la politique de rétention
4. Envoie l'audio à l'utilisateur

### Liens directs

Implémenté dans `commands/auto_download.py`.

Le bot intercepte les messages texte non-commandes et tente un téléchargement automatique si un lien est détecté.

## Upload des fichiers

Implémenté dans `utils/upload.py`.

Deux cas :

| Taille | Comportement |
|---|---|
| `< 35 Mo` | Envoi direct via l'API Telegram |
| `> 35 Mo` | Upload externe vers `curl.libriciel.fr`, puis envoi du lien |

Le seuil Telegram est défini directement dans `utils/upload.py` :

```python
MAX_FILE_SIZE = 35 * 1024 * 1024
```

## Déduplication

Implémentée dans `utils/file_manager.py`.

Le bot stocke un hash SHA-256 des URLs téléchargées dans :

```text
downloads/hashes.txt
```

Cela permet d'éviter de retraiter inutilement les mêmes liens.

## Cache

Implémenté dans `utils/cache.py`.

Le cache est stocké dans :

```text
download_temp/cache_metadata.json
```

Il contient des métadonnées temporaires utilisées par le bot.

## Nettoyage disque

Implémenté dans `utils/disk_manager.py`.

Deux mécanismes existent :

1. Vérification au démarrage
2. Thread de nettoyage périodique lancé depuis `main.py`

Le nettoyage supprime les fichiers du dossier `downloads/` tout en préservant `hashes.txt`.

## Rétention des fichiers

Implémentée dans `utils/retention.py`.

La rétention est basée sur :

- La taille du fichier
- Le type de fichier
- Les variables `.env`

Les fichiers MP3 et petits fichiers ont une rétention plus longue que les gros fichiers.

## Déploiement Docker Compose

Fichiers concernés :

- `Dockerfile.compose`
- `docker-compose.yml`
- `.dockerignore`

Commande :

```bash
docker compose up -d --build
```

Le service utilise :

```yaml
env_file:
  - .env
```

et monte les volumes :

```yaml
volumes:
  - ./logs:/app/logs
  - ./downloads:/app/downloads
  - ./download_temp:/app/download_temp
```

## Déploiement Pelican / Pterodactyl

Fichiers concernés :

- `egg-socialvideodownload.json`
- `Dockerfile.pelican`
- `entrypoint.sh`

L'image Pelican dédiée est :

```text
ghcr.io/overstylefr/socialvideodownload.py:pelican
```

Elle est basée sur :

```text
python:3.11-slim-bookworm
```

et inclut :

- Python 3.11
- git
- ffmpeg
- curl
- ca-certificates

Le script d'installation de l'egg :

1. Clone le dépôt dans `/mnt/server`
2. Crée `venv/`
3. Installe les dépendances
4. Crée les dossiers persistants
5. Copie `.env.example` vers `.env` si nécessaire

Le startup lance ensuite `main.py` via le virtualenv.

## CI/CD

Le workflow GitHub Actions se trouve dans :

```text
.github/workflows/deploy.yml
```

Il build et pousse :

| Branche | Tags |
|---|---|
| `main` | `latest`, version courte, version complète, `pelican` |
| `develop` | `dev`, `pelican-dev` |

## Tests

Aucune suite de tests automatisée n'est configurée pour le moment.

Commande placeholder actuelle :

```bash
echo "No tests to run"
```

## Dépannage

### Le bot affiche une erreur `Conflict: terminated by other getUpdates request`

Cela signifie que deux instances du même bot tournent avec le même `BOT_TOKEN`.

Solution : arrêter l'autre instance ou utiliser un token différent.

### Le bot ne trouve pas FFmpeg

Vérifier `FFMPEG_PATH` dans `.env`.

Valeurs typiques :

```env
FFMPEG_PATH=/usr/bin/ffmpeg
```

ou :

```env
FFMPEG_PATH=/usr/local/bin/ffmpeg
```

### Pelican démarre mais rien ne s'affiche

Vérifier que l'image `ghcr.io/overstylefr/socialvideodownload.py:pelican` est bien à jour et que l'egg importé contient le startup récent.

### Erreur `pkg_resources` ou APScheduler

Le projet dépend de `python-telegram-bot==13.7`, qui utilise APScheduler 3.6.3. Pour éviter l'erreur `pkg_resources`, `requirements.txt` contient :

```text
setuptools<71
```

## Notes de compatibilité

- Ne pas migrer partiellement vers `python-telegram-bot` v20+.
- Ne pas introduire `async/await` sans réécrire toute l'architecture Telegram.
- Garder Python 3.11 comme cible principale.
- Préserver le `.env` comme source unique de configuration.
