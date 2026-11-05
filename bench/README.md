# Banc de mesure commun (ancien bot Python et nouveau bot Rust)

Stdlib Python seulement. Jeton factice `123:ABC`, uniquement `127.0.0.1` / `host.docker.internal`.

- `fake_telegram.py` : faux Bot API (`getMe`, `getUpdates`, `sendMessage`, `editMessageText`, `sendVideo/Document/Audio` multipart horodatés).
- `media_server.py` : sert un mp4 sur `http://127.0.0.1:<port>/v/<n>.mp4`.
- `run_bench.py` : lance les deux, lance le bot (`--bot-cmd`), injecte 10 messages de 10 utilisateurs, écrit un JSON (`startup_s`, `latency_p50_s`, `latency_p95_s`, `throughput_*`, `latencies_s`, `method_counts`). Code de sortie 1 si un fichier manque.

Le bot reçoit : `BOT_TOKEN=123:ABC`, `TELEGRAM_API_URL=http://127.0.0.1:<port>` (racine, sans `/bot`), `UNSAFE_ALLOW_PRIVATE_HOSTS=1`.

## Préparer (une fois, hors git)

```
SCR=<dossier de travail hors git>
ffmpeg -y -f lavfi -i testsrc2=size=640x360:rate=25:duration=10 -f lavfi -i sine=frequency=440:duration=10 \
  -c:v libx264 -b:v 1.8M -maxrate 1.8M -bufsize 3M -c:a aac -movflags +faststart $SCR/bench-10s.mp4   # ~2,3 Mo (ou : media_server.py --make)
git worktree add --detach $SCR/svd-ancien a467a68
```
Patch d'une ligne, **dans la copie seulement** (`$SCR/svd-ancien/main.py`, ajouter `import os`) :
`Updater(token, use_context=True, base_url=(os.getenv('TELEGRAM_API_URL', 'https://api.telegram.org').rstrip('/') + '/bot'))`

```
python -m venv $SCR/venv-ancien
$SCR/venv-ancien/Scripts/python -m pip install -r $SCR/svd-ancien/requirements.txt "setuptools<81"
```

## Rejouer AVANT (ancien bot a467a68)

```
cd $SCR/svd-ancien && $SCR/venv-ancien/Scripts/python <repo>/bench/run_bench.py \
  --media-file $SCR/bench-10s.mp4 --bot-cmd "$SCR/venv-ancien/Scripts/python main.py" \
  --bot-cwd $SCR/svd-ancien --out avant-run.json
```
Sortie : une ligne JSON résumée + `avant-run.json` ; comparer à `docs/refonte-rust/mesures/avant.json` (médiane de 3 runs).

Mémoire au repos en conteneur : `python bench/fake_telegram.py --port 18091 --host 0.0.0.0` puis
`docker run -d --name wfrvo9ht-svd -e BOT_TOKEN=123:ABC -e TELEGRAM_API_URL=http://host.docker.internal:18091 --add-host host.docker.internal:host-gateway <image>`, attendre 30 s, `docker stats --no-stream wfrvo9ht-svd`, puis `docker rm -f wfrvo9ht-svd`.

## Rejouer APRÈS (bot Rust, même protocole)

```
cargo build --release
export ALLOWED_USER_IDS=1000,1001,1002,1003,1004,1005,1006,1007,1008,1009 MAX_CONCURRENT_DOWNLOADS=10 \n  DOWNLOAD_DIR=$SCR/dl HEARTBEAT_FILE=$SCR/hb
python bench/run_bench.py --media-file $SCR/bench-10s.mp4 \n  --bot-cmd "<cible cargo>/release/socialvideodownload" --bot-cwd . --out apres-N.json   # 3 fois, médiane
```
`run_bench.py` fixe `BOT_TOKEN`, `TELEGRAM_API_URL=http://127.0.0.1:<port>` et `UNSAFE_ALLOW_PRIVATE_HOSTS=1` ; les autres variables viennent de l'environnement. Les ids autorisés doivent être 1000 à 1009 (ids injectés par le faux Telegram). Chemin du binaire : la cible cargo peut être hors du dépôt (`cargo metadata --format-version 1 | jq -r .target_directory`). Utiliser des chemins `C:/...` (pas de `\`) dans `--bot-cmd` : il est découpé par `shlex`.
Piège : sans durée connue le filtre de yt-dlp du bot rejette tout média (bug SOCIAL-2611050403-f13, `src/download.rs:147`) ; les mesures ont été prises avec un binaire de banc où ce filtre vaut `duration? <=` (hors dépôt, à reconstruire après le correctif).
Le faux Telegram gère le corps `chunked` de teloxide, `getWebhookInfo`, les réponses `send*` avec média, et rattache un envoi à son message par `chat_id` quand le bot ne cite pas le message.

Résultats : `docs/refonte-rust/mesures/apres.{json,md}` ; graphes et tableau : `python bench/graphes.py [--table]` puis `docs/refonte-rust/STATS.md`.
