# Refonte Rust du bot Telegram : audit en 18 points, /settings et sources hybrides

## Résumé

- Remplace l'ancien bot Python (base `a467a68`) par un bot Rust (teloxide 0.17, tokio) ; `yt-dlp` et `ffmpeg` restent des programmes externes.
- Corrige les 18 points d'un audit de l'ancien bot : chaque point a un constat, une correction et un test (ou une justification écrite).
- Ajoute `/settings` (réglages à chaud par boutons, réservé aux administrateurs) et une couche hybride de sources (X, Bluesky, TikTok), désactivées par défaut.
- Durcit le conteneur (image en UID 10001 ; compose en root comme l'original mais avec la seule capacité `DAC_OVERRIDE`, système de fichiers en lecture seule) et ajoute un workflow CI `ci-rust.yml`.
- Honnêtement : le Rust n'est pas plus rapide sur ce banc ; l'apport est la sûreté et la testabilité. Premier run de la CI GitHub : trois échecs d'outillage, corrigés (voir « Limites »).

## Motivation

L'audit de l'ancien bot a trouvé 18 défauts : un téléchargement qui bloque tous les utilisateurs, aucune limite d'accès ni garde anti-SSRF, un nettoyage qui supprime les téléchargements en cours, un envoi chez un tiers sans avertissement, un conteneur root avec toutes les capacités par défaut, une documentation périmée, une adresse d'infrastructure et un ID Telegram publiés. Dix-sept tests Python rejoués sur `a467a68` échouent tous (17 échoués, 0 passé, MESURÉ), ce qui fixe la ligne de départ. La refonte corrige chaque point par un test qui rejoue le vrai parcours.

## Avant / après

Source : `docs/refonte-rust/STATS.md`. MESURÉ = sorti d'une commande rejouable ; DÉDUIT = calculé à partir de mesures. Latence, débit et démarrage : médiane de 3 runs. Les écarts sont DÉDUITS des deux colonnes.

| Métrique | Avant (a467a68) | Après (6f42b8a) | Écart | Étiquette |
|---|---|---|---|---|
| Lignes de code applicatif | 1 113 (Python, 21 fichiers) | 3 410 (Rust, `src/`) | +206,4 % | MESURÉ |
| Dépendances directes | 7 | 11 | +57,1 % | MESURÉ |
| Dépendances totales | 18 (venv) | 205 (arbre cargo) | +1 038,9 % | MESURÉ |
| Taille d'image Docker | 116 510 831 o (111,1 Mio) | 182 466 181 o (174,0 Mio) | +56,6 % | MESURÉ |
| Mémoire au repos (30 s) | 48,11 Mio (RSS) | 6,375 Mio (RSS) | −86,7 % | MESURÉ |
| Temps de démarrage | 1,05 s | 0,023 s | −97,8 % | MESURÉ |
| Latence p50, 10 simultanés | 2,574 s | 3,638 s | +41,3 % | MESURÉ |
| Latence p95, 10 simultanés | 4,626 s | 4,998 s | +8,0 % | MESURÉ |
| Débit, 10 simultanés | 5,063 Mo/s | 4,687 Mo/s | −7,4 % | MESURÉ |

Code actuel (re-mesure du 2026-11-05, commit `f8f27ce`, STATS.md, MESURÉ) : 6 007 lignes Rust dans `src/`, 1 362 lignes de tests, 114 tests (83 unitaires + 31 d'intégration), 11 dépendances directes, 205 au total, image Docker de 182 623 255 o (174,2 Mio). La taille du binaire release n'a pas été relevée (DÉDUIT : non mesurée). Latence, débit, mémoire et démarrage n'ont pas été refaits sur le code actuel : le tableau ci-dessus reste celui du `6f42b8a`. Au 06/11/2026, après les améliorations décidées par Tom : 145 tests (109 unitaires + 36 d'intégration, MESURÉ par `cargo test --locked`) ; lignes et image non re-mesurées.

Lecture honnête : les écarts de latence et de débit sont du même ordre que la dispersion entre runs (Rust : p50 de 3,079 à 5,415 s). Ils ne prouvent pas que le Rust est plus lent, ils prouvent qu'il n'est pas plus rapide sur ce banc. Le p95 est calculé sur 10 échantillons : c'est le maximum.

## Les 18 points d'audit : correction et test

Source : `docs/refonte-rust/INGEST.md` (rejoué le 05/11/2026 sur `f8f27ce`). ROUGE = test de l'ancien code (`docs/refonte-rust/preuves-ancien/`, sur `a467a68`) ou mutation ; VERT = test Rust de la branche.

| Point | Défaut | Correction | Test (rouge avant / vert après) |
|---|---|---|---|
| 1 | Un téléchargement bloque tous les utilisateurs | Handlers teloxide, une tâche par message | ROUGE `test_point_01` / VERT e2e `b_slow_user_does_not_block_other` |
| 2 | Aucun contrôle d'accès, SSRF | `url_guard.rs`, `access.rs`, arguments yt-dlp bornés | ROUGE `test_point_02` / VERT `url_guard::tests::refuses_internal_targets`, `access::tests::user_outside_list_refused`, e2e `d_ssrf_refused` ; mutation `<=?` en `<=` rouge |
| 3 | Le nettoyage supprime les téléchargements en cours | Dossier par tâche verrouillé (`JobDir`), épargné par `retention.rs` | ROUGE `test_point_03` / VERT `retention::tests::retention_jobdir_vivant_survit`, e2e `j_cleanup_spares_running_job` |
| 4 | `curl_uploader` renvoie un 4xx/5xx comme URL, sans timeout | `egress.rs` : statut vérifié, HTML refusé, timeouts | ROUGE `test_point_04` / VERT `egress::tests::egress_500_refuse`, e2e `f_external_500_gives_error_without_link` |
| 5 | Envoi chez un tiers sans avertissement | Par défaut, consentement Oui/Non nommant l'hôte avant chaque envoi chez l'hébergeur du bot Python (décision de Tom) ; « Accord permanent » dans `/settings`, ou `EXTERNAL_UPLOAD_CONSENT=0`, envoie sans redemander comme l'original | ROUGE `test_point_05` / VERT `egress::tests::consentement_nomme_hote`, e2e `f_oversize_needs_consent_then_uploads` (défaut), `f_refus_puis_accord_permanent` et `f0_oversize_goes_external_like_python` (`EXTERNAL_UPLOAD_CONSENT=0`) |
| 6 | PTB 13.7 en fin de vie | teloxide 0.17, reqwest rustls, `cargo audit` en CI | Justification écrite, pas de test |
| 7 | yt-dlp installé au build seulement | Version épinglée par sha256 + mise à jour au démarrage | VERT `startup::b_updater_calls_ytdlp_only_when_enabled` ; mutation rouge si le lancement est retiré |
| 8 | `/music` télécharge la vidéo entière, mp3 partiel réutilisé | Audio seul, conversion mp3 atomique | ROUGE `test_point_08` / VERT `download::tests::download_audio_sans_video_entiere`, e2e `h_music_sends_audio_and_failure_leaves_no_mp3` |
| 9 | Message édité = exception | Message édité traité comme un message | ROUGE `test_point_09` / VERT e2e `e_edited_message_is_processed` |
| 10 | Collisions de nom de fichier | Nom basé sur l'identifiant, dossier par tâche | ROUGE `test_point_10` / VERT `download::tests::download_deux_fetch_concurrents_fichiers_distincts` |
| 11 | `/stats` par pseudo, nom injecté dans le Markdown | Administrateur par ID numérique, échappement MarkdownV2 | ROUGE `test_point_11` / VERT `access::tests::admin_by_numeric_id_only`, e2e `g_stats_admin_only_and_escaped` |
| 12 | Conteneur root, sans healthcheck ni limites | Dockerfile épinglé par digest, image en UID 10001, `HEALTHCHECK` ; compose en root comme l'original (dossiers de l'hôte inscriptibles sans `chown`) mais avec la seule capacité `DAC_OVERRIDE`, lecture seule, `no-new-privileges`, limites | Justification écrite ; essai Docker du 06/11 (MESURÉ) : root sans capacité refusé sur un dossier `1000:1000`, accepté avec `DAC_OVERRIDE` |
| 13 | CI minimale | `ci-rust.yml` : fmt, clippy, tests, audit, Trivy, publish, actions épinglées par SHA | Justification écrite ; `actionlint` seulement (voir Limites) |
| 14 | Dépôt lourd, `.pyc` suivis | Sortis de l'index, `.gitignore` réécrit | Contrôle par commande : `git ls-files \| grep -cE '^ffmpeg/\|\.pyc$'` donne 0 |
| 15 | Code mort, deux caches | Un seul cache, `allow(dead_code)` retiré, clippy `-D warnings` | ROUGE `test_point_15` / VERT `cache::tests::expires_after_ttl`, e2e `i_same_url_twice_uses_cache` |
| 16 | Codes ANSI doublés dans les logs | Couleurs du bot Python en console seulement (`NO_COLOR` respecté), jamais dans le fichier de journal | ROUGE `test_point_16` / VERT `telemetry::tests::couleurs_en_console_jamais_dans_le_fichier` ; mutation (couleurs dans le fichier) rouge |
| 17 | Documentation en désaccord avec le code | AGENTS.md, README.md, DOCS.md réécrits | Justification écrite, pas de test |
| 18 | Adresse d'infrastructure et ID Telegram publiés ; jeton à masquer | Ancien Python supprimé, valeurs factices, jeton masqué dans tous les journaux | VERT `config::tests::config_liste_ids`, `tests/token_redaction.rs` (4 tests) ; mutation du writer rouge |

Bilan de l'audit : 17 tests rouges sur l'ancien code, 145 tests verts sur la branche (06/11). Cinq points n'ont pas de test automatique et sont justifiés par écrit (6, 12, 13, 14, 17). Recherche de secrets dans le diff (IP privées, ID Telegram, jetons, chemins locaux) : aucun secret ; un seul défaut mineur, voir Limites.

## Nouvelles fonctions

### `/settings` : réglages à chaud

Menu de boutons édité en place, réservé aux administrateurs (reconnus par ID numérique, revérifié à chaque clic). Arborescence courte :

```
/settings
├── Téléchargement   (dl.max_filesize_mb, dl.max_duration_s, dl.quality)
├── Sources          (providers.order, providers.x/bluesky/tiktok.enabled, providers.ytdlp_fallback)
├── Audio            (audio.music_enabled, audio.bitrate_kbps)
├── Envoi            (send.telegram_limit_mb, send.external_fallback, send.external_always_yes, send.compress)
├── Stockage         (sto.retention_*, sto.min_free_mb, sto.cleanup_min)
├── Accès            (acc.allowed_users, acc.rate_limit_per_min, acc.rate_limit_burst, acc.admins)
└── Système          (sys.version, sys.ytdlp_update_h, sys.log_level)
```

Persistance dans `SETTINGS_FILE` (JSON, écriture atomique, relu au démarrage, l'emporte sur le `.env`). Chaque changement est consigné dans `settings-audit.jsonl` (clé, valeurs, ID de l'admin). Toutes les clés sont branchées : à chaud, sauf `sto.cleanup_min`, `sys.ytdlp_update_h` et `sys.log_level`, pris au prochain démarrage (détail dans le README).

### Couche hybride de sources

Par défaut tout passe par yt-dlp seul. X, Bluesky et TikTok sont désactivées ; on les active une à une dans `/settings`. Une source activée est essayée avant yt-dlp, dans l'ordre de `providers.order`, pour les téléchargements vidéo (pas `/music`). Si elle échoue, repli sur yt-dlp tant que `providers.ytdlp_fallback` est actif. Le fichier direct respecte les mêmes bornes de taille et de durée. Tests : `tests/e2e_providers.rs` (5) et `tests/e2e_settings.rs` (8).

### `/stats` enrichi

`/stats` reste réservé aux administrateurs (ID numérique, comme `/settings`). Il affiche le texte d'origine du bot Python, puis un bloc « Activité » depuis le démarrage : tâches actives, envois en attente d'accord, plus gros fichier, réussites et échecs (au total et par source), taux de réussite, durée médiane et p95, erreurs les plus fréquentes (messages montrés à l'utilisateur, sans URL ni identifiant). Modules `src/metrics.rs` et `src/bot/stats.rs` testés. Limites : compteurs en mémoire (remis à zéro au redémarrage), pas de période ni d'export.

## Mise à jour d'une installation existante (compatibilité 1:1)

Le `.env` du bot Python fonctionne tel quel : `VERSION`, `DEVELOPED_BY`, `FFMPEG_PATH`, `CLEANUP_INTERVAL_HOURS`, `MIN_FREE_SPACE_MB`, `SMALL_FILE_SIZE_MB`, `RETENTION_SMALL_HOURS`, `RETENTION_LARGE_HOURS` sont lus avec le même sens (rétention à deux niveaux : mp3 ou fichier < seuil = « petit », sinon « gros »). Un `FFMPEG_PATH` qui n'existe pas dans l'image retombe sur `ffmpeg` du PATH.

- **Accès** : comme l'original, le bot est ouvert à tous tant que `ALLOWED_USER_IDS` est vide (un avertissement est journalisé) ; `ADMIN_USER_IDS` ne le restreint pas. Renseignée, `ALLOWED_USER_IDS` le restreint à ces IDs ; un administrateur y a toujours accès. `ADMIN_USER_IDS=<ID>` (celui qui était en dur dans `commands/stats.py`) ouvre `/stats` et `/settings`.
- **`/start` et `/help`** : textes identiques à l'original (Markdown, version et « Développé par » lus dans l'environnement).
- **Podman et Docker** : les images de base du `Dockerfile` sont qualifiées (`docker.io/…`), plus d'erreur « short-name … did not resolve ». Plus aucun `chown` : le compose tourne en root comme l'original, avec la seule capacité `DAC_OVERRIDE`, et écrit dans `./downloads`, `./logs` et `./state` quel que soit leur propriétaire (essai Docker du 06/11, MESURÉ : sans cette capacité, root est refusé sur un dossier `1000:1000`).
- **Réglages** : `/settings` s'écrit dans `./state` (monté sur `/data/state`, `SETTINGS_FILE` par défaut dans l'image), car le système de fichiers du conteneur est en lecture seule.
- **yt-dlp** : `/tmp` du conteneur est monté `exec` (le binaire officiel s'y extrait et y charge `libz.so.1`), avec un système de fichiers toujours en lecture seule.

## Passe de parité 1:1 avec le bot Python (05-06/11/2026)

Demande de Tom : « un vrai 1:1 » en fonctions et en affichage ; on améliore, on ne retire rien. Tableau fichier par fichier, différences restantes et décisions ouvertes : `docs/refonte-rust/AUDIT-1-1.md`.

- **Accès** : ouvert à tous par défaut ; `ADMIN_USER_IDS` ne réserve que `/stats` et `/settings`.
- **Envoi** : textes d'origine, 3 tentatives de téléchargement, progression par paliers de 10 %, vidéo, audio ou document selon l'extension, nom affiché tiré du titre, menu des commandes.
- **Repli externe** : `curl.libriciel.fr` par défaut, envoyé sous le nom de son sha256, 3 essais.
- **Journaux** : format `AAAA-MM-JJ HH:MM:SS,mmm - NIVEAU - message`, couleurs par catégorie en console, un fichier par jour dans `./logs` (sans couleurs), niveau `debug`, mêmes lignes `[INIT]`, `[UPLOAD]`, `[CURL UPLOAD]`, `[DISK_MANAGER]`.
- **Démarrage** : `.env` lu sans écraser l'environnement, créé depuis un modèle s'il manque, jeton modèle refusé avec le message d'origine ; token refusé par Telegram : message clair et sortie en échec, au lieu d'une panique ; dossier de téléchargements vidé ; espace disque vérifié avant chaque téléchargement.
- **`/settings`** : toutes les clés branchées ; `dl.playlists` retiré (un lien = une vidéo, comme l'original) ; `sys.version` = `VERSION`, comme `/help`.
- **Conteneur** : root comme l'original, avec la seule capacité `DAC_OVERRIDE` (plus de `chown`) ; `BOT_TOKEN` du shell prioritaire, comme le compose d'origine.

## Améliorations décidées par Tom (06/11/2026)

- **Limite de débit en seau de jetons** : 20 liens d'affilée, puis 10 par minute (`RATE_LIMIT_BURST=20`, `RATE_LIMIT_PER_MIN=10`, réglables à chaud dans `/settings` › Accès). Le délai annoncé (« réessayez dans N s ») est celui du prochain jeton. Tests `access::tests::burst_then_steady_rate`, `bucket_capped_at_burst`, e2e `limite_de_debit_appliquee_a_chaud_et_auditee`.
- **Accord avant l'envoi externe, par défaut** : Oui/Non à chaque fichier trop gros ; « Accord permanent » dans `/settings` › Envoi pour ne plus redemander. Test e2e `f_refus_puis_accord_permanent` (Non : rien ne part ; accord permanent : envoi direct, plus de question).
- **Seuil** : limite de Telegram, toujours 50 Mo pour l'API Bot publique (vérifiée le 06/11/2026) ; inchangé.
- **Gardés** : anti-SSRF, administrateurs par ID numérique.

## Comment tester

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked          # 145 tests attendus, 0 échoué
```

Lancement local avec un `.env` minimal (seul `BOT_TOKEN` est obligatoire) :

```bash
cp .env.example .env         # renseigner BOT_TOKEN (sans .env, le bot en crée un et dit quoi faire)
cargo run --release          # lit ./.env lui-même, comme load_dotenv du bot Python
```

Conteneur :

```bash
cp .env.example .env
docker compose up -d --build
```

Rejouer les preuves rouges de l'ancien bot : voir `docs/refonte-rust/preuves-ancien/README.md`.

## Limites

- **CI GitHub** : premier run réel (06/11, sur `adbc1c3`, MESURÉ) : `audit`, `check` et `image` en échec, corrigés par `bf33ac1` : avis RUSTSEC-2026-0173 ignoré (`proc-macro-error2` non maintenu, pas une faille, tiré par teloxide à la compilation), 2 tests `startup` qui ouvraient un dossier en écriture (refusé sous Linux ; vérifiés 3/3 dans `rust:1.99`), Trivy v0.65.0 retiré par Aqua (action v0.36.0 + Trivy v0.76.2). Le scan Trivy de l'image n'a encore jamais abouti : son premier passage peut signaler des failles. `ci-rust.yml` n'a été validé que statiquement : `actionlint` (avec shellcheck) ne signale plus rien. Le premier run réel dira si `cargo audit` ou Trivy échouent. La publication arm64 compile le Rust sous QEMU : elle sera lente (plusieurs minutes) ; si c'est gênant, on peut passer à un runner arm64 natif.
- **Conteneur essayé pour de vrai** (06/11, Docker Desktop, amd64, image reconstruite sur cette version, MESURÉ) : sans jeton, message clair et code 1 ; avec un jeton refusé par Telegram, « [INIT] Telegram refuse le token : vérifiez BOT_TOKEN dans le .env. » et code 1 (avant ce correctif : panique de teloxide, code 101), puis relance par `restart: unless-stopped` comme le compose d'origine ; dossiers `1000:1000` conservés ; journal du jour écrit dans `./logs`, sans aucun code de couleur ; `yt-dlp --update-to stable` dans l'image : « yt-dlp is up to date (stable@2026.10.26…) ». Pas essayés : un vrai téléchargement avec un vrai jeton, Podman, arm64.
- **yt-dlp et ffmpeg restent des binaires externes** : débit et latence en dépendent, Rust ne les accélère pas. Les e2e utilisent un faux Telegram, un faux yt-dlp et un faux ffmpeg ; seul le chemin `.cmd` (Windows) a été exécuté, le chemin `.sh` ne l'a pas été.
- **API tierces** : TikTok passe par un service externe tiers (hors de notre contrôle), désactivé par défaut ; les API publiques X et Bluesky peuvent changer sans préavis.
- **Commit `37dd146` rouge pris seul** : les e2e ont été ajoutés avant leur correctif `2850bf0`. L'historique n'est pas réécrit (choix assumé) ; pour `git bisect`, faire `git bisect skip 37dd146`.
- Le banc de latence a été mesuré avec un binaire patché d'un caractère (`duration? <=`, bug corrigé ensuite par `767ba1c`) : les valeurs de latence et de débit ne valent que pour ce chemin.
- Deux tests e2e attendent un faux yt-dlp lent (5 s) pendant 20 s au plus et restent sensibles à la charge machine sous Windows : `b_slow_user_does_not_block_other` (échoué une fois pendant un `docker build` en parallèle) et `j_cleanup_spares_running_job` ; le 06/11, ils ont échoué ensemble sur un passage où le poste était très chargé (processeur à 96 % mesuré peu après), puis 15/15 verts sur 3 passages de suite (MESURÉ).
- DNS rebinding non couvert (`shortcut:` dans `src/url_guard.rs`) ; cache en mémoire seule.
- Mineur : les chemins locaux de `docs/refonte-rust/preuves-ancien/RESULTAT-ancien.txt` sont masqués (`<HOME>`) depuis `c3790d2` ; la version non masquée reste dans l'historique de la branche (`48323c9`), qu'un « Squash and merge » ne recopie pas dans l'historique de `main` (elle reste visible dans les commits de la PR).
- Les valeurs historiques (ID, adresse) restent dans l'historique git, non réécrit.
- Les lignes de code ne se comparent pas à fonctionnalité égale : le Rust compte les tests unitaires en ligne et la refonte ajoute des fonctions absentes de l'ancien bot.

## À propos des −147 000 lignes du diff

Ce n'est pas du code retiré : 145 762 de ces lignes viennent du dossier `ffmpeg/` (binaire statique 7.0.2, pages de manuel, modèles VMAF) qui était commité dans le dépôt (audit, point 14). Le Dockerfile télécharge ffmpeg lui-même depuis une image figée par empreinte, donc le dépôt n'en a plus besoin. Le reste (~1 950 lignes) est l'ancien code Python et ses anciens .pyc. Retirer `ffmpeg/` n'allège pas l'historique git (le binaire reste dans les anciens commits) ; le réduire demanderait de réécrire l'historique, ce que cette PR ne fait pas.
