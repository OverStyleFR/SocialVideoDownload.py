# Preuves rouges sur l'ancien bot Python (a467a68)

Chaque test ASSERTE LE BON COMPORTEMENT ; il échoue sur l'ancien code (ROUGE) et devra passer (VERT) sur le bot Rust.
Aucun réseau ni token : Telegram, pycurl, yt-dlp et ffmpeg sont simulés. Les IDs sont fictifs (123456789).

## Rejouer
```
git -C <dépôt> worktree add <dossier>/svd-ancien a467a68
python -m venv <dossier>/venv
<dossier>/venv/Scripts/python -m pip install -r <dossier>/svd-ancien/requirements.txt pytest "setuptools<81"
SVD_ANCIEN=<dossier>/svd-ancien <dossier>/venv/Scripts/python -W ignore -m pytest -v -p no:cacheprovider --tb=short docs/refonte-rust/preuves-ancien
```
(`setuptools<81` : PTB 13.7 / APScheduler 3.6.3 importent `pkg_resources`. Linux : `venv/bin/python`.)
Résultat brut enregistré : `RESULTAT-ancien.txt` (17 FAILED, 0 passé, 11 fichiers de test).

## Tableau point -> test -> résultat (attendu ROUGE = obtenu ROUGE)
| Point | Test | Attendu | Obtenu |
|---|---|---|---|
| 1 | test_point_01 `test_second_message_traite_pendant_le_premier` (vrai Dispatcher PTB, main() réel) | ROUGE | ROUGE : `[1] == [1, 2]` |
| 1 | test_point_01 `test_handlers_declares_run_async` | ROUGE | ROUGE : 6 handlers sans run_async |
| 2 | test_point_02 `test_options_ytdlp_bornees` | ROUGE | ROUGE : `noplaylist` absent |
| 2 | test_point_02 `test_handlers_filtres_par_utilisateur` | ROUGE | ROUGE : `download` sans filtre utilisateur |
| 2 | test_point_02 `test_hote_interne_refuse_avant_ytdlp` | ROUGE | ROUGE : yt-dlp appelé sur 127.0.0.1 |
| 3 | test_point_03 `test_fichier_en_cours_survit_au_nettoyage_d_urgence` | ROUGE | ROUGE : `.part` supprimé |
| 4 | test_point_04 `test_reponse_500_n_est_pas_une_url` | ROUGE | ROUGE : HTML 500 renvoyé comme URL |
| 4 | test_point_04 `test_timeout_pycurl_defini` | ROUGE | ROUGE : aucun timeout |
| 5 | test_point_05 `test_avertissement_avant_envoi_tiers` | ROUGE | ROUGE : seul « Upload externe en cours » |
| 8 | test_point_08 `test_mp3_partiel_non_reutilise` | ROUGE | ROUGE : mp3 partiel envoyé |
| 9 | test_point_09 `test_message_edite_ne_leve_pas` | ROUGE | ROUGE : AttributeError `'NoneType'.text` |
| 10 | test_point_10 `test_noms_distincts_pour_meme_titre` | ROUGE | ROUGE : collision `Meme titre.mp4` |
| 11 | test_point_11 `test_pseudo_seul_ne_donne_pas_acces` | ROUGE | ROUGE : rapport servi au pseudo seul |
| 11 | test_point_11 `test_nom_de_fichier_echappe_dans_markdown` | ROUGE | ROUGE : nom brut en `parse_mode=Markdown` |
| 15 | test_point_15 `test_is_cache_valid_est_appelee` | ROUGE | ROUGE : aucun appel |
| 15 | test_point_15 `test_token_loader_sans_nom_indefini` | ROUGE | ROUGE : `env_file` indéfini |
| 16 | test_point_16 `test_fichier_de_log_sans_ansi` | ROUGE | ROUGE : `\x1b` dans le fichier |
| 6,7,12,13,14,17,18 | `constats-statiques.md` (commandes git/grep rejouables) | constat | constat |

## Contrôle VERT
Test du point 1 rejoué sur une copie de l'ancien code où chaque handler reçoit `run_async=True` : 2 passés. Les autres tests n'ont pas de contre-épreuve VERT ici (elle viendra avec le code Rust).

## Limites
- Point 2 (SSRF) : le test impose le refus de 127.0.0.1 ; les autres hôtes internes (plages privées, métadonnées cloud) sont à couvrir côté Rust.
- Point 2 (filtre utilisateur) : le test cherche `Filters.user` de PTB 13.7 ; le test Rust équivalent devra viser son propre mécanisme.
- Point 11b : le nom est injecté via `retention_stats` (le caractère `*` est interdit dans un nom de fichier Windows).
- Point 16 : sur 3.13/Windows les logs sont lus dans un cwd temporaire.
