"""Point 3 : check_and_clean_if_needed -> clear_downloads supprime un téléchargement en cours."""

import os
from unittest.mock import patch


def test_fichier_en_cours_survit_au_nettoyage_d_urgence(dossier_propre):
    import utils.disk_manager as dm

    en_cours = os.path.join(dossier_propre, "video_autre_user.mp4.part")
    with open(en_cours, "wb") as f:
        f.write(b"x" * 1024)
    with patch.object(dm, "get_free_space_mb", return_value=0):
        dm.check_and_clean_if_needed()
    assert os.path.exists(en_cours), (
        "téléchargement en cours d'un autre utilisateur supprimé"
    )
