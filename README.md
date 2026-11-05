# SocialVideoDownload

Bot Telegram qui télécharge une vidéo ou la bande son d'un lien (YouTube, TikTok, Instagram, X/Twitter, autres sites pris en charge par yt-dlp) et la renvoie dans la conversation. Écrit en Rust (teloxide, tokio) ; `yt-dlp` et `ffmpeg` sont des programmes externes.

Documentation liée : [AGENTS.md](AGENTS.md) (consignes pour agents), [DOCS.md](DOCS.md) (architecture et exploitation), [fiche de la refonte](docs/refonte-rust/FICHE.md), [mesures](docs/refonte-rust/STATS.md), [ingestion](docs/refonte-rust/INGEST.md).

## Commandes du bot

| Commande | Effet |
|---|---|
| `/start` | message d'accueil |
| `/help` | liste des commandes |
| `/download <lien>` | télécharge la vidéo (MP4) |
| `/music <lien>` | extrait l'audio en MP3 |
| `/stats` | statistiques, réservé aux administrateurs |
| un message contenant un lien | télécharge le premier lien comme `/download` (messages édités compris) |

Un fichier plus gros que la limite d'envoi de Telegram est d'abord recompressé ; s'il ne tient toujours pas, le bot demande un consentement (boutons Oui/Non) avant de l'envoyer chez l'hébergeur externe configuré. Sans `EXTERNAL_UPLOAD_URL`, le repli est désactivé.

## Configuration

Toute la configuration vient de l'environnement (`src/config.rs`) ; modèle : `.env.example`. Seul `BOT_TOKEN` est obligatoire. Un booléen accepte `1/true/yes/on` et `0/false/no/off`.

| Variable | Défaut | Rôle |
|---|---|---|
| `BOT_TOKEN` | obligatoire | jeton du bot (jamais journalisé) |
| `TELEGRAM_API_URL` | `https://api.telegram.org` | API Bot (surchargée par le faux Telegram des tests) |
| `ALLOWED_USER_IDS` | vide | IDs Telegram numériques autorisés, séparés par des virgules ; vide = personne |
| `ALLOW_ALL_USERS` | `false` | ouvre le bot à tout le monde |
| `ADMIN_USER_IDS` | vide | IDs numériques autorisés à `/stats` |
| `RATE_LIMIT_PER_MIN` | `5` | requêtes par utilisateur et par minute |
| `MAX_CONCURRENT_DOWNLOADS` | `3` | téléchargements simultanés (minimum 1) |
| `MAX_FILESIZE_MB` | `500` | taille maximale téléchargée |
| `MAX_DURATION_S` | `3600` | durée maximale d'un média (les directs sont refusés) |
| `TELEGRAM_UPLOAD_LIMIT_MB` | `50` | limite d'envoi de l'API Bot ; au-delà, compression puis repli externe |
| `DOWNLOAD_DIR` | `./downloads` | racine des dossiers de travail |
| `RETENTION_HOURS` | `24` | durée de conservation des dossiers de travail |
| `MIN_FREE_DISK_MB` | `500` | espace libre visé par le nettoyage |
| `YTDLP_PATH` | `yt-dlp` | binaire yt-dlp |
| `FFMPEG_PATH` | `ffmpeg` | binaire ffmpeg |
| `FFPROBE_PATH` | `ffprobe` | binaire ffprobe |
| `YTDLP_UPDATE_INTERVAL_H` | `24` | intervalle de mise à jour de yt-dlp (`0` = jamais) |
| `EXTERNAL_UPLOAD_URL` | absente | hébergeur externe du repli ; absente = repli désactivé |
| `EXTERNAL_UPLOAD_TIMEOUT_S` | `300` | délai de l'envoi externe |
| `CACHE_TTL_H` | `24` | durée de vie du cache des `file_id` |
| `HEARTBEAT_FILE` | `/tmp/svd-heartbeat` | fichier de battement de cœur lu par le healthcheck |
| `UNSAFE_ALLOW_PRIVATE_HOSTS` | `false` | autorise les adresses privées : tests et bancs seulement |

Variables lues hors `Config` : `LOG_FORMAT=json` (journaux JSON, sinon texte), `RUST_LOG` (filtre, défaut `info`). `VERSION` de `.env.example` n'est pas lue par le bot : seule la CI s'en sert.

Attention : `YTDLP_UPDATE_INTERVAL_H`, `RETENTION_HOURS` et `MIN_FREE_DISK_MB` n'ont pour l'instant aucun effet à l'exécution, car la mise à jour et le nettoyage périodiques ne sont pas lancés (voir [DOCS.md](DOCS.md#fonctions-non-branchées)).

## Lancement local

Prérequis : Rust 1.99, `yt-dlp`, `ffmpeg` et `ffprobe` dans le `PATH` (ou chemins via `YTDLP_PATH`, `FFMPEG_PATH`, `FFPROBE_PATH`).

```bash
cp .env.example .env     # renseigner BOT_TOKEN et ALLOWED_USER_IDS
set -a; . ./.env; set +a
cargo run --release
```

## Docker

```bash
cp .env.example .env
docker compose up -d --build
```

L'image embarque le binaire, `ffmpeg`/`ffprobe` statiques et un `yt-dlp` figé (somme sha256 vérifiée à la construction). Elle tourne en UID 10001 et porte un `HEALTHCHECK` (`socialvideodownload --healthcheck`, lit l'âge de `HEARTBEAT_FILE`). Le compose ajoute : système de fichiers en lecture seule, `/tmp` en tmpfs, `cap_drop: ALL`, `no-new-privileges`, 512 Mo de mémoire, 1 CPU, 200 processus, redémarrage `unless-stopped`. Les dossiers `./downloads` et `./logs` sont montés dans `/data`.

## Tests et CI

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

`cargo test` lance les tests unitaires et `tests/e2e.rs`, qui fait tourner le vrai bot contre un faux serveur Telegram (`tests/fake_telegram/mod.rs`) avec de faux `yt-dlp`, `ffmpeg` et `ffprobe` (`tests/fixtures/`). La CI (`.github/workflows/ci-rust.yml`, runners `[self-hosted, nas]`) enchaîne `check` (fmt, clippy, tests), `audit` (`cargo audit --deny warnings`), `image` (build Docker puis scan Trivy CRITICAL/HIGH) et `publish` (push GHCR, seulement sur `main`, après les trois autres).

## Sécurité

- Liste blanche d'IDs numériques ; jamais de décision sur un pseudo.
- Limite de débit par utilisateur (`RATE_LIMIT_PER_MIN`) et plafond de téléchargements simultanés.
- Anti-SSRF : seules les URL http(s) sont acceptées, le nom est résolu et toute adresse privée, loopback, link-local, CGNAT, ULA ou multicast est refusée.
- Aucun envoi chez un tiers sans le « Oui » de l'utilisateur.
- Conteneur non-root, système de fichiers en lecture seule, aucune capability, jeton masqué dans les journaux.

## Mise à jour de yt-dlp

L'image fige la version de yt-dlp (`YTDLP_VERSION` et `YTDLP_SHA256` dans le `Dockerfile`) : pour la changer, modifier ces deux arguments puis reconstruire. Le code sait aussi lancer `yt-dlp --update-to stable` (`src/download.rs`), mais dans l'image le binaire est en lecture seule, donc seul un avertissement serait journalisé.

## Dépannage TikTok

Quand les téléchargements TikTok échouent (`Unexpected response from webpage request`, blocage des IP de centres de données) :

1. Vérifier la version : `docker compose exec bot yt-dlp --version`.
2. Reconstruire l'image avec la dernière version de yt-dlp (voir ci-dessus) ; TikTok change souvent ses protections.
3. Le bot envoie déjà un User-Agent Chrome et le Referer `https://www.tiktok.com/` (`src/providers/mod.rs`, `src/providers/tiktok.rs`).
4. Chercher le message exact dans les tickets de yt-dlp.

## Licence

MIT (voir `Cargo.toml`).
