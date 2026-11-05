# Constats statiques sur a467a68 (points 6, 7, 12, 13, 14, 17, 18)

Chaque commande se rejoue depuis la racine du dépôt (`git ... a467a68`, aucun checkout requis) ou depuis la copie `svd-ancien`.
Sorties relevées le 05/11/2026. Les valeurs sensibles sont masquées (`<IP-INTERNE>`, `123456789`).

## Point 6 : PTB 13.7 impose urllib3<2
`git show a467a68:requirements.txt | grep -n -E "telegram|urllib3"`
```
1:python-telegram-bot==13.7
2:urllib3>=1.26,<2
```
Constat : PTB figé sur 13.7 (2021), urllib3 plafonné en 1.x. Installation réelle observée : `urllib3 1.26.20`, `setuptools<81` nécessaire (PTB importe `pkg_resources` via APScheduler 3.6.3).

## Point 7 : yt-dlp installé au build seulement
`git grep -n -i -E "yt-dlp|pip " a467a68 -- Dockerfile Dockerfile.compose entrypoint.sh start.sh`
```
Dockerfile:14:RUN pip wheel --no-cache-dir --wheel-dir /app/wheels -r requirements.txt
Dockerfile.compose:6:RUN pip wheel --no-cache-dir --wheel-dir /wheels -r requirements.txt
```
Constat : aucune ligne de `entrypoint.sh` / `start.sh` ne met yt-dlp à jour ; la version est celle du build (`yt-dlp>=2026.8.19` dans requirements.txt).

## Point 12 : conteneur root, pas de healthcheck, pas de limites, image ffmpeg `:latest`
`git grep -n -i -E "^FROM|^USER|HEALTHCHECK|mem_limit|cpus|limits" a467a68 -- Dockerfile Dockerfile.compose docker-compose.yml`
```
Dockerfile:2:FROM ghcr.io/linuxserver/ffmpeg:latest AS ffmpeg
Dockerfile:5:FROM python:3.11-slim-bookworm AS builder
Dockerfile:18:FROM python:3.11-slim-bookworm
Dockerfile.compose:1:FROM python:3.11-slim-bookworm AS builder
Dockerfile.compose:8:FROM python:3.11-slim-bookworm
```
Constat : aucune directive `USER` (donc root), `HEALTHCHECK`, `mem_limit`, `cpus` ni `limits` ; `ffmpeg:latest` non figée.

## Point 13 : CI minimale
`git grep -n -E "uses:|run:|Run tests|pip-audit|trivy|ruff|pytest" a467a68 -- '*.yml'` (fichier : `deploy.yml` du dossier des workflows)
```
deploy.yml:14:      - uses: actions/checkout@v6
deploy.yml:15:      - uses: actions/setup-python@v6
deploy.yml:19:        run: pip install -r requirements.txt
deploy.yml:21:        run: python -c "import main"
deploy.yml:31:        uses: actions/checkout@v6
deploy.yml:34:        uses: docker/setup-qemu-action@v4
deploy.yml:37:        uses: docker/setup-buildx-action@v4
deploy.yml:40:        uses: docker/login-action@v4
deploy.yml:65:        uses: docker/build-push-action@v7
```
Constat : seule vérification = `python -c "import main"` (ligne 21) ; actions référencées par tag (`@v6`, `@v4`, `@v7`), jamais par SHA ; aucune occurrence de test, lint, pip-audit ou Trivy. L'étape de publication d'image (lignes 30-73) suit les branches listées ligne 6 (`branches:`).

## Point 14 : poids du dépôt et `.pyc` suivis
`git ls-tree -r --name-only a467a68 | wc -l` -> `286`
`git ls-tree -r --name-only a467a68 | grep -c '^ffmpeg/'` -> `236`
`git ls-tree -r --name-only a467a68 | grep -c '\.pyc$'` -> `13`
`git show a467a68:.gitignore | grep -n pyc` -> `11:.pyc` (motif littéral `.pyc` au lieu de `*.pyc` : n'ignore aucun fichier `*.pyc`)
`du -sh svd-ancien/ffmpeg` -> `162M`

## Point 17 : docs en désaccord avec le code
- `git show a467a68:AGENTS.md | grep -n "upload.py"` -> ligne 39 `│   └── upload.py   # Upload helper (now in utils/)` placée sous `commands/` ; `git ls-tree --name-only a467a68 commands/` ne liste ni `upload.py` (liste : auto_download, download, help, music, start, stats).
- `git show a467a68:AGENTS.md | sed -n 91p` -> `2. load_cache() — loads cache tracking from disk.` ; `utils/cache.py:25-28` : `load_cache()` fait `download_cache.clear()` et logge « session en mémoire ».
- `git show a467a68:AGENTS.md | sed -n 154p` -> « The workflow's "Run tests" is a placeholder » ; le workflow n'a aucune étape de ce nom (étapes : Install dependencies, Verify imports, ...).
- Seuils : `git grep -n "SMALL_FILE_THRESHOLD =" a467a68 -- utils/cache.py` -> `5 * 1024 * 1024` (5 Mo) ; `git grep -n "SMALL_FILE_SIZE_MB\"," a467a68 -- config.py` -> défaut `12` (12 Mo). Deux seuils distincts pour « petit fichier ».
- Défaut de nettoyage 48 h vs 24 h : `config.py` donne `CLEANUP_INTERVAL_HOURS` 24 et `RETENTION_SMALL_HOURS` 24 ; la mention « 48 h » de l'audit n'a PAS été retrouvée dans AGENTS.md / DOCS.md / README.md par `git grep -n -E "48 ?h" a467a68 -- '*.md'` (sortie vide) : non confirmé.

## Point 18 : secrets/infra publiés
`git grep -n -E "ssh root@|[0-9]{9,10}" a467a68 -- '*.md' '*.py' ':!ffmpeg'`
```
a467a68:AGENTS.md:190:   ssh root@<IP-INTERNE>
a467a68:commands/stats.py:14:AUTHORIZED_IDS = {123456789}   (ID Telegram réel masqué ici)
```
Constat : une commande SSH root vers une adresse privée est publiée dans AGENTS.md, un ID Telegram est en dur dans `commands/stats.py`.
