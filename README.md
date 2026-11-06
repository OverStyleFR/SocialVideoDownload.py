# SocialVideoDownload

Bot Telegram qui télécharge une vidéo ou la bande son d'un lien (YouTube, TikTok, Instagram, X/Twitter, autres sites pris en charge par yt-dlp) et la renvoie dans la conversation. Écrit en Rust (teloxide, tokio) ; `yt-dlp` et `ffmpeg` sont des programmes externes.

Documentation liée : [AGENTS.md](AGENTS.md) (consignes pour agents), [DOCS.md](DOCS.md) (architecture et exploitation), [fiche de la refonte](docs/refonte-rust/FICHE.md), [mesures](docs/refonte-rust/STATS.md), [ingestion](docs/refonte-rust/INGEST.md), [audit 1:1 avec le bot Python](docs/refonte-rust/AUDIT-1-1.md).

## Commandes du bot

| Commande | Effet |
|---|---|
| `/start` | message d'accueil |
| `/help` | liste des commandes |
| `/download <lien>` | télécharge la vidéo (MP4) |
| `/music <lien>` | extrait l'audio en MP3 |
| `/stats` | statistiques, réservé aux administrateurs |
| `/settings` | réglages par menu de boutons, réservé aux administrateurs |
| un message contenant un lien | télécharge le premier lien comme `/download` (messages édités compris) |

Un fichier plus gros que la limite d'envoi de Telegram (`send.telegram_limit_mb`) part, comme dans le bot Python, chez l'hébergeur externe (`EXTERNAL_UPLOAD_URL`, `https://curl.libriciel.fr/` par défaut), et le bot répond avec le lien. En option : recompression d'abord (`send.compress`) et accord de l'utilisateur par boutons Oui/Non avant l'envoi (`EXTERNAL_UPLOAD_CONSENT=1`). `EXTERNAL_UPLOAD_URL=off` (ou vide), ou `send.external_fallback` coupé dans `/settings`, désactive le repli : le bot répond alors que le fichier est trop gros.

## Configuration

Toute la configuration vient de l'environnement (`src/config.rs`) ; modèle : `.env.example`. Seul `BOT_TOKEN` est obligatoire. Un booléen accepte `1/true/yes/on` et `0/false/no/off`.

| Variable | Défaut | Rôle |
|---|---|---|
| `BOT_TOKEN` | obligatoire | jeton du bot (jamais journalisé) |
| `TELEGRAM_API_URL` | `https://api.telegram.org` | API Bot (surchargée par le faux Telegram des tests) |
| `ALLOWED_USER_IDS` | vide | IDs Telegram numériques autorisés, séparés par des virgules. **Vide, le bot est ouvert à tous, comme le bot Python** ; renseignée, seuls ces IDs et les administrateurs l'utilisent |
| `ALLOW_ALL_USERS` | `false` | ouvre le bot à tout le monde même avec `ALLOWED_USER_IDS` |
| `ADMIN_USER_IDS` | vide | IDs numériques des administrateurs : `/stats` et `/settings`, et accès au bot même hors `ALLOWED_USER_IDS` ; ne restreint pas l'accès des autres |
| `RATE_LIMIT_PER_MIN` | `10` | liens par utilisateur et par minute, une fois la rafale épuisée (seau de jetons) |
| `RATE_LIMIT_BURST` | `20` | liens qu'un utilisateur peut envoyer d'affilée avant d'être limité |
| `MAX_CONCURRENT_DOWNLOADS` | `3` | téléchargements simultanés (minimum 1) |
| `MAX_FILESIZE_MB` | `500` | taille maximale téléchargée |
| `MAX_DURATION_S` | `3600` | durée maximale d'un média (les directs sont refusés) |
| `TELEGRAM_UPLOAD_LIMIT_MB` | `50` | limite d'envoi de l'API Bot (toujours 50 Mo sur l'API publique de Telegram, vérifié en novembre 2026 ; jusqu'à 2000 avec un serveur Bot API local) ; au-delà, repli externe (le bot Python basculait à 35 Mo) |
| `DOWNLOAD_DIR` | `./downloads` | racine des dossiers de travail |
| `LOGS_DIR` | `<DOWNLOAD_DIR>/../logs` (`./logs`) | journaux `AAAA-MM-JJ.log`, un fichier par jour en plus de la console, comme le bot Python |
| `RETENTION_SMALL_HOURS` | `24` | conservation des mp3 et des fichiers < `SMALL_FILE_SIZE_MB` (nom du bot Python) |
| `RETENTION_LARGE_HOURS` | `2` | conservation des gros fichiers (nom du bot Python) |
| `SMALL_FILE_SIZE_MB` | `12` | seuil petit/gros (nom du bot Python) |
| `RETENTION_HOURS` | absente | fixe les deux rétentions d'un coup si les deux lignes ci-dessus manquent |
| `CLEANUP_INTERVAL_HOURS` | `24` | intervalle du nettoyage (nom du bot Python) |
| `MIN_FREE_SPACE_MB` / `MIN_FREE_DISK_MB` | `500` | espace libre visé par le nettoyage (les deux noms sont lus) |
| `VERSION` / `DEVELOPED_BY` | `unknown` / `Tom V. \| OverStyleFR` | affichés par `/help`, `VERSION` aussi par `sys.version` (noms du bot Python) |
| `YTDLP_PATH` | `yt-dlp` | binaire yt-dlp |
| `FFMPEG_PATH` | `ffmpeg` | binaire ffmpeg ; un chemin inexistant (ex. celui du bot Python) retombe sur `ffmpeg` du PATH |
| `FFPROBE_PATH` | `ffprobe` | binaire ffprobe |
| `YTDLP_UPDATE_INTERVAL_H` | `24` | intervalle de mise à jour de yt-dlp (`0` = jamais) |
| `EXTERNAL_UPLOAD_URL` | `https://curl.libriciel.fr/` | hébergeur du repli externe, celui du bot Python ; vide ou `off` = repli coupé |
| `EXTERNAL_UPLOAD_CONSENT` | `true` | demande l'accord de l'utilisateur (boutons Oui/Non) avant chaque envoi externe ; `0`, ou « Accord permanent » dans `/settings`, envoie sans redemander comme le bot Python |
| `EXTERNAL_UPLOAD_TIMEOUT_S` | `300` | délai de l'envoi externe |
| `CACHE_TTL_H` | `24` | durée de vie du cache des `file_id` |
| `HEARTBEAT_FILE` | `/tmp/svd-heartbeat` | fichier de battement de cœur lu par le healthcheck |
| `SETTINGS_FILE` | `<DOWNLOAD_DIR>/../settings.json`, ou `./data/settings.json` | fichier JSON des réglages de `/settings` (voir plus bas) |
| `UNSAFE_ALLOW_PRIVATE_HOSTS` | `false` | autorise les adresses privées : tests et bancs seulement |

Variables lues dans l'environnement du processus seulement (pas dans `.env` en local ; le compose les y met depuis `.env`) : `LOG_FORMAT=json` (journaux JSON, sinon texte au format du bot Python), `RUST_LOG` (filtre, l'emporte sur `sys.log_level` ; défaut `debug` pour le bot et `info` pour les bibliothèques), `NO_COLOR` (console sans couleurs ; le fichier de journal n'en a jamais). `VERSION` est lue par le bot (`/help`, `sys.version`) et par la CI (étiquettes d'image).

`RETENTION_SMALL_HOURS`, `RETENTION_LARGE_HOURS` (ou `RETENTION_HOURS`), `MIN_FREE_SPACE_MB`, `CLEANUP_INTERVAL_HOURS` et `YTDLP_UPDATE_INTERVAL_H` donnent les défauts des réglages correspondants de `/settings`, que le fichier `SETTINGS_FILE` surcharge. Au démarrage, le dossier de téléchargements est vidé (comme le bot Python), puis le nettoyage périodique (premier passage immédiat) et la mise à jour de yt-dlp sont lancés ; l'espace libre est aussi vérifié avant chaque téléchargement.

## Réglages à chaud : `/settings`

`/settings` ouvre un menu de boutons, édité en place dans le même message. Il est réservé aux administrateurs, reconnus par leur ID Telegram numérique (liste `acc.admins`, qui part de `ADMIN_USER_IDS`) ; jamais par le pseudo. Un non-admin reçoit « Commande réservée aux administrateurs. » et aucun clavier. Le statut d'admin est revérifié à chaque clic, bouton forgé compris.

Arborescence (clé, bornes ou valeurs, défaut ; **chaud** = lu à l'usage, **redémarrage** = pris au prochain démarrage). Les défauts « env » viennent de la variable indiquée.

```
/settings
├── Téléchargement (dl)
│   ├── dl.max_filesize_mb     1 à 4000, env MAX_FILESIZE_MB (500)   chaud
│   ├── dl.max_duration_s      10 à 86400, env MAX_DURATION_S (3600) chaud
│   └── dl.quality             best | 1080 | 720 | 480, défaut best  chaud
├── Sources (providers)
│   ├── providers.order        ordre x, bluesky, tiktok              chaud
│   ├── providers.x.enabled    on/off, défaut off                    chaud
│   ├── providers.bluesky.enabled  on/off, défaut off                chaud
│   ├── providers.tiktok.enabled   on/off, défaut off                chaud
│   └── providers.ytdlp_fallback   on/off, défaut on                 chaud
├── Audio (audio)
│   ├── audio.music_enabled    on/off, défaut on                     chaud
│   └── audio.bitrate_kbps     64 à 320, défaut 192                  chaud
├── Envoi (send)
│   ├── send.telegram_limit_mb 1 à 2000, env TELEGRAM_UPLOAD_LIMIT_MB (50)  chaud
│   ├── send.external_fallback on/off, défaut on (off si EXTERNAL_UPLOAD_URL=off)  chaud
│   ├── send.external_always_yes  on/off, défaut off (on si EXTERNAL_UPLOAD_CONSENT=0)  chaud
│   └── send.compress          on/off, défaut off                    chaud
├── Stockage (sto)
│   ├── sto.retention_small_h  1 à 720 h, env RETENTION_SMALL_HOURS (24)  chaud
│   ├── sto.retention_large_h  1 à 720 h, env RETENTION_LARGE_HOURS (2)   chaud
│   ├── sto.min_free_mb        0 à 1000000, env MIN_FREE_SPACE_MB (500)  chaud
│   └── sto.cleanup_min        1 à 10080 min, env CLEANUP_INTERVAL_HOURS (24 h)  redémarrage
├── Accès (acc)
│   ├── acc.allowed_users      IDs numériques, env ALLOWED_USER_IDS  chaud
│   ├── acc.rate_limit_per_min 1 à 1000, env RATE_LIMIT_PER_MIN (10)  chaud
│   ├── acc.rate_limit_burst   1 à 1000, env RATE_LIMIT_BURST (20)   chaud
│   └── acc.admins             IDs numériques, jamais vide, env ADMIN_USER_IDS  chaud
└── Système (sys)
    ├── sys.version            lecture seule, VERSION (comme /help)
    ├── sys.ytdlp_update_h     0 à 720 h, env YTDLP_UPDATE_INTERVAL_H (24)  redémarrage
    └── sys.log_level          error | warn | info | debug, défaut debug   redémarrage
```

Navigation : racine (7 rubriques) puis rubrique (réglages et valeurs) puis réglage (description, valeur, bornes, avertissement, « Appliqué à chaud » ou « Effet au prochain redémarrage », boutons de valeur ou saisie). Chaque niveau a « ⬅️ Retour ». Pour une saisie, le prochain texte du même admin dans le même chat sert de valeur ; une valeur hors bornes est refusée avec « ❌ » et la raison, rien n'est enregistré.

Persistance : le fichier `SETTINGS_FILE` (JSON, droits 0600, écriture atomique) est relu au démarrage et **l'emporte sur le `.env`** pour les clés qu'il contient. Chaque changement est consigné dans `settings-audit.jsonl`, à côté de ce fichier (clé, valeurs, ID de l'admin).

Effets réels : tous les réglages sont branchés. À chaud : `dl.*`, `providers.*`, `audio.*`, `send.*`, `sto.retention_*`, `sto.min_free_mb` et `acc.*`. Au prochain démarrage : `sto.cleanup_min`, `sys.ytdlp_update_h` et `sys.log_level` (que `RUST_LOG` surcharge). Le nettoyage distingue petits fichiers (mp3 ou moins de `SMALL_FILE_SIZE_MB`) et gros fichiers, comme le bot Python.

## Sources hybrides

Par défaut, tout passe par yt-dlp seul : les sources X, Bluesky et TikTok sont **désactivées**. On les active une à une dans `/settings`, rubrique Sources. Une source activée est essayée avant yt-dlp, dans l'ordre de `providers.order`, pour les téléchargements vidéo (pas `/music`) :

| Source | Service interrogé | Remarque |
|---|---|---|
| X | API publique de type fxtwitter (`src/providers/x_api.rs`) | média direct |
| Bluesky | API publique Bluesky (`src/providers/bluesky.rs`) | les flux `.m3u8` repassent par yt-dlp |
| TikTok | API tierce de type tikwm (`src/providers/tiktok_api.rs`) | **attention : la vidéo passe par un service externe tiers**, hors de notre contrôle |

Si la source échoue, le bot retombe sur yt-dlp tant que `providers.ytdlp_fallback` est actif (défaut). Désactivé, l'erreur est renvoyée à l'utilisateur. Le fichier direct respecte les mêmes bornes de taille et de durée que yt-dlp.

## Lancement local

Prérequis : Rust 1.99, `yt-dlp`, `ffmpeg` et `ffprobe` dans le `PATH` (ou chemins via `YTDLP_PATH`, `FFMPEG_PATH`, `FFPROBE_PATH`).

```bash
cp .env.example .env     # renseigner BOT_TOKEN (sans .env, le bot en crée un et dit quoi faire)
cargo run --release      # lit ./.env lui-même, comme load_dotenv du bot Python
```

## Docker

```bash
cp .env.example .env
docker compose up -d --build
```

L'image embarque le binaire, `ffmpeg`/`ffprobe` statiques et un `yt-dlp` figé (somme sha256 vérifiée à la construction). Lancée seule, elle tourne en UID 10001 ; elle porte un `HEALTHCHECK` (`socialvideodownload --healthcheck`, lit l'âge de `HEARTBEAT_FILE`). Le compose la lance en root, comme le bot Python, pour que `./downloads`, `./logs` et `./state` (montés dans `/data`) soient inscriptibles sans `chown`, avec une seule capacité : `DAC_OVERRIDE` (sans elle, root est refusé sur un dossier de l'hôte qui ne lui appartient pas, essai Docker du 06/11/2026). Il ajoute : système de fichiers en lecture seule, `/tmp` en tmpfs, `no-new-privileges`, 512 Mo de mémoire, 1 CPU, 200 processus, redémarrage `unless-stopped`.

## Tests et CI

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

`cargo test` lance les tests unitaires et les tests d'intégration (`tests/e2e.rs`, `tests/e2e_settings.rs`, `tests/e2e_providers.rs`, `tests/startup.rs`, `tests/token_redaction.rs`) ; les e2e font tourner le vrai bot contre un faux serveur Telegram (`tests/fake_telegram/mod.rs`) avec de faux `yt-dlp`, `ffmpeg` et `ffprobe` (`tests/fixtures/`). La CI (`.github/workflows/ci-rust.yml`, runners `ubuntu-latest`) enchaîne `check` (fmt, clippy, tests), `audit` (`cargo audit --deny warnings`), `image` (build Docker puis scan Trivy CRITICAL/HIGH) et `publish` (push GHCR amd64 + arm64 après les trois autres : `latest`, `VERSION` et SHA sur `main`, `dev` et SHA sur `develop`).

## Sécurité

- Ouvert à tous par défaut, comme le bot Python ; liste blanche optionnelle (`ALLOWED_USER_IDS`) ; administrateurs par ID numérique, jamais par pseudo.
- Limite de débit par utilisateur en seau de jetons (20 liens d'affilée, puis 10 par minute : `RATE_LIMIT_BURST`, `RATE_LIMIT_PER_MIN`) et plafond de téléchargements simultanés.
- Anti-SSRF : seules les URL http(s) sont acceptées, le nom est résolu et toute adresse privée, loopback, link-local, CGNAT, ULA ou multicast est refusée.
- Repli externe vers l'hébergeur du bot Python par défaut, avec accord Oui/Non de l'utilisateur avant chaque envoi ; « Accord permanent » dans `/settings` (ou `EXTERNAL_UPLOAD_CONSENT=0`) pour ne plus redemander ; coupure par `EXTERNAL_UPLOAD_URL=off` ou dans `/settings`.
- Image en UID 10001 ; compose en root comme le bot Python, mais avec la seule capacité `DAC_OVERRIDE`, système de fichiers en lecture seule et `no-new-privileges` ; jeton masqué dans les journaux (console et fichier).

## Mise à jour de yt-dlp

L'image fige la version de yt-dlp (`YTDLP_VERSION` et `YTDLP_SHA256` dans le `Dockerfile`) : pour la changer, modifier ces deux arguments puis reconstruire. Le code sait aussi lancer `yt-dlp --update-to stable` (`src/download.rs`), mais dans l'image le binaire est en lecture seule, donc seul un avertissement serait journalisé. Intervalle : `YTDLP_UPDATE_INTERVAL_H` ou `sys.ytdlp_update_h` dans `/settings` (`0` = jamais).

## Dépannage TikTok

Quand les téléchargements TikTok échouent (`Unexpected response from webpage request`, blocage des IP de centres de données) :

1. Vérifier la version : `docker compose exec bot yt-dlp --version`.
2. Reconstruire l'image avec la dernière version de yt-dlp (voir ci-dessus) ; TikTok change souvent ses protections.
3. Le bot envoie déjà un User-Agent Chrome et le Referer `https://www.tiktok.com/` (`src/providers/mod.rs`, `src/providers/tiktok.rs`).
4. Chercher le message exact dans les tickets de yt-dlp.

## Licence

MIT (voir `Cargo.toml`).
