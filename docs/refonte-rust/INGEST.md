# Journal de la refonte Rust : point par point

Branche `refonte-rust`, base `a467a68` (bot Python). Rien n'est poussé. Étiquettes : MESURÉ = sorti d'une commande rejouée ; DÉDUIT = lecture du code ou calcul.
Les preuves rouges sont dans `docs/refonte-rust/preuves-ancien/` (pytest sur l'ancien code, `RESULTAT-ancien.txt` : 17 échecs, 0 réussite) et `constats-statiques.md` (commandes git/grep rejouables). Les tests verts sont des tests Rust (`cargo test -- --list` : 48 tests unitaires, 12 e2e dans `tests/e2e.rs`).

## Ce que Rust ne remplace pas, et limites connues

- **yt-dlp et ffmpeg/ffprobe restent des binaires externes**, pilotés en sous-processus (`src/download.rs`, `src/media.rs`). Rust ne les accélère pas : débit et latence en dépendent (voir `STATS.md`, section Limites : le Rust n'est pas plus rapide sur ce banc, p50 +41,3 %, débit -7,4 %, MESURÉ).
- Marqueurs `shortcut:` du code (`git grep -n 'shortcut:'`) :
  - `src/cache.rs:18` : cache en mémoire seule, pas de persistance disque ;
  - `src/retention.rs:15` : l'espace disque libre est estimé (la std ne l'expose pas) ;
  - `src/url_guard.rs:11` : la résolution faite ensuite par yt-dlp peut différer de la nôtre (DNS rebinding).
- **Mise à jour de yt-dlp et nettoyage périodique non branchés** : `spawn_updater` (`src/download.rs:211`) et `spawn_janitor` (`src/retention.rs:190`) existent mais ne sont appelés nulle part (bug déclaré SOCIAL-2611050351-k8h). Conséquence : `YTDLP_UPDATE_INTERVAL_H`, `RETENTION_HOURS` et `MIN_FREE_DISK_MB` n'ont pas d'effet à l'exécution. Le point 7 est donc seulement partiellement traité.
- Le banc de latence a été mesuré avec un binaire patché d'un caractère (bug SOCIAL-2611050403-f13, voir `STATS.md`).
- Les e2e tournent avec un faux Telegram, un faux yt-dlp et un faux ffmpeg : seul le chemin `.cmd` (Windows) a été exécuté, le chemin `.sh` ne l'a pas été. La compression réelle n'est pas exercée en e2e (test unitaire `media::tests::bout_en_bout_sonde_compression_mp3`).
- Le workflow de déploiement de l'ancien bot est toujours suivi par git (un hook a bloqué sa suppression) ; sa suppression reste à faire.
- Le commit `37dd146` (test e2e) est rouge à lui seul : son correctif est dans `2850bf0`. Gênant pour `git bisect`.

## Inspiration

L'architecture s'inspire d'un bot Discord interne (non nommé) : une couche provider par plateforme (`src/providers/`), compression sous la limite d'envoi (`src/media.rs`), garde-fou audio, cache de résultats (`src/cache.rs`), jamais de silence (toute erreur répond à l'utilisateur, test e2e `k_errors_are_never_silent`), et un faux serveur de messagerie pour les tests (`tests/fake_telegram/mod.rs`). Tout est réécrit à neuf ; aucun code n'a été repris.

## Journal chronologique des commits (a467a68..HEAD)

| Hash | Message | Rôle |
|---|---|---|
| d2177a1 | feat(rust): squelette du crate | config typée, erreurs, télémétrie, healthcheck, signatures |
| 48323c9 | test(preuves): reproduction des défauts sur a467a68 | 17 tests rouges + constats statiques |
| cabbd91 | chore: sortir ffmpeg/ et les .pyc du suivi git | point 14 |
| 15aaff5 | bench: banc commun et mesures avant | mesures de l'ancien bot |
| e17c411 | feat(url): garde anti-SSRF et providers | point 2 |
| 444c9f6 | build: Dockerfile multi-étapes épinglé, non-root, compose durci | point 12 |
| d586170 | ci: workflow ci-rust épinglé par SHA | point 13 |
| 80f65b8 | feat(download): yt-dlp en sous-processus avec limites | points 2, 7, 10 |
| ce51526 | feat(media): sonde ffprobe, compression, mp3 atomique | point 8 |
| ffd0870 | feat(access): liste blanche par ID et limite de débit | points 2, 11 |
| be121cd | feat(cache): cache unique borné avec TTL | point 15 |
| 74ade16 | feat(bot): handlers teloxide concurrents, messages édités, /stats par ID | points 1, 9, 11 |
| b64a495 | feat(egress): repli externe à statut vérifié, timeouts, consentement | points 4, 5 |
| e157f2f | feat(retention): nettoyage qui épargne les tâches en cours | point 3 |
| 9914b46 | test(download): le timeout tue le processus | test supplémentaire |
| 37dd146 | test(e2e): faux Telegram et parcours réels | 12 e2e |
| 2850bf0 | fix(bot): le consentement garde le JobDir vivant jusqu'à l'envoi | bug trouvé par les e2e |
| a8753e8 | chore(lib): retrait du allow(dead_code) | point 15 |
| d323b6b | test(url): vecteurs SSRF sans adresse privée littérale | point 18 |
| 7e5625a | chore: retrait de l'ancien bot Python | points 15, 16, 18 |
| 6f42b8a | docs: documentation alignée sur le bot Rust | points 17, 18 |
| 3c66adc | docs(stats): mesures avant/après et graphes | STATS.md |
| cb1c0c3 | docs(bench): continuations de commande du README | |
| e5d8822 | docs(stats): retire un run non consigné | |
| 33a067b | docs(stats): écart de la re-mesure sur une seule base (11,3 % au plus) | |

Mutations des e2e (`tests/RESULTAT-mutations.txt`) : 8 mutations, 7 rendent leur test rouge ; M7 (verrou ignoré) restait verte, le test `j_cleanup_spares_running_job` a été renforcé et M8 (même mutation) le rend rouge.

## Points d'audit

Les noms de tests sont ceux de `cargo test -- --list`. Les preuves rouges sont des fichiers de `docs/refonte-rust/preuves-ancien/`.

### Point 1 : un téléchargement bloque tous les utilisateurs
- Constat : aucun `run_async` (PTB 13.7 synchrone).
- ROUGE : `test_point_01_pas_de_run_async.py` ; extrait : `assert [1] == [1, 2]` (« 2e message non traité tant que le 1er télécharge ») et « handlers sans run_async : [6 handlers] ».
- Correctif : handlers teloxide sur tokio, une tâche par message (`src/bot/mod.rs`, `src/bot/handlers.rs`).
- VERT : `b_slow_user_does_not_block_other` (tests/e2e.rs).
- Commits : 74ade16, 37dd146.

### Point 2 : aucun contrôle d'accès ni limite, SSRF
- Constat : pas de `noplaylist`, `max_filesize`, liste d'utilisateurs, débit max ; SSRF via extracteur générique.
- ROUGE : `test_point_02_options_et_acces.py` (3 tests) ; extraits : `assert None is True` sur `noplaylist`, « download accessible à tout utilisateur Telegram », « yt-dlp appelé sur un hôte de boucle locale (SSRF) ». Limite du test rouge : le SSRF ne couvre que la boucle locale.
- Correctif : `src/url_guard.rs` (schéma, identifiants, hôtes internes), `src/access.rs` (liste blanche, débit), arguments yt-dlp bornés dans `src/download.rs`.
- VERT : `url_guard::tests::refuses_internal_targets`, `url_guard::tests::refuses_bad_scheme_and_credentials`, `access::tests::user_outside_list_refused`, `access::tests::rate_limit_at_n_plus_1_then_reset_after_window`, `download::tests::download_args_et_url_apres_double_tiret`, e2e `c_stranger_refused_without_ytdlp`, `d_ssrf_refused`.
- Commits : e17c411, ffd0870, 80f65b8. Limite : DNS rebinding (`shortcut:` url_guard.rs:11).

### Point 3 : le nettoyage supprime les téléchargements en cours
- ROUGE : `test_point_03_nettoyage_supprime_en_cours.py` ; extrait : « téléchargement en cours d'un autre utilisateur supprimé ».
- Correctif : dossier de travail par tâche (`JobDir`) verrouillé dans un registre, épargné par le nettoyage (`src/retention.rs`).
- VERT : `retention::tests::retention_jobdir_vivant_survit`, `retention::tests::retention_manque_de_place_supprime_les_plus_anciens_pas_le_verrouille`, e2e `j_cleanup_spares_running_job` (mutation M8 rouge).
- Commits : e157f2f, 37dd146. Limite : le nettoyage périodique n'est pas branché (bug k8h).

### Point 4 : `curl_uploader` renvoie un 4xx/5xx comme URL, sans timeout
- ROUGE : `test_point_04_curl_uploader.py` ; extraits : « réponse 500 renvoyée comme URL de partage », « aucun timeout pycurl ».
- Correctif : `src/egress.rs` (statut vérifié, réponse HTML refusée, timeouts).
- VERT : `egress::tests::egress_500_refuse`, `egress::tests::egress_html_refuse`, `egress::tests::egress_timeout_rapide`, `egress::tests::egress_200_url_ok`, e2e `f_external_500_gives_error_without_link`.
- Commit : b64a495.

### Point 5 : envoi chez un tiers sans avertissement
- ROUGE : `test_point_05_envoi_tiers_sans_avertissement.py` ; extrait : « aucun avertissement avant l'envoi ».
- Correctif : consentement Oui/Non nommant l'hôte avant tout envoi externe ; repli désactivé sans URL configurée.
- VERT : `egress::tests::consentement_nomme_hote`, `egress::tests::egress_desactive_sans_url`, `bot::handlers::tests::oversize_needs_consent_only_when_external_configured`, e2e `f_oversize_needs_consent_then_uploads`.
- Commits : b64a495, 74ade16, 2850bf0 (correctif : le fichier était supprimé avant l'envoi).

### Point 6 : PTB 13.7 en fin de vie, urllib3<2
- Constat : `constats-statiques.md` (requirements : `python-telegram-bot==13.7`, `urllib3>=1.26,<2`).
- Pas de test dédié : justification, c'est un choix de dépendance. Correctif : `teloxide 0.17` + `reqwest 0.12` en rustls (`Cargo.toml`), surveillé par le job `audit` de la CI (cargo-audit), sans test.
- Commit : d2177a1.

### Point 7 : yt-dlp installé au build seulement
- Constat statique : `constats-statiques.md`.
- Correctif partiel : yt-dlp épinglé et vérifié par sha256 dans l'image (`Dockerfile`), fonction `spawn_updater` (`src/download.rs:211`, `yt-dlp --update-to stable`).
- Pas de test vert : `spawn_updater` n'est appelé nulle part (bug SOCIAL-2611050351-k8h). Point NON résolu à l'exécution ; reste à brancher.
- Commits : 80f65b8, 444c9f6.

### Point 8 : /music télécharge la vidéo entière, mp3 partiel réutilisé
- ROUGE : `test_point_08_music_mp3_partiel.py` ; extrait : « mp3 partiel envoyé à l'utilisateur ».
- Correctif : téléchargement audio seul, conversion mp3 atomique sans résidu (`src/download.rs`, `src/media.rs`).
- VERT : `download::tests::download_audio_sans_video_entiere`, `media::tests::mp3_echec_ffmpeg_introuvable_ne_laisse_rien`, `media::tests::mp3_echec_ffmpeg_reel_source_corrompue`, `media::tests::garde_fou_audio`, e2e `h_music_sends_audio_and_failure_leaves_no_mp3`.
- Commits : ce51526, 80f65b8.

### Point 9 : message édité = exception
- ROUGE : `test_point_09_auto_download_message_edite.py` ; extrait : `'NoneType' object has no attribute 'text'`.
- Correctif : filtre `edited_message` traité comme un message (`src/bot/mod.rs`).
- VERT : e2e `e_edited_message_is_processed` (mutation M1 rouge).
- Commit : 74ade16.

### Point 10 : collisions de nom de fichier
- ROUGE : `test_point_10_nom_de_fichier.py` ; extrait : `assert 'downloads\Meme titre.mp4' != 'downloads\Meme titre.mp4'`.
- Correctif : nom basé sur l'identifiant, dossier par tâche (`src/download.rs`).
- VERT : `download::tests::download_deux_fetch_concurrents_fichiers_distincts`.
- Commit : 80f65b8.

### Point 11 : /stats par pseudo, nom injecté dans le Markdown
- ROUGE : `test_point_11_stats.py` (2 tests) ; extraits : « pseudo (reprenable) accepté comme preuve d'identité », « nom brut injecté dans un message parse_mode=Markdown ». Le nom piégé passe par `retention_stats` (le `*` est interdit dans un nom Windows).
- Correctif : administrateur par ID numérique seulement (`src/access.rs`), échappement MarkdownV2 (`src/bot/markdown.rs`, `src/bot/stats.rs`).
- VERT : `access::tests::admin_by_numeric_id_only`, `bot::markdown::tests::escapes_every_reserved_character`, `bot::markdown::tests::escapes_file_name_and_keeps_plain_text`, `bot::stats::tests::file_names_and_values_are_escaped`, e2e `g_stats_admin_only_and_escaped` (mutation M6 rouge). Limite : sous Windows l'e2e utilise `a_b[c].mp4` ; la validité est vérifiée par un contrôleur maison, pas par le parseur de Telegram.
- Commits : ffd0870, 74ade16.

### Point 12 : conteneur root, sans healthcheck ni limites, ffmpeg `:latest`
- Constat : `constats-statiques.md` (aucune directive USER ni HEALTHCHECK).
- Correctif : `Dockerfile` (4 `FROM` épinglés par digest, `USER 10001`, `HEALTHCHECK`), `docker-compose.yml` (read_only, cap_drop ALL, mem_limit, pids_limit).
- Pas de test automatisé : vérifié à la main (image construite, `id -u` = 10001, healthcheck présent) ; `docker compose up` non lancé. Côté binaire : `--healthcheck` (`src/telemetry.rs`).
- Commit : 444c9f6.

### Point 13 : CI minimale
- Constat : `constats-statiques.md` (seul `python -c "import main"`).
- Correctif : `ci-rust.yml` dans le dossier des workflows (jobs `check`, `audit`, `image`, `publish`), actions épinglées par SHA, `publish` seulement sur push main.
- Pas de test : le workflow n'a pas tourné (aucun runner `[self-hosted, nas]` disponible ici), seulement lint et parse. Le premier run réel dira si `cargo audit` ou Trivy échouent.
- Commit : d586170.

### Point 14 : dépôt lourd, `.pyc` suivis
- Constat : 286 fichiers dont 236 sous `ffmpeg/`, 13 `.pyc`, `.gitignore` avec `.pyc` au lieu de `*.pyc`.
- Correctif : sortis de l'index, `.gitignore`/`.dockerignore` réécrits. Vérification MESURÉE : `git ls-files | grep -cE '^ffmpeg/|\.pyc$'` donne 0.
- Pas de test Rust : contrôle par commande. Le poids de `.git` (historique) n'est pas réduit : aucune réécriture d'historique.
- Commit : cabbd91.

### Point 15 : code mort, deux caches
- ROUGE : `test_point_15_code_mort.py` ; extraits : « is_cache_valid définie mais jamais appelée », « env_file utilisé sans définition (code après exit(1)) ».
- Correctif : un seul cache (`src/cache.rs`), `#![allow(dead_code)]` retiré (clippy `-D warnings` propre), ancien Python supprimé.
- VERT : `cache::tests::hit_and_normalized_key`, `cache::tests::expires_after_ttl`, `cache::tests::evicts_oldest_at_capacity`, `cache::tests::video_differs_from_audio`, e2e `i_same_url_twice_uses_cache` (mutation M4 rouge). L'absence de code mort est garantie par la porte clippy, non par un test.
- Commits : be121cd, a8753e8, 7e5625a.

### Point 16 : codes ANSI doublés dans les fichiers de log
- ROUGE : `test_point_16_logger_ansi.py` ; extrait : « séquences ANSI écrites dans le fichier de log ».
- Correctif : `tracing_subscriber` avec `.with_ansi(false)` (`src/telemetry.rs`), sortie texte ou JSON.
- Pas de test vert dédié : justification, l'initialisation globale de tracing ne se rejoue pas deux fois dans un processus de test. Vérifié par lecture de `src/telemetry.rs`.
- Commits : d2177a1, 7e5625a.

### Point 17 : docs en désaccord avec le code
- Constat : `constats-statiques.md` (`upload.py` inexistant, cache « chargé depuis le disque », étape « Run tests » absente, seuils 5 Mo vs 12 Mo). Le « 48 h » de l'audit n'a pas été retrouvé dans les `.md` (code : 24 h) : sous-constat NON confirmé.
- Correctif : `AGENTS.md`, `README.md`, `DOCS.md` réécrits sur le bot Rust (22 variables de `src/config.rs` listées). Pas de test : documentation.
- Commit : 6f42b8a.

### Point 18 : adresse d'infrastructure et ID Telegram publiés
- Constat : `constats-statiques.md` (commande SSH root vers une adresse privée dans AGENTS.md, ID Telegram en dur dans `commands/stats.py`).
- Correctif : ancien Python supprimé (ID en dur disparu), AGENTS.md réécrit, vecteurs de test sans adresse privée littérale ; les ID viennent de la configuration (`ALLOWED_USER_IDS`).
- VERT : `config::tests::config_liste_ids`, `config::tests::config_id_non_numerique_refuse`, `url_guard::tests::refuses_internal_targets`.
- Commits : 7e5625a, d323b6b, 6f42b8a. Les valeurs historiques restent dans l'historique git (non réécrit).
