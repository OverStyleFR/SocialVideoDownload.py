"""Point 8 : un mp3 partiel laissé par un échec ffmpeg est renvoyé comme « déjà converti »."""

import os
from unittest.mock import patch


def test_mp3_partiel_non_reutilise(mk, dossier_propre):
    import commands.music as m

    video = os.path.join(dossier_propre, "clip.mp4")

    class Ydl:
        def __init__(self, *a, **k):
            pass

        def __enter__(self):
            return self

        def __exit__(self, *a):
            return False

        def extract_info(self, url, download=True):
            if download:
                with open(video, "wb") as f:
                    f.write(b"v" * 100)
            return {"id": "x"}

        def prepare_filename(self, info):
            return video

    def ffmpeg_en_echec(stream, cmd=None, quiet=None):
        with open(os.path.splitext(video)[0] + ".mp3", "wb") as f:
            f.write(b"ID3partiel")
        raise RuntimeError("ffmpeg interrompu")

    envoyes = []
    with (
        patch.object(m.yt_dlp, "YoutubeDL", Ydl),
        patch.object(m.ffmpeg, "run", ffmpeg_en_echec),
        patch.object(
            m, "upload_file", side_effect=lambda u, f, *a, **k: envoyes.append(f)
        ),
    ):
        for _ in range(2):  # 1re : échec de conversion ; 2e : même lien
            m.music(
                mk[0](text="/music https://example.com/v"),
                mk[1](["https://example.com/v"]),
            )
    assert not envoyes, f"mp3 partiel envoyé à l'utilisateur : {envoyes}"
