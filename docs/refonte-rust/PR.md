# Refonte Rust du bot Telegram : audit en 18 points, /settings et sources hybrides

## Résumé

- Remplace l'ancien bot Python (base `a467a68`) par un bot Rust (teloxide 0.17, tokio) ; `yt-dlp` et `ffmpeg` restent des programmes externes.
- Corrige les 18 points d'un audit de l'ancien bot : chaque point a un constat, une correction et un test (ou une justification écrite).
- Ajoute `/settings` (réglages à chaud par boutons, réservé aux administrateurs) et une couche hybride de sources (X, Bluesky, TikTok), désactivées par défaut.
- Durcit le conteneur (image en UID 10001 ; compose en root comme l'original mais avec la seule capacité `DAC_OVERRIDE`, système de fichiers en lecture seule) et ajoute un workflow CI `ci-rust.yml`.
- Banc final du 06/11 (Python et Rust mesurés dans la même session) : 10 demandes simultanées servies en 3,06 s contre 4,28 s (−28,5 %), débit +39,8 %, démarrage 48 fois plus court, mémoire au repos −85 % ; le p50 reste plus haut (+23,9 %) et l'image plus lourde (+59,8 %). Détail dans « Bilan : Python contre Rust ».

## Motivation

L'audit de l'ancien bot a trouvé 18 défauts : un téléchargement qui bloque tous les utilisateurs, aucune limite d'accès ni garde anti-SSRF, un nettoyage qui supprime les téléchargements en cours, un envoi chez un tiers sans avertissement, un conteneur root avec toutes les capacités par défaut, une documentation périmée, une adresse d'infrastructure et un ID Telegram publiés. Dix-sept tests Python rejoués sur `a467a68` échouent tous (17 échoués, 0 passé, MESURÉ), ce qui fixe la ligne de départ. La refonte corrige chaque point par un test qui rejoue le vrai parcours.

## Avant / après

Sources : `docs/refonte-rust/mesures/final.md` (banc du 06/11, les deux bots dans la même session) et `docs/refonte-rust/STATS.md` (code et dépendances). MESURÉ = sorti d'une commande rejouable ; DÉDUIT = calculé à partir de mesures. Latence, débit et démarrage : médiane de 3 runs, 10 utilisateurs simultanés. Les écarts sont DÉDUITS des deux colonnes.

| Métrique | Python (a467a68) | Rust (cette version) | Écart | Étiquette |
|---|---|---|---|---|
| Démarrage | 1,048 s | 0,022 s | −97,9 % | MESURÉ |
| Latence p50 | 2,405 s | 2,979 s | +23,9 % | MESURÉ |
| Latence p95 (= le dernier des 10) | 4,277 s | 3,060 s | −28,5 % | MESURÉ |
| Débit | 5,476 Mo/s | 7,655 Mo/s | +39,8 % | MESURÉ |
| Mémoire au repos (RSS) | 48,11 Mio | 7,0 Mio | −85,4 % | MESURÉ |
| Taille d'image Docker (compressée, `docker image inspect --format '{{.Size}}'`) | 116 510 831 o (111,1 Mio) | 186 214 112 o (177,6 Mio) | +59,8 % | MESURÉ |
| Lignes de code (`src/`, tests unitaires compris côté Rust) | 1 113 (Python, 21 fichiers) | 8 442 | ×7,6 | MESURÉ |
| Lignes de tests d'intégration (`tests/`) | 0 | 1 555 | — | MESURÉ |
| Dépendances directes / totales | 7 / 18 | 11 / 205 | — | MESURÉ |

Lecture : le Python sert vite les premiers puis la file s'allonge (de 0,80 à 4,28 s au run médian) ; le Rust traite les 10 demandes en parallèle et elles finissent presque ensemble (de 2,57 à 3,06 s). Le p95 est calculé sur 10 échantillons : c'est le maximum. Le premier banc (`6f42b8a`, 05/11) donnait un Rust plus lent (p50 3,638 s, débit 4,687 Mo/s) : la passe de latence du 06/11 l'a fait passer devant sur le débit et le temps total, sans rattraper le p50 (détail dans « Bilan »).

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

Bilan de l'audit : 17 tests rouges sur l'ancien code, 151 tests verts sous Windows et 153 sous Linux sur la branche (06/11). Cinq points n'ont pas de test automatique et sont justifiés par écrit (6, 12, 13, 14, 17). Recherche de secrets dans le diff (IP privées, ID Telegram, jetons, chemins locaux) : aucun secret ; un seul défaut mineur, voir Limites.

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

`/stats` reste réservé aux administrateurs (ID numérique, comme `/settings`). Il affiche le texte d'origine du bot Python, puis un bloc « Activité » depuis le démarrage : tâches actives et pic de tâches simultanées, envois en attente d'accord, RAM du bot, demandes (vidéos, musiques, nombre d'utilisateurs distincts, jamais leurs ID), fichiers et Mo envoyés par Telegram, envois par lien externe, plus gros fichier, réussites et échecs (au total et par source), taux de réussite, durée médiane et p95, erreurs les plus fréquentes (messages montrés à l'utilisateur, sans URL ni identifiant). Modules `src/metrics.rs` et `src/bot/stats.rs` testés. Limites : compteurs en mémoire (remis à zéro au redémarrage), pas de période ni d'export.

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
- **Envoi** : textes d'origine, 3 tentatives de téléchargement, progression par paliers de 10 %, vidéo, audio ou document selon l'extension, nom affiché tiré du titre, menu des commandes (avec `/music` en plus, que le bot Python n'annonçait pas).
- **Repli externe** : `curl.libriciel.fr` par défaut, envoyé sous le nom de son sha256, 3 essais.
- **Journaux** : format `AAAA-MM-JJ HH:MM:SS,mmm - NIVEAU - message`, couleurs par catégorie en console, un fichier par jour dans `./logs` (sans couleurs), niveau `debug`, mêmes lignes `[INIT]`, `[UPLOAD]`, `[CURL UPLOAD]`, `[DISK_MANAGER]`.
- **Démarrage** : `.env` lu sans écraser l'environnement, créé depuis un modèle s'il manque, jeton modèle refusé avec le message d'origine ; token refusé par Telegram : message clair et sortie en échec, au lieu d'une panique ; dossier de téléchargements vidé (et créé s'il manque : sur un volume neuf, le nettoyage périodique journalisait « nettoyage échoué ») ; espace disque vérifié avant chaque téléchargement.
- **`/settings`** : toutes les clés branchées ; `dl.playlists` retiré (un lien = une vidéo, comme l'original) ; `sys.version` = `VERSION`, comme `/help`.
- **Conteneur** : root comme l'original, avec la seule capacité `DAC_OVERRIDE` (plus de `chown`) ; `BOT_TOKEN` du shell prioritaire, comme le compose d'origine.

## Améliorations décidées par Tom (06/11/2026)

- **Limite de débit en seau de jetons** : 20 liens d'affilée, puis 10 par minute (`RATE_LIMIT_BURST=20`, `RATE_LIMIT_PER_MIN=10`, réglables à chaud dans `/settings` › Accès). Le délai annoncé (« réessayez dans N s ») est celui du prochain jeton. Tests `access::tests::burst_then_steady_rate`, `bucket_capped_at_burst`, e2e `limite_de_debit_appliquee_a_chaud_et_auditee`.
- **Accord avant l'envoi externe, par défaut** : Oui/Non à chaque fichier trop gros ; « Accord permanent » dans `/settings` › Envoi pour ne plus redemander. Test e2e `f_refus_puis_accord_permanent` (Non : rien ne part ; accord permanent : envoi direct, plus de question).
- **Hébergeur nommé** : la question cite l'hôte de `EXTERNAL_UPLOAD_URL` (« un hébergeur tiers (curl.libriciel.fr) ») au lieu de répéter « un hébergeur tiers ». Test e2e `f_oversize_needs_consent_then_uploads`.
- **Cache des liens externes** : un même lien (même mode) redemandé renvoie le lien déjà créé, sans retélécharger ni renvoyer le fichier ; durée 1 h (celle des fichiers de plus de 5 Mo). Test e2e `f0_lien_externe_en_cache` (deux demandes, un seul envoi, la 2e réponse dit « 📦 Envoyé depuis le cache »).
- **File d'attente** (signalée par Tom : une rafale de liens semblait traitée un par un) : une place de `MAX_CONCURRENT_DOWNLOADS` (3 par défaut, pour tous les utilisateurs) n'est plus tenue que pendant le téléchargement, pas pendant la conversion ni l'envoi à Telegram ; un lien qui attend une place affiche « ⏳ En file d'attente... ». Test e2e `file_d_attente_annoncee`.
- **Seuil** : limite de Telegram, toujours 50 Mo pour l'API Bot publique (vérifiée le 06/11/2026) ; inchangé.
- **Gardés** : anti-SSRF, administrateurs par ID numérique.
- **Pelican/Pterodactyl** : egg `pelican/egg-socialvideodownload.json` et image `:pelican` (même `Dockerfile`, cible `pelican`), construite, scannée par Trivy, essayée et publiée par la CI (`:pelican`, `:pelican-VERSION`, `:pelican-SHA` sur `main` ; `:pelican-dev` sur `develop`). Essai façon Wings en local (MESURÉ) : UID quelconque (988), volume sur `/home/container`, polling lancé en 1 s ; `bin/` et `tmp/` créés en 988:988 ; au redémarrage, `cp -u` garde la copie de yt-dlp (`--update-to stable` : « up to date ») et `tmp/` repart vide ; `VERSION` de `/help` = celle de `.env.example` (`V10.0.0`), ou la variable de l'egg si elle existe. yt-dlp s'extrait à chaque lancement : dans un `/tmp` tmpfs de 20 ou 50 Mo il échoue, à 100 Mo il passe, d'où `TMPDIR` dans `tmp/` du volume. Pas essayé : un vrai panel.
- **Arrêt propre** : `STOPSIGNAL SIGINT` dans les deux images, car le bot ne gère que SIGINT (^C). Mesuré : 1,5 s et code 0, contre 9,4 s et code 137 (tué) avec le SIGTERM par défaut de `docker stop -t 8`. Sous Wings, le `^C` de l'egg devient un `docker stop` sans délai (DÉDUIT du code du panel et de Wings) : sans ce réglage, le bot tournait encore après 15 s (MESURÉ).

## Re-check complet et passe de latence (06/11/2026)

Demande de Tom : `/stats` enrichi, `/music` annoncé dans le menu de Telegram, puis un re-check de tout le bot (fonctions, performance, optimisation) avant la comparaison finale.

Corrigés pendant le re-check, chacun avec un test :

- **Qualité YouTube** : le format demandé à yt-dlp mettait `b[ext=mp4]` en tête, ce qui donnait le 360p de YouTube (seul flux mp4 tout-en-un). La meilleure qualité passe maintenant d'abord, comme le défaut `bv*+ba/b` du bot Python, avec le mp4 préféré à qualité égale et la taille toujours bornée par `--max-filesize`. Test `download::tests::reglages_appliques_a_yt_dlp`.
- **Contrôle audio** : une erreur de ffprobe pendant le contrôle « vidéo sans son » faisait échouer tout l'envoi ; elle est maintenant journalisée et l'envoi continue.
- **Message d'erreur précis** : un lien non pris en charge, un fichier trop gros, un média trop long ou un délai dépassé étaient réessayés 3 fois, puis affichés comme « échec après plusieurs tentatives ». Ces erreurs définitives s'arrêtent maintenant au 1er essai, avec leur message ; les pannes d'extracteur gardent leurs 3 tentatives. Tests e2e `k2_erreur_definitive_sans_nouvelle_tentative` et `k_errors_are_never_silent`.
- **Cache** : un fichier envoyé en document (extension inconnue) était mis en cache puis renvoyé comme vidéo ; il n'est plus mis en cache.
- **Message d'origine supprimé** : si l'utilisateur supprime son message pendant le téléchargement, les réponses partaient en erreur (« message to be replied not found ») ; elles partent maintenant sans citation (`allow_sending_without_reply`).

Latence :

- Le contrôle audio (ffprobe et volumedetect) tourne en même temps que l'envoi au lieu de le précéder ; l'avertissement « vidéo sans son » arrive juste après le fichier.
- Image standard : yt-dlp en version « dossier », qui ne se décompresse plus dans `/tmp` à chaque lancement : 322 à 394 ms par lancement contre 776 à 1 222 ms (MESURÉ). Coût (MESURÉ) : +1,2 Mo à télécharger (taille compressée, 186 214 112 o contre 185 027 550 o) et +59 Mo sur le disque une fois décompressée (669 Mo contre 610 Mo, `docker images`). L'image Pelican garde la version « un seul fichier », qui sait se mettre à jour seule dans le volume.
- Résultat sur le banc (Rust seul, avant et après cette passe, MESURÉ) : p50 de 3,589 à 2,979 s (−17,0 %), p95 de 3,664 à 3,060 s (−16,5 %), débit de 6,393 à 7,655 Mo/s (+19,7 %).

Relevés sans correction dans cette PR (DÉDUIT de la lecture du code, aucun n'a été vu en panne) :

- Les fichiers trop gros en attente d'un « Oui » restent sur le disque jusqu'à la réponse ou au nettoyage : beaucoup d'utilisateurs qui ne répondent pas peuvent remplir le disque. Piste : un seul envoi en attente par utilisateur.
- Deux demandes du même lien en même temps le téléchargent deux fois (le cache ne sert qu'après le premier envoi).
- La résolution DNS de la garde anti-SSRF n'a pas de délai propre (elle reste bornée par le délai global de la tâche).

## Bilan : Python contre Rust

| Sujet | Bot Python (`a467a68`) | Bot Rust (cette PR) |
|---|---|---|
| Commandes | `/start`, `/help`, `/download`, `/music`, `/stats`, lien direct | les mêmes, mêmes textes ; plus `/settings` ; `/music` annoncé dans le menu |
| Plusieurs utilisateurs | un téléchargement lent bloque tous les autres (MESURÉ, point 1) | une tâche par message, 10 en parallèle sur le banc |
| Temps pour servir 10 demandes simultanées | 4,277 s | 3,060 s (−28,5 %) |
| Premier servi (p50) | 2,405 s | 2,979 s (+23,9 %) |
| Démarrage | 1,048 s | 0,022 s |
| Mémoire au repos | 48,11 Mio | 7,0 Mio |
| Image Docker | 111,1 Mio | 177,6 Mio (ffmpeg et yt-dlp « dossier » embarqués) |
| Sécurité | aucune liste d'accès, pas d'anti-SSRF, root avec toutes les capacités, envoi chez un tiers sans prévenir | liste d'accès optionnelle, anti-SSRF, limite de débit, accord avant l'envoi externe, conteneur durci |
| Fiabilité | nettoyage qui supprime les téléchargements en cours, mp3 partiel réutilisé, erreurs muettes | 18 défauts de l'audit corrigés, toute erreur produit un message |
| Tests | aucun (17 tests écrits pour l'audit, tous rouges sur l'ancien code) | 151 sous Windows, 153 sous Linux, CI GitHub verte |
| Réglages | `.env` et redémarrage | `.env` compatible tel quel, plus `/settings` à chaud |
| `/stats` | cache, disque, journaux, système, URLs enregistrées, rétention | les mêmes rubriques, plus l'activité depuis le démarrage |
| Code | 1 113 lignes, 18 dépendances | 8 442 lignes (tests unitaires compris), 205 dépendances |

Ce que le Rust ne fait pas mieux : le premier servi attend un peu plus (+0,57 s au p50), l'image pèse 66 Mio de plus, le code et l'arbre de dépendances sont bien plus gros. Ce qu'il apporte : le service reste réactif sous charge, la mémoire et le démarrage sont divisés par 7 et par 48, et chaque défaut connu a un test. yt-dlp et ffmpeg restent les mêmes programmes externes : sur un vrai téléchargement, le site source et le réseau pèsent plus que le bot.

## Comment tester

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked          # 151 tests sous Windows, 153 sous Linux, 0 échoué
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

Image Pelican/Pterodactyl (même `Dockerfile`, cible `pelican`) ; l'egg est `pelican/egg-socialvideodownload.json` :

```bash
docker build --target pelican -t socialvideodownload:pelican .
```

Rejouer les preuves rouges de l'ancien bot : voir `docs/refonte-rust/preuves-ancien/README.md`.

## Limites

- **CI GitHub** (MESURÉ) :
  - 1er run réel (06/11, `adbc1c3`) : `audit`, `check` et `image` en échec, corrigés par `bf33ac1` : avis RUSTSEC-2026-0173 ignoré (`proc-macro-error2` non maintenu, pas une faille, tiré par teloxide à la compilation), 2 tests `startup` qui ouvraient un dossier en écriture (refusé sous Linux), Trivy v0.65.0 retiré par Aqua (action v0.36.0 + Trivy v0.76.2).
  - 2e run (`5ec61dc`) : `audit` vert. `check` : un test instable, « Text file busy » (ETXTBSY : un faux outil encore ouvert en écriture par le processus de test quand un test parallèle lance un programme, qui hérite du descripteur) ; 2 échecs sur 25 passages en local, 0 sur 60 depuis que les faux outils sont écrits par un `sh` enfant. `image` : Trivy signale CVE-2026-109518 (HIGH) dans `apt` 2.6.1 de la base Debian figée, corrigée par `apt-get upgrade` à la construction ; Trivy v0.76.2 en local : 0 faille HIGH/CRITICAL corrigeable, sur les deux images.
  - Trouvé en vérifiant : `b_slow_user_does_not_block_other` reconnaissait le chat par un `contains("111")` sur tout le corps multipart, dont la frontière aléatoire peut contenir « 111 » (1 échec sur 25 passages sous Linux) ; il lit maintenant le champ `chat_id`.
  - 3e run (`042c932`, 06/11) : `audit`, `check` et `image` verts, essai de l'image Pelican et deux scans Trivy compris.
  - Pas encore passé sur GitHub : `publish` (seulement sur un push vers `main` ou `develop`). La publication arm64 compile le Rust sous QEMU : elle sera lente (plusieurs minutes) ; si c'est gênant, on peut passer à un runner arm64 natif.
- **Conteneur essayé pour de vrai** (06/11, Docker Desktop, amd64, image reconstruite sur cette version, MESURÉ) : sans jeton, message clair et code 1 ; avec un jeton refusé par Telegram, « [INIT] Telegram refuse le token : vérifiez BOT_TOKEN dans le .env. » et code 1 (avant ce correctif : panique de teloxide, code 101), puis relance par `restart: unless-stopped` comme le compose d'origine ; dossiers `1000:1000` conservés ; journal du jour écrit dans `./logs`, sans aucun code de couleur ; `yt-dlp --update-to stable` dans l'image : « yt-dlp is up to date (stable@2026.10.26…) ». Pas essayés : un vrai téléchargement avec un vrai jeton, Podman, arm64.
- **yt-dlp et ffmpeg restent des binaires externes** : débit et latence en dépendent, Rust ne les accélère pas. Les e2e utilisent un faux Telegram, un faux yt-dlp et un faux ffmpeg, en `.cmd` sous Windows et en `.sh` sous Linux (exécutés tous les deux, le second dans `rust:1.99`).
- **API tierces** : TikTok passe par un service externe tiers (hors de notre contrôle), désactivé par défaut ; les API publiques X et Bluesky peuvent changer sans préavis.
- **Commit `37dd146` rouge pris seul** : les e2e ont été ajoutés avant leur correctif `2850bf0`. L'historique n'est pas réécrit (choix assumé) ; pour `git bisect`, faire `git bisect skip 37dd146`.
- Le premier banc (`6f42b8a`) avait été mesuré avec un binaire patché d'un caractère (`duration? <=`, bug corrigé ensuite par `767ba1c`) ; le banc final du 06/11 tourne sur le binaire de la branche, sans patch.
- Le banc tourne hors Docker sur Windows, avec un faux Telegram et un serveur de média local : il compare les deux bots entre eux, pas la vitesse d'un vrai téléchargement (site source, réseau). Le gain de la version « dossier » de yt-dlp dans l'image (environ 0,4 s par lancement) n'y apparaît donc pas.
- Deux tests e2e attendent un faux yt-dlp lent (5 s) pendant 20 s au plus et restent sensibles à la charge machine sous Windows : `b_slow_user_does_not_block_other` (échoué une fois pendant un `docker build` en parallèle) et `j_cleanup_spares_running_job` ; le 06/11, ils ont échoué ensemble sur un passage où le poste était très chargé (processeur à 96 % mesuré peu après), puis 15/15 verts sur 3 passages de suite (MESURÉ).
- DNS rebinding non couvert (`shortcut:` dans `src/url_guard.rs`) ; cache en mémoire seule.
- Mineur : les chemins locaux de `docs/refonte-rust/preuves-ancien/RESULTAT-ancien.txt` sont masqués (`<HOME>`) depuis `c3790d2` ; la version non masquée reste dans l'historique de la branche (`48323c9`), qu'un « Squash and merge » ne recopie pas dans l'historique de `main` (elle reste visible dans les commits de la PR).
- Les valeurs historiques (ID, adresse) restent dans l'historique git, non réécrit.
- Les lignes de code ne se comparent pas à fonctionnalité égale : le Rust compte les tests unitaires en ligne et la refonte ajoute des fonctions absentes de l'ancien bot.

## À propos des −147 000 lignes du diff

Ce n'est pas du code retiré : 145 762 de ces lignes viennent du dossier `ffmpeg/` (binaire statique 7.0.2, pages de manuel, modèles VMAF) qui était commité dans le dépôt (audit, point 14). Le Dockerfile télécharge ffmpeg lui-même depuis une image figée par empreinte, donc le dépôt n'en a plus besoin. Le reste (~1 950 lignes) est l'ancien code Python et ses anciens .pyc. Retirer `ffmpeg/` n'allège pas l'historique git (le binaire reste dans les anciens commits) ; le réduire demanderait de réécrire l'historique, ce que cette PR ne fait pas.
