# Mesures AVANT : ancien bot Python (commit a467a68)

Date 2026-11-05. Valeurs brutes : `avant.json`. Banc : `bench/` (voir `bench/README.md`).
`$SCR` = dossier de travail hors git : `svd-ancien` (worktree a467a68 + patch `base_url` d'une ligne dans `main.py`), `venv-ancien`, `bench-10s.mp4`.
Machine : Windows 11, Docker Desktop 29.6.1 (7,7 Go), Python 3.13 pour le bot hors Docker (le conteneur utilise 3.11).

| Métrique | Valeur | Unité | Commande | Étiquette |
|---|---|---|---|---|
| Lignes de code Python | 1113 (21 fichiers) | lignes | `cd $SCR/svd-ancien && git ls-files '*.py' \| grep -v '^ffmpeg/' \| xargs wc -l \| tail -1` | MESURÉ |
| Dépendances directes | 7 | paquets | `wc -l < requirements.txt` | MESURÉ |
| Dépendances totales | 18 (16 hors pip et setuptools) | paquets | `python -m venv venv-ancien && venv-ancien/Scripts/python -m pip install -r requirements.txt && venv-ancien/Scripts/python -m pip install "setuptools<81" && venv-ancien/Scripts/python -m pip list --format=freeze \| wc -l` | MESURÉ |
| Dépendances totales dans l'image | 19 | paquets | `docker run --rm --entrypoint sh svd-ancien:bench -c 'pip list --format=freeze \| wc -l'` | MESURÉ |
| Taille d'image Docker | 116 510 831 (111,1 Mio) | octets | `docker build -t svd-ancien:bench $SCR/svd-ancien && docker image inspect --format '{{.Size}}' svd-ancien:bench` | MESURÉ |
| Mémoire au repos (docker stats, sous-évaluée ; voir STATS.md, RSS = 48,11) | 37,98 (CPU 0,09 %) | Mio | `docker run -d --name wfrvo9ht-svd -e BOT_TOKEN=123:ABC -e TELEGRAM_API_URL=http://host.docker.internal:18091 --add-host host.docker.internal:host-gateway svd-ancien:bench`, 30 s, `docker stats --no-stream wfrvo9ht-svd` (faux Telegram : `python bench/fake_telegram.py --port 18091 --host 0.0.0.0`) | MESURÉ |
| Temps de démarrage | 1,05 (runs : 1,05 / 1,07 / 0,84) | s | `python bench/run_bench.py ...` (champ `startup_s` : lancement du processus → premier getUpdates reçu) | MESURÉ |
| Latence p50 sous 10 téléchargements simultanés | 2,574 (2,94 / 2,57 / 2,51) | s | `python bench/run_bench.py` (commande complète dans `bench/README.md`) | MESURÉ |
| Latence p95 sous 10 téléchargements simultanés | 4,626 (5,29 / 4,53 / 4,63) | s | idem | MESURÉ |
| Débit total sous 10 téléchargements simultanés | 5,06 Mo/s ; 2,16 fichiers/s (fichier de 2,34 Mo) | Mo/s | idem (champs `throughput_mb_per_s`, `throughput_files_per_s`) | MESURÉ |

Notes de lecture :
- Latence = du moment où le faux Telegram sert le message au bot jusqu'à la fin de réception du fichier (multipart) ; médiane de 3 runs, valeurs brutes entre parenthèses dans `avant.json` (`runs_bruts`).
- p95 sur 10 messages = le plus lent (nombre d'échantillons faible).
- Le test de mémoire et la taille d'image portent sur le conteneur ; démarrage et latence sur le bot lancé hors Docker (Python 3.13, Windows) : ils ne sont pas strictement comparables à une exécution Linux/3.11.
- `setuptools<81` est ajouté au venv de banc seulement : PTB 13.7 (APScheduler 3.6.3) importe `pkg_resources`, absent de setuptools 84. Le Dockerfile exclut setuptools du wheelhouse et s'appuie sur celui de l'image 3.11.
- Les téléchargements passent par yt-dlp (extracteur « generic ») sur un mp4 local ; l'upload gros fichiers (curl.libriciel.fr) n'est pas exercé (fichier < 35 Mo).
- Aucune valeur DÉDUITE : les six métriques ont été mesurées.
