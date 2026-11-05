# Mesures APRÈS : bot Rust (commit 6f42b8a, branche refonte-rust)

Date 2026-11-05. Valeurs brutes : `apres.json`. Même banc et même protocole que `avant.md` (`bench/`, voir `bench/README.md`).
`$SCR` = dossier de travail hors git : `svd-bench.exe` (binaire release, voir « Binaire patché »), `bench-10s.mp4` (2,34 Mo), `svd-ancien` et `venv-ancien` pour la re-mesure de l'avant.
Machine : Windows 11, Docker Desktop 29.6.1 (7,7 Go), cargo/rustc 1.99.0, yt-dlp 2026.08.19, ffmpeg 8.1.2 (mêmes yt-dlp/ffmpeg que pour l'avant, hors Docker).

| Métrique | Valeur | Unité | Commande | Étiquette |
|---|---|---|---|---|
| Lignes de code Rust (src/) | 3 410 | lignes | `git ls-files 'src/*.rs' \| xargs wc -l \| tail -1` | MESURÉ |
| Lignes de tests Rust (tests/) | 616 | lignes | `git ls-files 'tests/*.rs' 'tests/**/*.rs' \| xargs wc -l \| tail -1` | MESURÉ |
| Dépendances directes | 11 | crates | lignes de la section `[dependencies]` de `Cargo.toml` : `sed -n '/\[dependencies\]/,/^\[dev/p' Cargo.toml \| rg -c '^[a-z]'` | MESURÉ |
| Dépendances totales | 205 | crates | `cargo tree --prefix none -e normal \| sort -u \| wc -l` (inclut le crate racine) | MESURÉ |
| Taille d'image Docker | 182 466 181 (174,0 Mio) | octets | `docker build -t svd-rust:bench . && docker image inspect --format '{{.Size}}' svd-rust:bench` | MESURÉ |
| Mémoire au repos | 1,379 (CPU 0,06 %) | Mio | `docker run -d --name wfrvo9ht-svdrust -e BOT_TOKEN=123:ABC -e TELEGRAM_API_URL=http://host.docker.internal:18091 --add-host host.docker.internal:host-gateway svd-rust:bench`, 30 s, `docker stats --no-stream wfrvo9ht-svdrust` (faux Telegram : `python bench/fake_telegram.py --port 18091 --host 0.0.0.0`) | MESURÉ |
| Temps de démarrage | 0,023 (runs : 0,023 / 0,081 / 0,023) | s | `python bench/run_bench.py ...` (champ `startup_s`) | MESURÉ |
| Latence p50, 10 téléchargements simultanés | 3,638 (3,638 / 5,415 / 3,079) | s | `run_bench.py`, commande ci-dessous | MESURÉ |
| Latence p95, 10 téléchargements simultanés | 4,998 (3,988 / 5,778 / 4,998) | s | idem | MESURÉ |
| Débit total, 10 téléchargements simultanés | 4,687 Mo/s ; 2,001 fichiers/s (fichier de 2,34 Mo) | Mo/s | idem (`throughput_mb_per_s`, `throughput_files_per_s`) | MESURÉ |

Commande de banc (3 fois, médiane) :

```
export ALLOWED_USER_IDS=1000,1001,1002,1003,1004,1005,1006,1007,1008,1009 MAX_CONCURRENT_DOWNLOADS=10 \
  DOWNLOAD_DIR=$SCR/dl HEARTBEAT_FILE=$SCR/hb
python bench/run_bench.py --media-file $SCR/bench-10s.mp4 --bot-cmd "$SCR/svd-bench.exe" --bot-cwd . --out apres-N.json
```

## Re-mesure de l'avant (même session, faux Telegram modifié)

Python 3.13 hors Docker, venv de banc (`PYTHONPATH=$SCR/venv-ancien/Lib/site-packages python main.py`, l'interpréteur du venv avait disparu), 3 runs :
démarrage 1,143 s (1,851 / 1,143 / 1,102), p50 2,284 s, p95 4,233 s, débit 5,533 Mo/s et 2,362 fichiers/s. Les valeurs d'origine (`avant.md` : 2,574 / 4,626 / 5,063) sont à 13 % au plus : le faux Telegram modifié ne change pas la mesure de l'ancien bot. `avant.json` n'est pas modifié ; la re-mesure est dans `apres.json` (`avant_remesure`).

## Binaire patché et faux Telegram

- Le banc d'origine ne convenait pas à teloxide : `getWebhookInfo` absent, `getMe` incomplet, réponses `send*` sans champs `video`/`audio` (mime_type obligatoire), corps `multipart` envoyé en transfert fractionné (chunked) non lu, et le bot Rust ne cite pas le message d'origine (pas de `reply_to_message_id`). `bench/fake_telegram.py` a été corrigé (lecture chunked, champs manquants, appariement par `chat_id` quand `reply_to` manque : un message par utilisateur, chat privé = id utilisateur).
- Sans durée connue, le filtre de yt-dlp du bot rejette tout média (`src/download.rs:147`, bug SOCIAL-2611050403-f13). Pour la mesure seulement, un binaire `svd-bench.exe` compilé avec un caractère de plus dans ce filtre (`duration? <=`, qui laisse passer une durée inconnue), hors dépôt ; `src/` n'est pas modifié. Mesures sur ce binaire : elles ne valent que pour le chemin de téléchargement, pas pour la correction du bug.

## Notes de lecture

- Latence = du moment où le faux Telegram sert le message au bot jusqu'à la fin de réception du fichier ; médiane de 3 runs.
- p95 sur 10 messages = le plus lent (nombre d'échantillons faible).
- 205 = lignes de `cargo tree` dédupliquées, crate racine compris.
- Mémoire et taille d'image : conteneur. Démarrage et latence : bot hors Docker (Windows).
- Aucune valeur DÉDUITE : toutes les métriques ont été mesurées.
