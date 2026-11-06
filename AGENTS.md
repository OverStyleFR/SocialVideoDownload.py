# Consignes pour agents : SocialVideoDownload

Bot Telegram en **Rust** (teloxide 0.17, tokio) qui télécharge vidéo/audio via `yt-dlp` (externe) et `ffmpeg` (externe). L'ancien bot Python est retiré : ne pas y toucher ni le réintroduire. Branche de travail : `refonte-rust`.

Lire d'abord : [README.md](README.md) (variables et défauts), [DOCS.md](DOCS.md) (architecture, CI, sécurité), [docs/refonte-rust/FICHE.md](docs/refonte-rust/FICHE.md), [STATS.md](docs/refonte-rust/STATS.md), [INGEST.md](docs/refonte-rust/INGEST.md).

## Commandes

| Commande | Usage |
|---|---|
| `cargo run --release` | lance le bot (lit `./.env` sans écraser l'environnement, voir `.env.example`) |
| `cargo fmt --check` | format |
| `cargo clippy --locked --all-targets -- -D warnings` | lint, aucun avertissement toléré |
| `cargo test --locked` | tests unitaires et d'intégration (`tests/`) |
| `docker compose up -d --build` | exécution en conteneur |

La porte de livraison est la CI `.github/workflows/ci-rust.yml` : fmt, clippy, tests, `cargo audit`, build d'image + Trivy.

## Carte du code

- `src/main.rs` : point d'entrée, `--healthcheck`.
- `src/lib.rs` : déclare les modules `access`, `bot`, `cache`, `config`, `download`, `egress`, `error`, `media`, `metrics`, `providers`, `retention`, `settings`, `system`, `telemetry`, `url_guard`.
- `src/config.rs` : le module qui lit l'environnement (exceptions : `LOG_FORMAT`, `RUST_LOG` et `NO_COLOR` dans `src/telemetry.rs`, `HEARTBEAT_FILE` pour `--healthcheck` dans `src/main.rs`).
- `src/bot/` : état partagé, gestionnaires (`/start`, `/help`, `/download`, `/music`, `/stats`, `/settings`, texte libre, messages édités, boutons Oui/Non).
- `tests/e2e.rs`, `tests/fake_telegram/mod.rs`, `tests/fixtures/` : faux Telegram et faux outils.

## Règles

- Texte destiné aux utilisateurs, commentaires et documentation en **français**.
- Toute nouvelle variable : l'ajouter à `src/config.rs`, `.env.example` et au tableau du README (nom, défaut, rôle).
- Garder l'anti-SSRF (`src/url_guard.rs`) devant toute URL fournie par un utilisateur ; `UNSAFE_ALLOW_PRIVATE_HOSTS` ne sert qu'aux tests.
- Repli externe : seulement vers `EXTERNAL_UPLOAD_URL` (`https://curl.libriciel.fr/` par défaut, comme le bot Python), accord Oui/Non de l'utilisateur si `EXTERNAL_UPLOAD_CONSENT=1` ; les tests ne visent jamais le vrai hébergeur (`EXTERNAL_UPLOAD_URL=off` ou faux serveur).
- Ne jamais journaliser le jeton ; passer par `Cx::redact` (`src/bot/handlers.rs`) pour les erreurs réseau.
- Un correctif livre un test qui rejoue le vrai parcours (de préférence dans `tests/e2e.rs`).
- Aucune adresse IP, identifiant Telegram réel, pseudo ou commande d'accès à un serveur dans le dépôt : valeurs factices uniquement (`111111111`).
- `Dockerfile` : images et binaires épinglés (digest, sha256) ; ne pas les remplacer par des tags mobiles. Image en UID 10001 ; le compose la lance en root avec la seule capacité `DAC_OVERRIDE` (comme le bot Python, sans `chown`) : ne pas en ajouter d'autre.

## Pièges connus

- Avec `env_file: .env`, le compose écrase les `ENV` de l'image (`YTDLP_PATH`, `DOWNLOAD_DIR`…).
- teloxide 0.17 exige `mime_type` sur les médias renvoyés par l'API ; le faux Telegram le fournit.
- Les fixtures `.sh` ne sont exercées que sur Linux, les `.cmd` que sous Windows.

## Dépannage TikTok

Voir la section « Dépannage » de [DOCS.md](DOCS.md) : mettre yt-dlp à jour (arguments `YTDLP_VERSION` et `YTDLP_SHA256` du `Dockerfile`), vérifier `yt-dlp --version` dans le conteneur (`docker compose exec bot yt-dlp --version`), puis chercher l'erreur exacte dans les tickets de yt-dlp. User-Agent et Referer sont centralisés dans `src/providers/mod.rs` et `src/providers/tiktok.rs`.
