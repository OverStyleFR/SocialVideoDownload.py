"""Point 1 : un téléchargement bloque tous les utilisateurs (aucun run_async).

Bon comportement : un second message est traité pendant que le premier télécharge.
On enregistre les handlers via le vrai main(), dans un vrai Dispatcher PTB 13.7.
"""

import queue
import threading
import time
from unittest.mock import MagicMock, patch

import telegram
from telegram.ext import Dispatcher


def _update(uid, texte="/download https://example.com/v"):
    bot = telegram.Bot("123456789:AAFakeTokenSansReseau")
    bot._bot = telegram.User(
        id=1, first_name="b", is_bot=True, username="testbot"
    )  # évite getMe (réseau)
    return telegram.Update.de_json(
        {
            "update_id": uid,
            "message": {
                "message_id": uid,
                "date": 1700000000,
                "chat": {"id": 42, "type": "private"},
                "from": {"id": 123456789, "is_bot": False, "first_name": "T"},
                "text": texte,
                "entities": [{"type": "bot_command", "offset": 0, "length": 9}],
            },
        },
        bot,
    )


def _dispatcher_avec_handlers(bloquant):
    import main

    bot = MagicMock()
    bot.defaults = None  # sinon MagicMock truthy => run_async implicite
    disp = Dispatcher(bot, queue.Queue(), workers=2, use_context=True)
    upd = MagicMock()
    upd.dispatcher = disp
    with (
        patch.object(main, "Updater", return_value=upd),
        patch.object(main, "get_token", return_value="x"),
        patch.object(main, "download", bloquant),
        patch.object(main, "clear_downloads"),
        patch.object(main.threading, "Thread"),
    ):
        main.main()
    return disp


def test_second_message_traite_pendant_le_premier():
    debut, libere, vus = threading.Event(), threading.Event(), []

    def bloquant(update, context):
        vus.append(update.update_id)
        if len(vus) == 1:
            debut.set()
            libere.wait(10)

    disp = _dispatcher_avec_handlers(bloquant)
    t = threading.Thread(target=disp.start, daemon=True)
    t.start()
    try:
        disp.update_queue.put(_update(1))
        assert debut.wait(5)
        disp.update_queue.put(_update(2))
        time.sleep(1.5)
        # BON : le 2e est déjà pris en charge alors que le 1er n'a pas fini
        assert vus == [1, 2], (
            f"2e message non traité tant que le 1er télécharge : {vus}"
        )
    finally:
        libere.set()
        disp.stop()


def test_handlers_declares_run_async():
    disp = _dispatcher_avec_handlers(lambda u, c: None)
    handlers = [h for g in disp.handlers.values() for h in g]
    assert handlers
    sans = [type(h).__name__ for h in handlers if not getattr(h, "run_async", False)]
    assert not sans, f"handlers sans run_async : {sans}"
