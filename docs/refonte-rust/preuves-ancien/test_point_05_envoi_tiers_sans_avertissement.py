"""Point 5 : > 35 Mo, le fichier part chez un tiers sans avertir l'utilisateur AVANT l'envoi."""

import re
import sys
import types
from unittest.mock import MagicMock


def test_avertissement_avant_envoi_tiers(mk, dossier_propre, monkeypatch):
    fp = types.ModuleType("pycurl")
    fp.error = Exception
    monkeypatch.setitem(sys.modules, "pycurl", fp)
    sys.modules.pop("utils.curl_uploader", None)
    import utils.curl_uploader as cu
    import utils.upload as up

    journal = []
    update, ctx = mk[0](), mk[1]()
    update.message.reply_text.side_effect = lambda *a, **k: (
        journal.append(("msg", a, k)) or MagicMock()
    )
    ctx.bot.edit_message_text.side_effect = lambda *a, **k: journal.append(
        ("msg", a, k)
    )
    monkeypatch.setattr(
        cu,
        "upload_large_file_via_curl",
        lambda *a, **k: (
            journal.append(("UPLOAD",)) or "https://curl.libriciel.fr/x.mp4"
        ),
    )

    gros = f"{dossier_propre}/gros.mp4"
    with open(gros, "wb") as f:
        f.truncate(40 * 1024 * 1024)
    up.upload_file(update, gros, ctx, progress_msg_id=1)

    idx = next(i for i, e in enumerate(journal) if e[0] == "UPLOAD")
    avant = " ".join(str(e) for e in journal[:idx])
    assert re.search(r"tiers|libriciel|third", avant, re.I), (
        f"aucun avertissement avant l'envoi : {avant!r}"
    )
