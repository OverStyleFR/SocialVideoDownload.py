"""Point 2 : aucune limite yt-dlp, aucun filtre d'utilisateur, hôtes internes acceptés."""

from unittest.mock import MagicMock, patch

from telegram.ext import Filters


def test_options_ytdlp_bornees():
    from utils.ytdlp_config import DEFAULT_YDL_OPTS as o

    assert o.get("noplaylist") is True, "playlist entière téléchargée"
    assert o.get("max_filesize"), "aucune taille maximale"


def _filtres(f):
    yield f
    for attr in ("and_filter", "or_filter", "base_filter", "f"):
        sub = getattr(f, attr, None)
        if sub is not None:
            yield from _filtres(sub)


def test_handlers_filtres_par_utilisateur():
    import main

    upd = MagicMock()
    with (
        patch.object(main, "Updater", return_value=upd),
        patch.object(main, "get_token", return_value="x"),
        patch.object(main, "clear_downloads"),
        patch.object(main.threading, "Thread"),
    ):
        main.main()
    hs = [c.args[0] for c in upd.dispatcher.add_handler.call_args_list]
    assert hs
    for h in hs:
        if h.callback.__name__ in ("start", "help_command"):
            continue
        assert any(isinstance(x, Filters.user) for x in _filtres(h.filters)), (
            f"{h.callback.__name__} accessible à tout utilisateur Telegram"
        )


def test_hote_interne_refuse_avant_ytdlp(mk):
    import commands.download as dl

    update, ctx = mk[0](), mk[1](["http://127.0.0.1:8080/secret"])
    with (
        patch.object(dl, "check_and_clean_if_needed"),
        patch.object(dl.yt_dlp, "YoutubeDL") as ydl,
    ):
        dl.download(update, ctx)
    assert not ydl.called, "yt-dlp appelé sur un hôte de boucle locale (SSRF)"
