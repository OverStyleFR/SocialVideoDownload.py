"""Preuves rouges sur l'ancien bot Python (a467a68).

SVD_ANCIEN = chemin du checkout a467a68 (git worktree). Aucun réseau, aucun token réel :
Telegram / pycurl / yt-dlp / ffmpeg sont simulés. Le cwd est un dossier temporaire
(le code ancien écrit downloads/ et logs/ en chemin relatif).
"""

import os
import shutil
import sys
import tempfile
from unittest.mock import MagicMock

import pytest

ANCIEN = os.environ.get("SVD_ANCIEN")
if not ANCIEN or not os.path.isfile(os.path.join(ANCIEN, "main.py")):
    pytest.exit(
        "Définir SVD_ANCIEN = checkout de a467a68 (voir README.md)", returncode=2
    )
ANCIEN = os.path.abspath(ANCIEN)
sys.path.insert(0, ANCIEN)

# cwd isolé AVANT tout import de l'ancien code (utils.logger crée logs/ au chargement)
WORK = tempfile.mkdtemp(prefix="svd-preuves-")
os.chdir(WORK)
os.environ.pop("BOT_TOKEN", None)


@pytest.fixture(autouse=True)
def dossier_propre():
    dl = os.path.join(WORK, "downloads")
    assert dl.startswith(tempfile.gettempdir())
    shutil.rmtree(dl, ignore_errors=True)
    os.makedirs(dl)
    yield dl


@pytest.fixture
def ancien():
    return ANCIEN


def fake_update(
    user_id=123456789,
    username="testeur",
    text="/download https://example.com/v",
    chat_id=42,
):
    u = MagicMock()
    u.message.from_user.id = user_id
    u.message.from_user.username = username
    u.message.chat_id = chat_id
    u.message.text = text
    u.message.message_id = 7
    return u


def fake_context(args=None):
    c = MagicMock()
    c.args = args
    return c


@pytest.fixture
def mk():
    return fake_update, fake_context
