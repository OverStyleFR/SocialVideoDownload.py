# SocialVideoDownload.rs

Bot Telegram qui télécharge la vidéo ou la bande son d'un lien (YouTube, TikTok, Instagram, X/Twitter et les autres sites pris en charge par yt-dlp) et les renvoie dans la conversation.

Refonte en Rust de [SocialVideoDownload.py](https://github.com/OverStyleFR/SocialVideoDownload.py) : mêmes commandes, même `.env`, un bot plus robuste et réactif sous charge. `yt-dlp` et `ffmpeg` restent des programmes externes.

## Fonctionnalités

- Envoi de la vidéo (`/download`) ou de l'audio en MP3 (`/music`), ou simplement en collant un lien.
- Plusieurs utilisateurs en même temps : un téléchargement lent ne bloque pas les autres.
- Fichiers trop gros pour Telegram (plus de 50 Mo) : envoi vers un hébergeur externe, avec la demande d'accord de l'utilisateur.
- Limite de débit par utilisateur, cache des liens déjà traités, nettoyage automatique des fichiers.
- `/settings` : réglages modifiables à chaud par menu de boutons (administrateurs).
- Sources directes optionnelles pour X, Bluesky et TikTok, désactivées par défaut.

## Commandes

| Commande | Effet |
|---|---|
| `/start` | message d'accueil |
| `/help` | liste des commandes |
| `/download <lien>` | télécharge la vidéo |
| `/music <lien>` | extrait l'audio en MP3 |
| `/stats` | statistiques (administrateurs) |
| `/settings` | réglages par boutons (administrateurs) |
| un message contenant un lien | télécharge le premier lien, comme `/download` |

## Installation avec Docker

Prérequis : Docker avec le plugin Compose, et un jeton de bot obtenu auprès de [@BotFather](https://t.me/BotFather).

```bash
git clone https://github.com/OverStyleFR/SocialVideoDownload.rs.git
cd SocialVideoDownload.rs
cp .env.example .env
```

Dans `.env`, renseigner au minimum :

```ini
BOT_TOKEN=123456789:VOTRE_JETON
ADMIN_USER_IDS=111111111   # votre ID Telegram numérique (pour /stats et /settings)
```

Puis lancer le bot :

```bash
docker compose up -d --build
docker compose logs -f bot
```

Le bot est prêt quand les journaux affichent `[INIT] Démarrage du bot et lancement du polling...`. Si le jeton est refusé par Telegram, le bot s'arrête avec un message qui le dit.

Mise à jour :

```bash
git pull
docker compose up -d --build
```

Les dossiers `downloads/`, `logs/` et `state/` sont montés depuis l'hôte et restent en place lors des mises à jour.

## Configuration

Toute la configuration vient de l'environnement. `.env.example` liste chaque variable avec sa valeur par défaut et un commentaire. Seul `BOT_TOKEN` est obligatoire.

| Variable | Défaut | Rôle |
|---|---|---|
| `BOT_TOKEN` | obligatoire | jeton du bot (jamais écrit dans les journaux) |
| `ADMIN_USER_IDS` | vide | IDs numériques des administrateurs : `/stats` et `/settings` |
| `ALLOWED_USER_IDS` | vide | liste d'utilisateurs autorisés (IDs séparés par des virgules) |
| `RATE_LIMIT_BURST` / `RATE_LIMIT_PER_MIN` | `20` / `10` | liens d'affilée, puis liens par minute, par utilisateur |
| `MAX_CONCURRENT_DOWNLOADS` | `3` | téléchargements simultanés |
| `MAX_FILESIZE_MB` / `MAX_DURATION_S` | `500` / `3600` | taille et durée maximales d'un média |
| `TELEGRAM_UPLOAD_LIMIT_MB` | `50` | au-delà, le fichier part chez l'hébergeur externe |
| `EXTERNAL_UPLOAD_URL` | `https://curl.libriciel.fr/` | hébergeur du repli externe ; `off` le désactive |
| `EXTERNAL_UPLOAD_CONSENT` | `1` | demande l'accord avant chaque envoi externe ; `0` envoie sans demander |
| `RETENTION_SMALL_HOURS` / `RETENTION_LARGE_HOURS` | `24` / `2` | durée de conservation des fichiers (petits / gros) |

**Accès** : sans `ALLOWED_USER_IDS`, le bot est ouvert à tous, comme le bot Python. Renseignée, seuls ces IDs et les administrateurs peuvent l'utiliser.

### Réglages à chaud : `/settings`

`/settings` ouvre un menu de boutons, réservé aux administrateurs. Les réglages sont enregistrés dans `state/settings.json` et l'emportent sur le `.env`. Chaque changement est consigné dans `state/settings-audit.jsonl`. La plupart sont appliqués immédiatement ; le nettoyage et la mise à jour de yt-dlp sont pris au prochain démarrage.

Rubriques : Téléchargement, Sources, Audio, Envoi, Stockage, Accès, Système.

### Sources directes

Par défaut, tout passe par yt-dlp. Les sources X, Bluesky et TikTok s'activent une à une dans `/settings` › Sources. Une source activée est essayée avant yt-dlp ; en cas d'échec, yt-dlp prend le relais tant que le repli est actif.

Attention : la source TikTok passe par un service tiers, hors de contrôle du projet. Les API publiques de X et Bluesky peuvent changer sans préavis.

## Lancement sans Docker

Prérequis : Rust 1.99, et `yt-dlp`, `ffmpeg` et `ffprobe` dans le `PATH` (ou leurs chemins dans `YTDLP_PATH`, `FFMPEG_PATH`, `FFPROBE_PATH`).

```bash
cp .env.example .env
cargo run --release
```

## Tests

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Les tests de bout en bout font tourner le vrai bot contre un faux serveur Telegram, avec de faux `yt-dlp` et `ffmpeg`.

## Sécurité

- Les adresses privées, locales ou internes sont refusées (protection contre les requêtes vers le réseau interne).
- L'image est prévue pour un utilisateur non privilégié (UID 10001). Le compose la lance en root, comme le bot Python, avec une seule capacité Linux (`DAC_OVERRIDE`) pour écrire dans les dossiers montés, un système de fichiers en lecture seule et `no-new-privileges`.
- Le jeton n'apparaît jamais dans les journaux.

## Documentation

- [DOCS.md](DOCS.md) : architecture, fonctionnement détaillé, dépannage.
- [AGENTS.md](AGENTS.md) : consignes pour les agents qui travaillent sur le code.
- [docs/refonte-rust/](docs/refonte-rust/) : audit de l'ancien bot, mesures et comparaison avec la version Python.

## Crédits

- Développé par Tom V. | OverStyleFR
- Refonte Rust : Toji ([@TojiiSamaa](https://github.com/TojiiSamaa))

## Licence

MIT.
