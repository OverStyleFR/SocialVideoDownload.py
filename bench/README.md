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
python bench/run_bench.py --media-file $SCR/bench-10s.mp4 \
  --bot-cmd "./target/release/socialvideodownload" --bot-cwd . --out apres-run.json
```
`run_bench.py` fixe `TELEGRAM_API_URL=http://127.0.0.1:<port>` et `UNSAFE_ALLOW_PRIVATE_HOSTS=1` pour le bot ; équivalent à la main :
`TELEGRAM_API_URL=http://127.0.0.1:18081 UNSAFE_ALLOW_PRIVATE_HOSTS=1 BOT_TOKEN=123:ABC ./target/release/socialvideodownload`.
Résultats à consigner dans `docs/refonte-rust/mesures/apres.json` / `apres.md` (même tableau que `avant.md`).
