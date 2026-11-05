# Refonte Rust du bot Telegram : audit en 18 points, /settings et sources hybrides

## Résumé

- Remplace l'ancien bot Python (base `a467a68`) par un bot Rust (teloxide 0.17, tokio) ; `yt-dlp` et `ffmpeg` restent des programmes externes.
- Corrige les 18 points d'un audit de l'ancien bot : chaque point a un constat, une correction et un test (ou une justification écrite).
- Ajoute `/settings` (réglages à chaud par boutons, réservé aux administrateurs) et une couche hybride de sources (X, Bluesky, TikTok), désactivées par défaut.
- Durcit le conteneur (non-root, système de fichiers en lecture seule) et ajoute un workflow CI `ci-rust.yml`.
- Honnêtement : le Rust n'est pas plus rapide sur ce banc ; l'apport est la sûreté et la testabilité. La CI GitHub n'a jamais tourné (voir « Limites »).

## Motivation

L'audit de l'ancien bot a trouvé 18 défauts : un téléchargement qui bloque tous les utilisateurs, aucune limite d'accès ni garde anti-SSRF, un nettoyage qui supprime les téléchargements en cours, un envoi chez un tiers sans avertissement, un conteneur root, une documentation périmée, une adresse d'infrastructure et un ID Telegram publiés. Dix-sept tests Python rejoués sur `a467a68` échouent tous (17 échoués, 0 passé, MESURÉ), ce qui fixe la ligne de départ. La refonte corrige chaque point par un test qui rejoue le vrai parcours.

## Avant / après

Source : `docs/refonte-rust/STATS.md`. MESURÉ = sorti d'une commande rejouable ; DÉDUIT = calculé à partir de mesures. Latence, débit et démarrage : médiane de 3 runs. Les écarts sont DÉDUITS des deux colonnes.

| Métrique | Avant (a467a68) | Après (6f42b8a) | Écart | Étiquette |
|---|---|---|---|---|
| Lignes de code applicatif | 1 113 (Python, 21 fichiers) | 3 410 (Rust, `src/`) | +206,4 % | MESURÉ |
| Dépendances directes | 7 | 11 | +57,1 % | MESURÉ |
| Dépendances totales | 18 (venv) | 205 (arbre cargo) | +1 038,9 % | MESURÉ |
| Taille d'image Docker | 116 510 831 o (111,1 Mio) | 182 466 181 o (174,0 Mio) | +56,6 % | MESURÉ |
| Mémoire au repos (30 s) | 37,98 Mio | 1,379 Mio | −96,4 % | MESURÉ |
| Temps de démarrage | 1,05 s | 0,023 s | −97,8 % | MESURÉ |
| Latence p50, 10 simultanés | 2,574 s | 3,638 s | +41,3 % | MESURÉ |
| Latence p95, 10 simultanés | 4,626 s | 4,998 s | +8,0 % | MESURÉ |
| Débit, 10 simultanés | 5,063 Mo/s | 4,687 Mo/s | −7,4 % | MESURÉ |

Code actuel (re-mesure du 2026-11-05, commit `f8f27ce`, STATS.md, MESURÉ) : 6 007 lignes Rust dans `src/`, 1 362 lignes de tests, 114 tests (83 unitaires + 31 d'intégration), 11 dépendances directes, 205 au total, image Docker de 182 623 255 o (174,2 Mio). La taille du binaire release n'a pas été relevée (DÉDUIT : non mesurée). Latence, débit, mémoire et démarrage n'ont pas été refaits sur le code actuel : le tableau ci-dessus reste celui du `6f42b8a`.

Lecture honnête : les écarts de latence et de débit sont du même ordre que la dispersion entre runs (Rust : p50 de 3,079 à 5,415 s). Ils ne prouvent pas que le Rust est plus lent, ils prouvent qu'il n'est pas plus rapide sur ce banc. Le p95 est calculé sur 10 échantillons : c'est le maximum.

## Les 18 points d'audit : correction et test

Source : `docs/refonte-rust/INGEST.md` (rejoué le 05/11/2026 sur `f8f27ce`). ROUGE = test de l'ancien code (`docs/refonte-rust/preuves-ancien/`, sur `a467a68`) ou mutation ; VERT = test Rust de la branche.

| Point | Défaut | Correction | Test (rouge avant / vert après) |
|---|---|---|---|
| 1 | Un téléchargement bloque tous les utilisateurs | Handlers teloxide, une tâche par message | ROUGE `test_point_01` / VERT e2e `b_slow_user_does_not_block_other` |
| 2 | Aucun contrôle d'accès, SSRF | `url_guard.rs`, `access.rs`, arguments yt-dlp bornés | ROUGE `test_point_02` / VERT `url_guard::tests::refuses_internal_targets`, `access::tests::user_outside_list_refused`, e2e `d_ssrf_refused` ; mutation `<=?` en `<=` rouge |
| 3 | Le nettoyage supprime les téléchargements en cours | Dossier par tâche verrouillé (`JobDir`), épargné par `retention.rs` | ROUGE `test_point_03` / VERT `retention::tests::retention_jobdir_vivant_survit`, e2e `j_cleanup_spares_running_job` |
| 4 | `curl_uploader` renvoie un 4xx/5xx comme URL, sans timeout | `egress.rs` : statut vérifié, HTML refusé, timeouts | ROUGE `test_point_04` / VERT `egress::tests::egress_500_refuse`, e2e `f_external_500_gives_error_without_link` |
| 5 | Envoi chez un tiers sans avertissement | Consentement Oui/Non nommant l'hôte | ROUGE `test_point_05` / VERT `egress::tests::consentement_nomme_hote`, e2e `f_oversize_needs_consent_then_uploads` |
| 6 | PTB 13.7 en fin de vie | teloxide 0.17, reqwest rustls, `cargo audit` en CI | Justification écrite, pas de test |
| 7 | yt-dlp installé au build seulement | Version épinglée par sha256 + mise à jour au démarrage | VERT `startup::b_updater_calls_ytdlp_only_when_enabled` ; mutation rouge si le lancement est retiré |
| 8 | `/music` télécharge la vidéo entière, mp3 partiel réutilisé | Audio seul, conversion mp3 atomique | ROUGE `test_point_08` / VERT `download::tests::download_audio_sans_video_entiere`, e2e `h_music_sends_audio_and_failure_leaves_no_mp3` |
| 9 | Message édité = exception | Message édité traité comme un message | ROUGE `test_point_09` / VERT e2e `e_edited_message_is_processed` |
| 10 | Collisions de nom de fichier | Nom basé sur l'identifiant, dossier par tâche | ROUGE `test_point_10` / VERT `download::tests::download_deux_fetch_concurrents_fichiers_distincts` |
| 11 | `/stats` par pseudo, nom injecté dans le Markdown | Administrateur par ID numérique, échappement MarkdownV2 | ROUGE `test_point_11` / VERT `access::tests::admin_by_numeric_id_only`, e2e `g_stats_admin_only_and_escaped` |
| 12 | Conteneur root, sans healthcheck ni limites | Dockerfile épinglé par digest, UID 10001, `HEALTHCHECK`, compose durci | Justification écrite, vérifié à la main, build d'image non rejoué |
| 13 | CI minimale | `ci-rust.yml` : fmt, clippy, tests, audit, Trivy, publish, actions épinglées par SHA | Justification écrite ; `actionlint` seulement (voir Limites) |
| 14 | Dépôt lourd, `.pyc` suivis | Sortis de l'index, `.gitignore` réécrit | Contrôle par commande : `git ls-files \| grep -cE '^ffmpeg/\|\.pyc$'` donne 0 |
| 15 | Code mort, deux caches | Un seul cache, `allow(dead_code)` retiré, clippy `-D warnings` | ROUGE `test_point_15` / VERT `cache::tests::expires_after_ttl`, e2e `i_same_url_twice_uses_cache` |
| 16 | Codes ANSI doublés dans les logs | `tracing_subscriber` avec `.with_ansi(false)` | ROUGE `test_point_16` ; pas de vert dédié (justifié) |
| 17 | Documentation en désaccord avec le code | AGENTS.md, README.md, DOCS.md réécrits | Justification écrite, pas de test |
| 18 | Adresse d'infrastructure et ID Telegram publiés ; jeton à masquer | Ancien Python supprimé, valeurs factices, jeton masqué dans tous les journaux | VERT `config::tests::config_liste_ids`, `tests/token_redaction.rs` (4 tests) ; mutation du writer rouge |

Bilan de l'audit : 17 tests rouges sur l'ancien code, 114 tests verts sur la branche. Cinq points n'ont pas de test et sont justifiés par écrit (6, 12, 13, 14, 17 ; le 16 n'a pas de vert dédié). Recherche de secrets dans le diff (IP privées, ID Telegram, jetons, chemins locaux) : aucun secret ; un seul défaut mineur, voir Limites.

## Nouvelles fonctions

### `/settings` : réglages à chaud

Menu de boutons édité en place, réservé aux administrateurs (reconnus par ID numérique, revérifié à chaque clic). Arborescence courte :

```
/settings
├── Téléchargement   (dl.max_filesize_mb, dl.max_duration_s, dl.playlists, dl.quality)
├── Sources          (providers.order, providers.x/bluesky/tiktok.enabled, providers.ytdlp_fallback)
├── Audio            (audio.music_enabled, audio.bitrate_kbps)
├── Envoi            (send.telegram_limit_mb, send.external_fallback, send.compress)
├── Stockage         (sto.retention_*, sto.min_free_mb, sto.cleanup_min)
├── Accès            (acc.allowed_users, acc.rate_limit_per_min, acc.admins)
└── Système          (sys.version, sys.ytdlp_update_h, sys.log_level)
```

Persistance dans `SETTINGS_FILE` (JSON, écriture atomique, relu au démarrage, l'emporte sur le `.env`). Chaque changement est consigné dans `settings-audit.jsonl` (clé, valeurs, ID de l'admin). Plusieurs clés déclarées dans le schéma ne sont pas encore branchées (`dl.*`, `audio.*`, `send.compress`, `sys.ytdlp_update_h`) : la liste exacte est dans le README.

### Couche hybride de sources

Par défaut tout passe par yt-dlp seul. X, Bluesky et TikTok sont désactivées ; on les active une à une dans `/settings`. Une source activée est essayée avant yt-dlp, dans l'ordre de `providers.order`, pour les téléchargements vidéo (pas `/music`). Si elle échoue, repli sur yt-dlp tant que `providers.ytdlp_fallback` est actif. Le fichier direct respecte les mêmes bornes de taille et de durée. Tests : `tests/e2e_providers.rs` (5) et `tests/e2e_settings.rs` (7).

## Comment tester

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked          # 114 tests attendus, 0 échoué
```

Lancement local avec un `.env` minimal (seul `BOT_TOKEN` est obligatoire) :

```bash
cp .env.example .env         # renseigner BOT_TOKEN et ALLOWED_USER_IDS (valeurs factices dans le modèle)
set -a; . ./.env; set +a
cargo run --release
```

Conteneur :

```bash
cp .env.example .env
docker compose up -d --build
```

Rejouer les preuves rouges de l'ancien bot : voir `docs/refonte-rust/preuves-ancien/README.md`.

## Limites

- **La CI GitHub n'a jamais été exécutée.** `ci-rust.yml` n'a été validé que statiquement : `actionlint` ne signale que le libellé de runner auto-hébergé `nas` (4 erreurs `runner-label`, sortie 0 avec `-ignore`). Le premier run réel dira si `cargo audit` ou Trivy échouent.
- **yt-dlp et ffmpeg restent des binaires externes** : débit et latence en dépendent, Rust ne les accélère pas. Les e2e utilisent un faux Telegram, un faux yt-dlp et un faux ffmpeg ; seul le chemin `.cmd` (Windows) a été exécuté, le chemin `.sh` ne l'a pas été.
- **API tierces** : TikTok passe par un service externe tiers (hors de notre contrôle), désactivé par défaut ; les API publiques X et Bluesky peuvent changer sans préavis.
- **Commit `37dd146` rouge pris seul** : les e2e ont été ajoutés avant leur correctif `2850bf0`. L'historique n'est pas réécrit (choix assumé) ; pour `git bisect`, faire `git bisect skip 37dd146`.
- Le banc de latence a été mesuré avec un binaire patché d'un caractère (`duration? <=`, bug corrigé ensuite par `767ba1c`) : les valeurs de latence et de débit ne valent que pour ce chemin.
- Le test e2e `b_slow_user_does_not_block_other` est sensible à la charge machine (échoué une fois pendant un `docker build` en parallèle, vert seul).
- DNS rebinding non couvert (`shortcut:` dans `src/url_guard.rs`) ; cache en mémoire seule.
- Mineur : `docs/refonte-rust/preuves-ancien/RESULTAT-ancien.txt` contient 39 lignes avec un chemin local du poste (pas un secret ; bug SOCIAL-2611050551-izv).
- Les valeurs historiques (ID, adresse) restent dans l'historique git, non réécrit.
- Les lignes de code ne se comparent pas à fonctionnalité égale : le Rust compte les tests unitaires en ligne et la refonte ajoute des fonctions absentes de l'ancien bot.
