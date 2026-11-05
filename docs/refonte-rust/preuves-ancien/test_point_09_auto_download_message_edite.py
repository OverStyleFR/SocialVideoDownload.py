"""Point 9 : auto_download sur un message édité (update.message is None)."""

from unittest.mock import MagicMock, patch


def test_message_edite_ne_leve_pas(mk):
    import commands.auto_download as ad

    update = MagicMock()
    update.message = None
    update.edited_message.text = "voir https://example.com/v"
    with patch.object(ad, "download_command"):
        ad.auto_download(update, mk[1]())  # BON : ignoré ou traité, jamais d'exception
