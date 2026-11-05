# Refonte Rust de SocialVideoDownload : fiche

Le bot Telegram Python (PTB 13.7, synchrone) est réécrit en Rust (teloxide, tokio). Les 18 défauts de l'audit ont chacun une preuve rouge sur l'ancien code (`preuves-ancien/`, 17 tests pytest en échec + constats statiques) et, sauf exception justifiée, un test vert sur le nouveau (journal : `INGEST.md`).

## Pourquoi c'est mieux

Un utilisateur ne bloque plus les autres, l'accès est par liste blanche d'ID avec limite de débit et garde anti-SSRF, le nettoyage épargne les tâches en cours, rien ne part chez un tiers sans consentement, et chaque erreur répond à l'utilisateur. L'image tourne en non-root, épinglée par digest, avec healthcheck.

## Avant / après

| Axe | Avant (Python, a467a68) | Après (Rust, refonte-rust) |
|---|---|---|
| Concurrence | un téléchargement bloque tous les messages | une tâche par message (e2e `b_slow_user_does_not_block_other`) |
| Sécurité | pas de contrôle d'accès, SSRF possible, pseudo comme identité, ID en dur | liste blanche d'ID, débit limité, garde SSRF, /stats par ID numérique |
| Fiabilité | nettoyage qui supprime les tâches en cours, mp3 partiel renvoyé, 5xx renvoyé comme URL | dossier par tâche verrouillé, mp3 atomique, statut vérifié, consentement |
| Exploitation | root, sans healthcheck, ffmpeg `:latest`, yt-dlp non mis à jour | UID 10001, healthcheck, digests épinglés ; mise à jour de yt-dlp écrite mais non branchée |
| Tests | aucun | 48 unitaires + 12 e2e avec faux Telegram, 8 mutations (7 rouges, 1 renforcée) |
| CI | `python -c "import main"`, actions par tag | fmt, clippy, test, audit, Trivy, actions par SHA (non encore exécutée sur runner) |
| Image Docker | 111,1 Mio (MESURÉ) | 174,0 Mio, +56,6 % (MESURÉ ; écart DÉDUIT) |
| Mémoire au repos | 37,98 Mio (MESURÉ) | 1,379 Mio, -96,4 % (MESURÉ ; écart DÉDUIT ; au repos seulement) |
| Démarrage | 1,05 s (MESURÉ) | 0,023 s, -97,8 % (MESURÉ ; écart DÉDUIT) |
| Latence p50 / p95 (10 simultanés) | 2,574 s / 4,626 s (MESURÉ) | 3,638 s / 4,998 s, +41,3 % / +8,0 % (MESURÉ ; écarts DÉDUITS) |
| Débit | 5,063 Mo/s (MESURÉ) | 4,687 Mo/s, -7,4 % (MESURÉ ; écart DÉDUIT) |
| Lignes de code | 1 113 (MESURÉ) | 3 410 dont tests en ligne, +206,4 % (MESURÉ) ; tests : 616 lignes |
| Dépendances totales | 18 (MESURÉ) | 205, +1 038,9 % (MESURÉ) |

Chiffres repris de `STATS.md` (conditions et limites y sont). En clair : Rust n'est pas plus rapide sur ce banc, car yt-dlp et ffmpeg restent des binaires externes ; le gain est sur la mémoire, le démarrage, la sécurité et la fiabilité.

## Risques restants

- `spawn_updater` et `spawn_janitor` ne sont pas branchés (bug SOCIAL-2611050351-k8h) : pas de mise à jour automatique de yt-dlp ni de nettoyage périodique.
- Cache en mémoire seule ; estimation de l'espace disque ; DNS rebinding non couvert (marqueurs `shortcut:`).
- CI jamais exécutée sur un vrai runner ; chemin Linux des faux binaires non exécuté ; l'ancien workflow de déploiement est encore suivi par git.
- Mesures de latence faites avec un binaire patché (bug SOCIAL-2611050403-f13) ; le gain mémoire est mesuré au repos, pas sous charge.
- L'historique git garde les anciennes valeurs sensibles (non réécrit) ; l'upload externe de plus de 35 Mo n'est pas exercé par le banc.

## Relire la branche

```
git log --oneline a467a68..refonte-rust
git diff --stat a467a68..refonte-rust
cargo test                      # 48 unitaires + 12 e2e
git grep -n 'shortcut:'
```
Les 17 tests rouges se rejouent avec pytest sur un checkout de `a467a68` (voir `preuves-ancien/README.md`). Lire ensuite `docs/refonte-rust/INGEST.md` (point par point) et `docs/refonte-rust/STATS.md` (mesures et graphes).
