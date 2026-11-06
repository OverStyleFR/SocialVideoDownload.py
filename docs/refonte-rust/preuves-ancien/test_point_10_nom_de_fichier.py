"""Point 10 : outtmpl %(title)s -> deux vidéos de même titre se retrouvent sur le même fichier."""

import yt_dlp

from utils.ytdlp_config import DEFAULT_YDL_OPTS


def test_noms_distincts_pour_meme_titre():
    with yt_dlp.YoutubeDL({**DEFAULT_YDL_OPTS, "quiet": True}) as ydl:
        a = ydl.prepare_filename({"id": "AAA", "title": "Meme titre", "ext": "mp4"})
        b = ydl.prepare_filename({"id": "BBB", "title": "Meme titre", "ext": "mp4"})
    assert a != b, f"collision : {a}"
