# Journal de la refonte Rust : point par point

Branche `refonte-rust`, base `a467a68` (bot Python). Rien n'est poussé. Étiquettes : MESURÉ = sorti d'une commande rejouée ; DÉDUIT = lecture du code ou calcul.
Les preuves rouges sont dans `docs/refonte-rust/preuves-ancien/` (pytest sur l'ancien code, `RESULTAT-ancien.txt` : 17 échecs, 0 réussite) et `constats-statiques.md` (commandes git/grep rejouables). Les tests verts sont des tests Rust (à `33a067b` : 48 tests unitaires, 12 e2e dans `tests/e2e.rs` ; à `f8f27ce` : 114, voir « Portes rejouées »).

## Ce que Rust ne remplace pas, et limites connues

- **yt-dlp et ffmpeg/ffprobe restent des binaires externes**, pilotés en sous-processus (`src/download.rs`, `src/media.rs`). Rust ne les accélère pas : débit et latence en dépendent (voir `STATS.md`, section Limites : le Rust n'est pas plus rapide sur ce banc, p50 +41,3 %, débit -7,4 %, MESURÉ).
- Marqueurs `shortcut:` du code (`git grep -n 'shortcut:'`) :
  - `src/cache.rs:18` : cache en mémoire seule, pas de persistance disque ;
  - `src/retention.rs:15` : l'espace disque libre est estimé (la std ne l'expose pas) ;
  - `src/url_guard.rs:11` : la résolution faite ensuite par yt-dlp peut différer de la nôtre (DNS rebinding).
- **Mise à jour de yt-dlp et nettoyage périodique** : non branchés jusqu'à `33a067b` (bug SOCIAL-2611050351-k8h), branchés par `4f88299` (`bot::spawn_background`, `src/bot/mod.rs:80-81`, appelé par `src/main.rs`) ; tests `tests/startup.rs`. MESURÉ : `git grep -n 'spawn_janitor\|spawn_updater' src` renvoie les deux appels de `src/bot/mod.rs` et les deux définitions.
- Le banc de latence a été mesuré avec un binaire patché d'un caractère (bug SOCIAL-2611050403-f13, voir `STATS.md`).
- Les e2e tournent avec un faux Telegram, un faux yt-dlp et un faux ffmpeg : seul le chemin `.cmd` (Windows) a été exécuté, le chemin `.sh` ne l'a pas été. La compression réelle n'est pas exercée en e2e (test unitaire `media::tests::bout_en_bout_sonde_compression_mp3`).
- Le workflow de déploiement de l'ancien bot est toujours suivi par git (un hook a bloqué sa suppression) ; sa suppression reste à faire.
- Le commit `37dd146` (e2e) est **rouge pris seul** : les e2e ont été ajoutés avant leur correctif `2850bf0` (le consentement gardait un `JobDir` déjà supprimé). L'historique n'est **pas réécrit** : choix assumé (branche partagée en lecture, hook de refus de recul). Pour `git bisect`, sauter `37dd146` (`git bisect skip 37dd146`).
- `docs/refonte-rust/preuves-ancien/RESULTAT-ancien.txt` contient 39 lignes avec le chemin local du poste (`C:\Users\<nom>\...`) : pas un secret, mais le nom d'utilisateur du poste est publié (bug SOCIAL-2611050551-izv, voir « Recherche de secrets »).
- `actionlint` signale le libellé de runner `nas` comme inconnu : c'est un runner auto-hébergé, sans fichier `actionlint.yaml` pour le déclarer.

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
| 62207c1 | docs(refonte): fiche et journal point par point | INGEST/FICHE |
| 5329449 | docs(refonte): chiffres de tests dans STATS.md, étiquettes dans la fiche | |
| 3b6d3a7 | docs(refonte): fiche sans chiffre absent de STATS.md | |
| 767ba1c | fix(download): filtre de durée tolérant aux durées absentes (duration <=?) | bug f13, point 2 (arguments yt-dlp) |
| a209aa1 | fix: masquer le jeton Telegram dans tous les journaux + test token_redaction | point 18 |
| 4f88299 | fix: lancer le janitor et la mise à jour yt-dlp au démarrage (spawn_background) | points 3, 7 (bug k8h) |
| 670ad32 | feat(providers): trait Provider (enum), routeur et providers X, Bluesky, TikTok | hors 18 points |
| ca7de8e | feat(settings): module de réglages typés, stockage atomique, audit et menus | hors 18 points |
| 922f77f | feat(download): routeur hybride branché + tests/e2e_providers.rs | hors 18 points |
| c0acf47 | test(token): journaux isolés par test, 500 via getUpdates, panique getMe vérifiée | point 18 |
| 43390f2 | feat(bot): /settings câblé + tests/e2e_settings.rs | hors 18 points |
| 3c537a9 | docs: README/DOCS/FICHE alignés sur /settings et providers | point 17 |
| f8f27ce | test(bot): raccord /settings -> providers, e2e activation de X par bouton | hors 18 points |
| 1258793 | docs(stats): re-mesure au commit f8f27ce (lignes, dépendances, tests, image) | STATS.md (documentation seule, les portes ci-dessous restent valables) |

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
- Branché au démarrage par `4f88299` (bug SOCIAL-2611050351-k8h corrigé). VERT : `b_updater_calls_ytdlp_only_when_enabled` (`tests/startup.rs`), rouge si `spawn_background` ne lance rien (voir tableau).
- Commits : 80f65b8, 444c9f6, 4f88299.

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
- Passe 2 : jeton Telegram masqué dans tous les journaux (`a209aa1`, `c0acf47`, `src/telemetry.rs` writer `RedactWriter`) ; VERT `tests/token_redaction.rs` (4 tests).

## Tableau des 18 points (rejoué le 05/11/2026 sur HEAD `f8f27ce`)

ROUGE ancien = `pytest docs/refonte-rust/preuves-ancien` rejoué sur un worktree temporaire de `a467a68` (commande du README des preuves) : **17 échoués, 0 passé**, MESURÉ. VERT = `cargo test --no-fail-fast` sur HEAD : **114 passés, 0 échoué**, MESURÉ ; chaque nom cité ci-dessous figure en `... ok` dans cette sortie. Les rouges « mutation » ont été rejoués dans un worktree temporaire de HEAD (rien modifié dans l'arbre de travail), correctif retiré à la main puis restauré (`git checkout`).

| Point | Commit(s) | Test rouge-avant / vert-après (nom exact, rejoué) ou justification | Résultat |
|---|---|---|---|
| 1 | 74ade16, 37dd146 | ROUGE `test_point_01::test_second_message_traite_pendant_le_premier`, `test_handlers_declares_run_async` (a467a68) / VERT e2e `b_slow_user_does_not_block_other` | CONFORME |
| 2 | e17c411, ffd0870, 80f65b8, d323b6b, 767ba1c | ROUGE `test_point_02::test_options_ytdlp_bornees`, `test_handlers_filtres_par_utilisateur`, `test_hote_interne_refuse_avant_ytdlp` / VERT `url_guard::tests::refuses_internal_targets`, `refuses_bad_scheme_and_credentials`, `access::tests::user_outside_list_refused`, `rate_limit_at_n_plus_1_then_reset_after_window`, `download::tests::download_args_et_url_apres_double_tiret`, e2e `c_stranger_refused_without_ytdlp`, `d_ssrf_refused`. Filtre de durée (`767ba1c`) : mutation `<=?` → `<=` dans `src/download.rs:306` → `download_args_et_url_apres_double_tiret` FAILED (0 passé, 1 échoué) | CONFORME ; limite DNS rebinding (`shortcut:`) |
| 3 | e157f2f, 37dd146, 4f88299 | ROUGE `test_point_03::test_fichier_en_cours_survit_au_nettoyage_d_urgence` / VERT `retention::tests::retention_jobdir_vivant_survit`, `retention_manque_de_place_supprime_les_plus_anciens_pas_le_verrouille`, e2e `j_cleanup_spares_running_job`, `startup::a_janitor_removes_expired_job_dir`, `c_min_free_disk_reaches_cleanup`. Branchement (`4f88299`) : mutation `spawn_background` sans les deux appels (`src/bot/mod.rs:80-81`) → `tests/startup.rs` 0 passé, 3 échoués | CONFORME |
| 4 | b64a495 | ROUGE `test_point_04::test_reponse_500_n_est_pas_une_url`, `test_timeout_pycurl_defini` / VERT `egress::tests::egress_500_refuse`, `egress_html_refuse`, `egress_timeout_rapide`, `egress_200_url_ok`, e2e `f_external_500_gives_error_without_link` | CONFORME |
| 5 | b64a495, 74ade16, 2850bf0 | ROUGE `test_point_05::test_avertissement_avant_envoi_tiers` / VERT `egress::tests::consentement_nomme_hote`, `egress_desactive_sans_url`, `bot::handlers::tests::oversize_needs_consent_only_when_external_configured`, e2e `f_oversize_needs_consent_then_uploads` | CONFORME |
| 6 | d2177a1 | Justification : choix de dépendance (teloxide 0.17, reqwest rustls), pas de comportement à tester ; constat rouge dans `constats-statiques.md` ; surveillance par le job `audit` (cargo-audit) de la CI, qui n'a pas encore tourné | CONFORME sans test (justifié) |
| 7 | 80f65b8, 444c9f6, 4f88299 | ROUGE : pas de test Python (constat statique, yt-dlp installé au build) ; rouge par mutation `spawn_background` vide → `b_updater_calls_ytdlp_only_when_enabled` FAILED / VERT `startup::b_updater_calls_ytdlp_only_when_enabled` | CONFORME (k8h corrigé) ; la mise à jour réelle de yt-dlp (réseau) n'est pas exercée |
| 8 | ce51526, 80f65b8 | ROUGE `test_point_08::test_mp3_partiel_non_reutilise` / VERT `download::tests::download_audio_sans_video_entiere`, `media::tests::mp3_echec_ffmpeg_introuvable_ne_laisse_rien`, `mp3_echec_ffmpeg_reel_source_corrompue`, `garde_fou_audio`, e2e `h_music_sends_audio_and_failure_leaves_no_mp3` | CONFORME |
| 9 | 74ade16 | ROUGE `test_point_09::test_message_edite_ne_leve_pas` / VERT e2e `e_edited_message_is_processed` | CONFORME |
| 10 | 80f65b8 | ROUGE `test_point_10::test_noms_distincts_pour_meme_titre` / VERT `download::tests::download_deux_fetch_concurrents_fichiers_distincts` | CONFORME |
| 11 | ffd0870, 74ade16 | ROUGE `test_point_11::test_pseudo_seul_ne_donne_pas_acces`, `test_nom_de_fichier_echappe_dans_markdown` / VERT `access::tests::admin_by_numeric_id_only`, `bot::markdown::tests::escapes_every_reserved_character`, `escapes_file_name_and_keeps_plain_text`, `bot::stats::tests::file_names_and_values_are_escaped`, e2e `g_stats_admin_only_and_escaped` | CONFORME ; MarkdownV2 contrôlé par un vérificateur maison, pas par Telegram |
| 12 | 444c9f6 | Justification : configuration d'image (USER 10001, HEALTHCHECK, digests), vérifiée à la main lors du commit ; pas de test automatisé ni de build d'image rejoué ici | CONFORME sans test (justifié, non rejoué) |
| 13 | d586170 | Justification : un workflow ne se teste qu'en tournant ; rejoué `actionlint` : seul défaut, libellé de runner `nas` inconnu (auto-hébergé) | CONFORME sans test (justifié) ; aucun run réel |
| 14 | cabbd91 | Justification : contrôle par commande, rejoué : `git ls-files \| grep -cE '^ffmpeg/\|\.pyc$'` → 0 | CONFORME ; historique `.git` non allégé |
| 15 | be121cd, a8753e8, 7e5625a | ROUGE `test_point_15::test_is_cache_valid_est_appelee`, `test_token_loader_sans_nom_indefini` / VERT `cache::tests::hit_and_normalized_key`, `expires_after_ttl`, `evicts_oldest_at_capacity`, `video_differs_from_audio`, e2e `i_same_url_twice_uses_cache` ; code mort : porte clippy `-D warnings` rejouée, sortie 0 | CONFORME |
| 16 | d2177a1, 7e5625a | ROUGE `test_point_16::test_fichier_de_log_sans_ansi` / pas de VERT dédié : justification, tracing ne s'initialise qu'une fois par processus de test ; `.with_ansi(false)` lu dans `src/telemetry.rs` | CONFORME sans test vert (justifié) |
| 17 | 6f42b8a, 3c537a9 | Justification : documentation, pas de comportement ; constat rouge dans `constats-statiques.md` | CONFORME sans test (justifié) ; `.env.example` sans `SETTINGS_FILE`, AGENTS.md périmé (piège k8h) |
| 18 | 7e5625a, d323b6b, 6f42b8a, a209aa1, c0acf47 | ROUGE : constat statique (ID en dur, SSH vers une IP privée) ; jeton : mutation du writer de `src/telemetry.rs:123` sans `redact` → `tests/token_redaction.rs` 3 passés, 1 échoué (`erreur_reqwest_directe_ne_fuit_pas_le_jeton` : jeton `bot123456:FAKE-TOKEN-xyz` en clair dans le journal ; les deux autres restent verts sans le writer, le masquage amont `Cx::redact` les couvre déjà) / VERT `config::tests::config_liste_ids`, `config_id_non_numerique_refuse`, `url_guard::tests::refuses_internal_targets`, `token_redaction::connexion_refusee_ne_fuit_pas_le_jeton`, `reponse_500_ne_fuit_pas_le_jeton`, `erreur_reqwest_directe_ne_fuit_pas_le_jeton`, `redact_motif_sans_jeton_connu` ; recherche de secrets ci-dessous | PARTIEL : aucun secret, mais chemin local du poste publié (ÉCHEC mineur, bug izv) ; valeurs historiques encore dans git |

## Recherche de secrets dans `git diff a467a68..HEAD` (05/11/2026, MESURÉ)

Lignes ajoutées seulement (`git diff a467a68..HEAD | grep '^+'`), depuis la racine du dépôt.

| Recherche | Commande | Résultat |
|---|---|---|
| IP privées | `git diff a467a68..HEAD \| grep '^+' \| grep -E '(^\|[^0-9.])(10\.\|192\.168\.\|172\.(1[6-9]\|2[0-9]\|3[01])\.\|100\.(6[4-9]\|[7-9][0-9]\|1[01][0-9]\|12[0-7])\.)[0-9]'` | Code : `src/url_guard.rs:87,88,91` (`http://10.1.2.3/`, `http://172.31.0.5/`, `http://100.64.0.1/`) : vecteurs anti-SSRF génériques, une adresse quelconque par plage RFC 1918 / CGNAT, aucune machine désignée (`d323b6b` a retiré les deux adresses qui ressemblaient à celles du réseau). Faux positifs : `.env.example:5` `VERSION=V10.0.0` ; User-Agent `Windows NT 10.0` ; `tests/RESULTAT-mutations.txt` (commandes sed). |
| Nombres de 9-10 chiffres (ID Telegram) | `git diff a467a68..HEAD -- . ':!Cargo.lock' \| grep '^+' \| grep -E '(^\|[^0-9A-Za-z])[0-9]{9,10}([^0-9A-Za-z]\|$)'` | Tous factices ou non-ID : `111111111`, `222222222` (exemples, tests), `123456789` (preuves Python ; l'ID réel est masqué dans `constats-statiques.md`), `1700000000` (horodatage de fixture), `600000000` / `524288000` (octets du faux yt-dlp), `2130706433` / `3232235777` (127.0.0.1 et 192.168.1.1 en décimal, vecteurs SSRF), `SOCIAL-2611050351` / `SOCIAL-2611050403` (ID de bugs), `182466181` / `116510831` (octets d'image, STATS). `Cargo.lock` exclu : sommes sha256. |
| Jetons | `git diff a467a68..HEAD \| grep '^+' \| grep -E '[0-9]{8,10}:[A-Za-z0-9_-]{30,}\|ghp_\|sk-\|Bearer'` | aucun résultat. Seul jeton de forme Telegram : `123456789:AAFakeTokenSansReseau` (preuves Python), factice et trop court pour le motif. |
| Chemins `C:/Users`, `C:\Users` | `git diff a467a68..HEAD \| grep '^+' \| grep -iE 'C:/Users\|C:\\\\Users'` puis `git grep -n glenn HEAD` | **ÉCHEC mineur** : 39 lignes, toutes dans `docs/refonte-rust/preuves-ancien/RESULTAT-ancien.txt` (sortie brute de pytest : chemins du venv, du dépôt, du dossier temporaire). Pas de secret, mais le nom d'utilisateur du poste est publié. Bug SOCIAL-2611050551-izv ; correctif hors de cette tâche (remplacer par `<dépôt>` / `<tmp>`). |
| `ketopy` / `Toji` dans le code | `git diff a467a68..HEAD -- src tests Dockerfile docker-compose.yml \| grep '^+' \| grep -iE 'ketopy\|toji'` | aucun résultat. |

## Portes rejouées (05/11/2026, HEAD `f8f27ce`, MESURÉ)

| Porte | Commande | Sortie résumée |
|---|---|---|
| Format | `cargo fmt --check` | sortie 0 |
| Lint | `cargo clippy --all-targets -- -D warnings` | sortie 0 ; seul avis : future-incompat de la dépendance `proc-macro-error2 v2.0.1` |
| Tests | `cargo test --no-fail-fast` | 114 passés, 0 échoué : lib 83, `e2e` 12, `e2e_providers` 5, `e2e_settings` 7, `startup` 3, `token_redaction` 4 |
| Tâches de fond | `git grep -n 'spawn_janitor\|spawn_updater' src` | appels `src/bot/mod.rs:80`, `:81` ; définitions `src/download.rs:370`, `src/retention.rs:192` |
| Filtre de durée | `grep -nE 'duration <=?' src/download.rs` | `:306` `duration <=? {} & !is_live` ; `:542`, `:547` dans le test ; `:473` fixture imitant le message de yt-dlp |
| Workflow CI | `actionlint .github/workflows/ci-rust.yml` | 4 erreurs `runner-label` (libellé `nas`, l. 20, 32, 42, 62) ; avec `-ignore 'label "nas" is unknown'` : sortie 0 |
| Point 14 | `git ls-files \| grep -cE '^ffmpeg/\|\.pyc$'` | 0 |
