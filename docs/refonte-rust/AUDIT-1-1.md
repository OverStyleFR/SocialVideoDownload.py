# Audit 1:1 : bot Python (`a467a68`) → bot Rust

Demande de Tom (05/11/2026) : « un vrai 1:1 » en fonctions et en affichage. Règle suivie : on améliore, on ne retire rien. Chaque fichier du bot Python est mis en face de son équivalent Rust. Les différences encore visibles sont listées plus bas avec leur raison, et les choix qui reviennent au mainteneur sont en fin de document.

Statuts : **identique** (même comportement et mêmes textes) ; **amélioré** (tout l'original, avec des ajouts) ; **différent** (comportement visible changé, raison donnée) ; **non repris** ; **sans objet** (propre à Python ou inutilisé).

## Fichier par fichier

| Fichier Python | Équivalent Rust | Statut | Détail |
|---|---|---|---|
| `main.py` | `src/main.rs`, `src/bot/mod.rs` | amélioré | `.env` lu sans écraser l'environnement (comme `load_dotenv(override=False)`), dossier de téléchargements vidé au démarrage (`clear_downloads`), mêmes lignes `[INIT]`, menu des 3 commandes. Une tâche par message : un téléchargement lent ne bloque plus les autres utilisateurs ; token refusé par Telegram : message clair et sortie en échec (le Python s'arrêtait sur une exception non rattrapée, DÉDUIT) |
| `config.py` | `src/config.rs` | amélioré | mêmes noms de variables et mêmes défauts (`VERSION`, `DEVELOPED_BY`, `MIN_FREE_SPACE_MB`, `CLEANUP_INTERVAL_HOURS`, `SMALL_FILE_SIZE_MB`, `RETENTION_SMALL_HOURS`, `RETENTION_LARGE_HOURS`) ; les nouvelles variables sont décrites dans le README |
| `commands/start.py`, `commands/help.py` | `start_text`, `help_text` (`src/bot/handlers.rs`) | identique | mêmes textes ; `/help` affiche `VERSION` et `DEVELOPED_BY` |
| `commands/download.py` | `src/bot/handlers.rs`, `src/download.rs` | amélioré | 3 tentatives, mêmes lignes de journal (« Tentative N de téléchargement pour l'URL… », « … échouée … ») ; bornes de taille et de durée en plus |
| `commands/music.py` | `src/bot/handlers.rs`, `src/media.rs` | amélioré | même mp3 (`libmp3lame`, 192 kb/s par défaut) ; seule la piste audio est téléchargée (le Python prenait la vidéo entière avant de la convertir) |
| `commands/auto_download.py` | `src/bot/handlers.rs` | amélioré | premier lien du message ; les messages modifiés sont aussi traités |
| `commands/stats.py` | `src/bot/stats.rs` | différent | mêmes rubriques, plus une rubrique Activité ; accès par ID numérique seulement (`ADMIN_USER_IDS`) : le pseudo et l'ID écrits en dur dans le code ne sont pas repris |
| `utils/upload.py` | `src/bot/handlers.rs`, `src/bot/progress.rs` | amélioré | même choix par extension (mp4/mkv/avi en vidéo, mp3/wav en audio, le reste en document), progression par paliers de 10 %, textes « Upload externe en cours » ; seuil du repli à 50 Mo au lieu de 35 (voir plus bas) |
| `utils/progress_file.py` | `ProgressRead` (`src/bot/progress.rs`) | identique | lecture du fichier par blocs pour la progression d'envoi |
| `utils/curl_uploader.py` | `src/egress.rs` | amélioré | `PUT` vers `https://curl.libriciel.fr/` par défaut, fichier renommé par son sha256, 3 essais ; en plus : code HTTP vérifié, réponse HTML refusée, délais bornés, accord Oui/Non en option (`EXTERNAL_UPLOAD_CONSENT=1`), coupure possible (`EXTERNAL_UPLOAD_URL=off`) |
| `utils/cache.py` | `src/cache.rs` | amélioré | le cache Python, en mémoire, ne servait qu'à `/stats` ; celui du Rust renvoie aussi un lien déjà envoyé par son `file_id`, sans nouveau téléchargement (`CACHE_TTL_H`) |
| `utils/disk_manager.py` | `src/retention.rs` | amélioré | deux paliers (petits et gros fichiers), `check_and_clean_if_needed` avant chaque téléchargement, lignes `[DISK_MANAGER]` ; le nettoyage d'urgence épargne les téléchargements en cours (le Python vidait tout) |
| `utils/retention.py` | `Tiers` (`src/retention.rs`) | différent | mêmes durées (24 h pour les petits fichiers, 2 h pour les gros, seuil 12 Mo, un mp3 compte comme petit) ; le Python mettait une date de modification dans le futur, le Rust calcule l'échéance sans toucher aux fichiers |
| `utils/file_manager.py` | `src/cache.rs`, `src/bot/stats.rs` | différent | `hashes.txt` est remplacé par le cache ; le compteur « URLs enregistrées » de `/stats` est gardé |
| `utils/helpers.py` | (aucun) | non repris | code mort : aucun appel dans le bot Python |
| `utils/logger.py` | `src/telemetry.rs` | amélioré | format `AAAA-MM-JJ HH:MM:SS,mmm - NIVEAU - message`, couleurs par catégorie en console, un fichier `AAAA-MM-JJ.log` par jour, niveau `debug`, anciens journaux conservés comme avant ; en plus : aucun code ANSI dans le fichier (point 16 de l'audit), jeton masqué, passage au fichier du jour suivant à minuit sans redémarrage (le Python nommait le fichier au démarrage) |
| `utils/token_loader.py` | `missing_token` (`src/main.rs`) | amélioré | `.env` absent : créé depuis le modèle, avec le message d'origine ; jeton modèle : message d'origine ; puis arrêt. En plus : si `.env` ne peut pas être créé (conteneur en lecture seule), le bot demande quand même de renseigner `BOT_TOKEN` |
| `utils/ytdlp_config.py` | `src/download.rs`, `src/providers/mod.rs` | amélioré | même User-Agent Chrome 128 et même Referer TikTok ; chaque plateforme reçoit son propre Referer (le Python envoyait celui de TikTok partout) ; fichier envoyé sous le titre de la vidéo |
| `utils/__init__.py`, `imghdr.py` | (aucun) | sans objet | fichiers propres à Python (paquet, remplaçant du module `imghdr`) |
| `requirements.txt` | `Cargo.toml`, `Cargo.lock` | sans objet | dépendances Rust épinglées |
| `Dockerfile` | `Dockerfile` | amélioré | ffmpeg embarqué, bot lancé au démarrage ; en plus : images de base épinglées par digest, `yt-dlp` figé et vérifié par sha256, `HEALTHCHECK`, image en UID 10001 |
| `Dockerfile.compose` | (aucun) | sans objet | inutilisé : le compose d'origine fait `build: .` |
| `docker-compose.yml` | `docker-compose.yml` | amélioré | `env_file: .env`, `BOT_TOKEN` du shell prioritaire, `./downloads` et `./logs` montés, root, `restart: unless-stopped` ; en plus : `./state` pour `/settings`, lecture seule, une seule capacité (`DAC_OVERRIDE`), `no-new-privileges`, limites mémoire, CPU et processus, rotation des journaux Docker |
| `entrypoint.sh`, `start.sh` | (aucun) | non repris | lancement Pelican/Pterodactyl (`/home/container`, `venv/bin/python`) : question ouverte ci-dessous |
| `setup.sh` | README : `cp .env.example .env` puis `cargo build --release` | différent | plus de `.venv` ni de `pip` ; le bot crée aussi `.env` lui-même s'il manque |
| `.env.example` | `.env.example` | amélioré | toutes les clés de l'original y sont (MESURÉ), avec la même valeur modèle du jeton (`YOUR_TELEGRAM_BOT_TOKEN_HERE`) |
| `.github/workflows/deploy.yml` | `.github/workflows/ci-rust.yml` | amélioré | mêmes branches (`main`, `develop`), mêmes étiquettes (`latest` et `VERSION` sur `main`, `dev` sur `develop`), amd64 + arm64 ; en plus : fmt, clippy, tests, `cargo audit` et scan Trivy avant publication, étiquette SHA |
| `.github/ISSUE_TEMPLATE/*.yml` | inchangés | identique | diff vide avec `a467a68` (MESURÉ) |
| `.dockerignore`, `.gitignore`, `AGENTS.md`, `DOCS.md`, `README.md` | réécrits | sans objet | décrivent le bot Rust |

## Ajouts (rien n'est retiré)

- Une tâche par message : un utilisateur lent ne bloque plus les autres.
- `/settings` : menu de boutons réservé aux administrateurs, tous les réglages branchés (détail dans le README).
- Cache des `file_id` : un lien déjà envoyé repart sans nouveau téléchargement.
- Limite de débit (`RATE_LIMIT_PER_MIN`) et garde anti-SSRF sur les liens.
- Bornes de taille et de durée (`MAX_FILESIZE_MB=500`, `MAX_DURATION_S=3600`), directs refusés.
- Nettoyage d'urgence qui épargne les téléchargements en cours.
- Messages modifiés traités comme des nouveaux messages.
- Journaux : fichier sans codes ANSI, jeton masqué, nouveau fichier à minuit.
- Sources directes optionnelles (X, Bluesky, TikTok), désactivées par défaut.
- Conteneur durci, `HEALTHCHECK`, CI avec tests et scans.
- Token refusé par Telegram : message clair (« vérifiez BOT_TOKEN ») et sortie en échec, au lieu d'une trace d'exception.

## Différences encore visibles

| Différence | Raison | Pour retrouver l'original |
|---|---|---|
| Repli externe au-delà de 50 Mo au lieu de 35 | 50 Mo est la vraie limite d'envoi de l'API Bot | `TELEGRAM_UPLOAD_LIMIT_MB=35` |
| Heure des journaux en UTC | le bot calcule la date lui-même, sans base de fuseaux horaires ; dans un conteneur sans `TZ`, le Python écrivait déjà en UTC (DÉDUIT) | aucun réglage |
| Pas de lignes `[RETENTION] Set future mtime…` ni `[FILE_MANAGER] Hash enregistré…` | ces deux mécanismes sont remplacés (échéance calculée, cache) | sans objet |
| Ligne d'envoi externe sans « via curl » | l'envoi est un `PUT` fait par le bot lui-même, sans `curl` | sans objet |
| La ligne de progression d'envoi nomme le fichier d'origine, pas son nom sha256 | le fichier n'est pas renommé sur le disque : seul le nom donné à l'hébergeur est le sha256 | sans objet |
| Premier passage du nettoyage périodique dès le démarrage (le Python attendait un intervalle) | sans effet visible : le dossier vient d'être vidé au démarrage, comme dans le Python | sans objet |
| Taille et durée bornées, directs refusés, mp4 préféré | protège le disque et la durée d'une tâche ; mp4 se lit directement dans Telegram | `MAX_FILESIZE_MB`, `MAX_DURATION_S`, `/settings` |
| `/music` télécharge l'audio seul | même mp3, moins de données | sans objet |
| `/stats` par ID numérique seulement | un pseudo Telegram peut changer de propriétaire | `ADMIN_USER_IDS=<ID>` |
| Nom du fichier envoyé en ASCII | noms sûrs pour Telegram et le disque | aucun réglage |
| Dossiers montés dans `/data` au lieu de `/app` (compose) | image en lecture seule, `WORKDIR /data` | mêmes dossiers côté hôte : `./downloads`, `./logs` |
| Sans jeton, le conteneur ne crée pas de modèle `.env` | système de fichiers en lecture seule : le bot le dit (« impossible de créer .env ») puis demande le jeton dans le `.env` ou l'environnement ; le Python créait ce `.env` dans le conteneur, perdu quand on le recrée (DÉDUIT) | sans objet : le `.env` se remplit sur l'hôte, à côté du compose |
| Pas de scripts Pelican/Pterodactyl | voir les décisions ouvertes | à décider |

## Décisions ouvertes pour Tom

1. **Seuil du repli** : 50 Mo (limite de l'API Bot) ou 35 Mo comme avant ?
2. **Accord Oui/Non avant l'envoi externe** : désactivé par défaut, comme l'original. L'activer par défaut ?
3. **Limite de débit et garde anti-SSRF** : à garder telles quelles ?
4. **`/stats`** : accès par ID seulement ; y mettre l'ID de l'administrateur avec `ADMIN_USER_IDS`.
5. **Bornes de taille et de durée** (500 Mo, 3600 s) : à garder, relever ou retirer ?
6. **Pelican/Pterodactyl** : faut-il un équivalent de `start.sh` et `entrypoint.sh` (lancement depuis `/home/container`) ?
7. **CI** : approuver les runs `ci-rust` de la PR (« Approve and run workflows »).

## Vérification (06/11/2026)

- `cargo test --locked` : 143 tests verts (108 unitaires + 35 d'intégration), `cargo clippy --all-targets -- -D warnings` propre (MESURÉ).
- Point 16 : le test `telemetry::tests::couleurs_en_console_jamais_dans_le_fichier` rougit si le fichier prend les couleurs ou si la console les perd (2 mutations, toutes deux détectées, MESURÉ).
- Compose : root avec `cap_drop: ALL` est refusé sur un dossier `1000:1000`, accepté avec `DAC_OVERRIDE` (essai Docker, MESURÉ).
- Conteneur réel (Docker Desktop, amd64, image reconstruite sur cette version, MESURÉ) : sans jeton, message clair et code 1 ; jeton refusé par Telegram : « [INIT] Telegram refuse le token… » et code 1 au lieu de la panique de teloxide (code 101), relancé par `restart: unless-stopped` comme le compose d'origine ; dossiers `1000:1000` conservés ; journal du jour écrit, sans code de couleur ; `yt-dlp --update-to stable` dans l'image : « yt-dlp is up to date (stable@2026.10.26…) », la ligne que le bot écrit après `[YTDLP]`.
- Token refusé : le test `o_token_refuse_arret_propre` rougit avec la mutation M11 (`get_me` préalable retiré : panique d'origine), MESURÉ.
- Non vérifié : un vrai téléchargement avec un vrai jeton, Podman, arm64 ; la CI GitHub attend son approbation.
