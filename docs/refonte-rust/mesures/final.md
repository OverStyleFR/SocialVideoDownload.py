# Mesures finales : Python contre Rust, même session (06/11/2026)

Valeurs brutes : `final.json`. Même banc que `avant.md` et `apres.md` (`bench/`, voir `bench/README.md`) : faux Telegram, `media_server.py`, 10 utilisateurs qui envoient un lien en même temps, `bench-10s.mp4` (2,34 Mo), 3 runs par bot, médiane. Les deux bots ont été mesurés l'un après l'autre sur la même machine (Windows 11, hors Docker, même yt-dlp et même ffmpeg), pour que l'écart ne vienne pas du poste.

| Métrique | Python (`a467a68`) | Rust (cette version) | Écart | Étiquette |
|---|---|---|---|---|
| Démarrage | 1,048 s (1,490 / 1,048 / 1,032) | 0,022 s (0,194 / 0,022 / 0,017) | −97,9 % | MESURÉ |
| Latence p50 | 2,405 s (2,236 / 3,833 / 2,405) | 2,979 s (3,023 / 2,818 / 2,979) | +23,9 % | MESURÉ |
| Latence p95 = dernier des 10 servis | 4,277 s (4,213 / 6,006 / 4,277) | 3,060 s (3,188 / 2,871 / 3,060) | −28,5 % | MESURÉ |
| Débit | 5,476 Mo/s (5,559 / 3,899 / 5,476) | 7,655 Mo/s (7,349 / 8,160 / 7,655) | +39,8 % | MESURÉ |
| Image Docker (compressée, `.Size` sous le moteur containerd) | 116 510 831 o (111,1 Mio) | 186 214 112 o (177,6 Mio) ; Pelican 185 035 271 o ; décompressée : 669 Mo (610 Mo avec yt-dlp « un seul fichier ») | +59,8 % | MESURÉ |
| Mémoire au repos (RSS) | 48,11 Mio | 7,0 Mio (mesurée sur `042c932`, code du service inchangé depuis) | −85,4 % | MESURÉ |

Écart = (Rust − Python) / Python, DÉDUIT des deux colonnes.

## Commandes

```
# Rust
export ALLOWED_USER_IDS=1000,1001,1002,1003,1004,1005,1006,1007,1008,1009 MAX_CONCURRENT_DOWNLOADS=10 RATE_LIMIT_BURST=1000
python bench/run_bench.py --media-file $SCR/bench-10s.mp4 --bot-cmd "<cible cargo>/release/socialvideodownload.exe" --bot-cwd . --out apres-N.json
# Python : copie de a467a68 (git archive) + patch base_url de bench/README.md
cd $SCR/svd-ancien && PYTHONPATH=$SCR/venv-ancien/Lib/site-packages python <repo>/bench/run_bench.py \
  --media-file $SCR/bench-10s.mp4 --bot-cmd "python main.py" --bot-cwd $SCR/svd-ancien --out avant-N.json
# yt-dlp dans l'image standard (3 lancements) : 394 / 353 / 322 ms
docker run --rm --entrypoint sh <image> -c 'time yt-dlp --version'
```

## Lecture

- **Pour 10 demandes simultanées, le Rust les a toutes servies en 3,06 s, contre 4,28 s pour le Python** (p95 = maximum sur 10 échantillons). Le p50 du Python est plus bas : les premiers arrivés passent vite, puis la file s'allonge. Le Rust traite les 10 en parallèle et ils finissent presque ensemble (au run médian, de 2,566 à 3,060 s pour le Rust, de 0,802 à 4,277 s pour le Python, MESURÉ dans `latencies_s`).
- Le run 2 du Python (p50 3,833 s, débit 3,899 Mo/s) est loin des deux autres ; la médiane l'écarte. La dispersion du Rust sur ses 3 runs est plus faible (p50 de 2,818 à 3,023 s).
- **Gain de la passe de latence du 06/11** (même banc, Rust seul) : par rapport à `042c932` (p50 3,589 s, p95 3,664 s, 6,393 Mo/s), −17,0 % sur le p50, −16,5 % sur le p95 et +19,7 % sur le débit (MESURÉ). Cause probable (DÉDUIT, pas isolée par une mesure dédiée) : le contrôle audio (ffprobe et volumedetect) tourne maintenant en même temps que l'envoi au lieu de le précéder.
- Dans l'image standard, yt-dlp est installé en version « dossier » : il ne se décompresse plus à chaque lancement (316 à 408 ms contre 776 à 1 222 ms pour la version « un seul fichier », MESURÉ). Ce gain ne se voit pas sur le banc ci-dessus, qui tourne hors Docker. L'image Pelican garde la version « un seul fichier », qui sait se mettre à jour seule.
- yt-dlp et ffmpeg restent des programmes externes dans les deux versions : la latence d'un vrai téléchargement dépend d'abord du site source et du réseau, que ce banc ne mesure pas.
