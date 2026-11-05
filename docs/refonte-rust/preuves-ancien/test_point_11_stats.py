"""Point 11 : /stats autorisé par pseudo ; nom de fichier brut dans du Markdown."""



def test_pseudo_seul_ne_donne_pas_acces(mk):
    import commands.stats as s

    update = mk[0](user_id=123456789, username=s.AUTHORIZED_USER.upper())
    s.stats(update, mk[1]())
    texte = update.message.reply_text.call_args[0][0]
    assert "refus" in texte.lower(), (
        "pseudo (reprenable) accepté comme preuve d'identité"
    )


def test_nom_de_fichier_echappe_dans_markdown(mk, monkeypatch):
    import commands.stats as s

    nom = "a_b*c[d.mp4"
    # `*` est interdit dans un nom Windows : on injecte le nom via retention_stats (Linux : fichier réel)
    rs = dict(
        total_files=1,
        total_size=1,
        next_file=nom,
        next_remaining_min=10,
        freed_2h=1,
        freed_24h=1,
        count_2h=1,
        count_24h=1,
    )
    monkeypatch.setattr(s, "retention_stats", lambda: rs)
    update = mk[0](user_id=next(iter(s.AUTHORIZED_IDS)), username="autre")
    s.stats(update, mk[1]())
    args, kw = update.message.reply_text.call_args
    assert args[0].startswith("📊"), "précondition : /stats a produit le rapport"
    assert not (nom in args[0] and kw.get("parse_mode") == "Markdown"), (
        "nom brut injecté dans un message parse_mode=Markdown (envoi cassé par _ * [)"
    )
