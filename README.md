<div align="center">

# 🎬 SocialVideoDownload.py

Bot Telegram de téléchargement vidéo/audio depuis les réseaux sociaux (YouTube, TikTok, etc.) via `yt-dlp`.

[![CI](https://img.shields.io/github/actions/workflow/status/OverStyleFR/SocialVideoDownload.py/deploy.yml?branch=main&label=CI&logo=github)](https://github.com/OverStyleFR/SocialVideoDownload.py/actions)
[![Python](https://img.shields.io/badge/python-3.11-blue?logo=python)](https://www.python.org/)
[![Docker](https://img.shields.io/badge/docker-ghcr.io-blue?logo=docker)](https://github.com/OverStyleFR/SocialVideoDownload.py/pkgs/container/socialvideodownload.py)
[![Telegram](https://img.shields.io/badge/Telegram-%40yt__dlp__telegram__bot-2CA5E0?logo=telegram)](https://t.me/yt_dlp_telegram_bot)

---

**📦 Déployé via GitHub Container Registry** · Multi-arch (`linux/amd64`, `linux/arm64`)

</div>

---

## 🎯 Fonctionnalités

- **📥 Téléchargement vidéo** – `/download <lien>`
- **🎵 Extraction audio MP3** – `/music <lien>`
- **⚡ Auto-détection** – Envoie un lien directement, le bot télécharge automatiquement
- **📤 Upload intelligent** – Envoi Telegram direct (< 35 Mo) ou upload externe via `curl.libriciel.fr`
- **♻️ Rétention configurable** – Nettoyage automatique basé sur la taille et le type de fichier
- **🔁 Déduplication** – Évite de retélécharger les mêmes URLs (hash SHA-256)
- **🗑️ Nettoyage périodique** – Thread de fond qui libère l'espace disque

---

## 📦 Démarrage rapide

### 🐳 Docker Compose (recommandé)

```bash
cp .env.example .env
# Éditer .env : renseigner BOT_TOKEN
docker compose up -d --build
```

Le token peut aussi être passé via variable d'environnement (prioritaire sur `.env`) :

```bash
BOT_TOKEN=123456:ABC... docker compose up -d --build
```

### 🐍 Python Standalone

```bash
python3.11 -m venv venv
venv/bin/pip install --upgrade pip setuptools wheel
venv/bin/pip install -r requirements.txt
venv/bin/python main.py
```

---

## 🐳 Déploiement Docker

### Docker Compose

Construire et lancer :

```bash
docker compose up -d --build
```

Logs :

```bash
docker compose logs -f
```

Arrêter :

```bash
docker compose down
```

Volumes montés localement : `logs/`, `downloads/`, `download_temp/`.

### Images disponibles

| Branche  | Tags Docker |
|----------|-------------|
| `main`   | `latest`, `VERSION` |
| `develop` | `dev` |

Toutes les images : `ghcr.io/overstylefr/socialvideodownload.py`

---

## ⚙️ Configuration

Copier le fichier modèle et éditer :

```bash
cp .env.example .env
```

### Variables d'environnement

| Variable | Obligatoire | Description | Défaut |
|----------|:-----------:|-------------|--------|
| `BOT_TOKEN` | ✅ Oui | Token Telegram du bot | — |
| `VERSION` | ❌ Non | Version affichée dans `/help` et `/stats` | `unknown` |
| `DEVELOPED_BY` | ❌ Non | Crédit auteur | `Tom V. \| OverStyleFR` |
| `FFMPEG_PATH` | ❌ Non | Chemin vers le binaire FFmpeg | `ffmpeg/ffmpeg-7.0.2-amd64-static/ffmpeg` (→ fallback `"ffmpeg"`) |
| `MIN_FREE_SPACE_MB` | ❌ Non | Seuil d'espace disque minimal avant nettoyage d'urgence | `500` |
| `CLEANUP_INTERVAL_HOURS` | ❌ Non | Intervalle entre les nettoyages périodiques | `24` |
| `SMALL_FILE_SIZE_MB` | ❌ Non | Seuil pour considérer un fichier comme "petit" | `12` |
| `RETENTION_SMALL_HOURS` | ❌ Non | Rétention des petits fichiers + MP3 | `24` |
| `RETENTION_LARGE_HOURS` | ❌ Non | Rétention des gros fichiers | `2` |

Le fichier `.env` est la **source unique de configuration**, prioritaire sur les variables d'environnement système.

---

## 🛠️ Commandes Telegram

| Commande | Description |
|----------|-------------|
| `/start` | Message d'accueil |
| `/help` | Aide détaillée |
| `/download <lien>` | Télécharge une vidéo |
| `/music <lien>` | Télécharge et convertit l'audio en MP3 |
| Lien direct | Téléchargement automatique (texte libre) |

---

## 📁 Structure du projet

```
├── main.py               # Point d'entrée
├── config.py              # Configuration (.env)
├── commands/
│   ├── start.py           # /start
│   ├── help.py            # /help
│   ├── download.py        # /download
│   ├── music.py           # /music
│   ├── stats.py           # /stats
│   └── auto_download.py   # Auto-détection de liens
└── utils/
    ├── cache.py           # Cache de métadonnées
    ├── disk_manager.py    # Nettoyage disque
    ├── file_manager.py    # Déduplication (SHA-256)
    ├── logger.py          # Logs console + fichier
    ├── progress_file.py   # Wrapper de progression pour upload
    ├── retention.py       # Politique de rétention
    ├── token_loader.py    # Chargement du token
    ├── upload.py          # Upload Telegram
    └── curl_uploader.py   # Upload externe (curl)
```

---

## 📚 Documentation technique

Voir [`DOCS.md`](DOCS.md) pour les détails d'architecture, le flux applicatif, les notes de compatibilité et le dépannage.

---

## 📄 Licence

Projet personnel développé par [Tom V. | OverStyleFR](https://github.com/OverStyleFR).

<p align="right"><a href="#top">⬆ Retour en haut</a></p>
