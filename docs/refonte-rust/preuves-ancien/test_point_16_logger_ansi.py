"""Point 16 : codes ANSI dans le fichier de log (formatter partagé qui modifie record.msg)."""

import glob


def test_fichier_de_log_sans_ansi():
    from utils.logger import console_logger

    console_logger.error("[DOWNLOAD] message de preuve ANSI")
    for h in console_logger.handlers:
        h.flush()
    contenu = ""
    for f in glob.glob("logs/*.log"):
        with open(f, encoding="utf-8") as fh:
            contenu += fh.read()
    assert "message de preuve ANSI" in contenu
    assert "\x1b" not in contenu, "séquences ANSI écrites dans le fichier de log"
