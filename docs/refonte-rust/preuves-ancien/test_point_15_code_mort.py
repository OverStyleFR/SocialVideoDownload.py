"""Point 15 : is_cache_valid jamais appelée ; code inatteignable dans token_loader."""

import ast
import os


def test_is_cache_valid_est_appelee(ancien):
    appels = []
    for racine, dirs, fichiers in os.walk(ancien):
        dirs[:] = [d for d in dirs if d not in ("ffmpeg", ".git")]
        for f in fichiers:
            if f.endswith(".py"):
                with open(os.path.join(racine, f), encoding="utf-8") as fh:
                    tree = ast.parse(fh.read())
                for n in ast.walk(tree):
                    if (
                        isinstance(n, ast.Call)
                        and getattr(n.func, "id", getattr(n.func, "attr", ""))
                        == "is_cache_valid"
                    ):
                        appels.append(f)
    assert appels, "is_cache_valid définie mais jamais appelée"


def test_token_loader_sans_nom_indefini(ancien):
    with open(os.path.join(ancien, "utils", "token_loader.py"), encoding="utf-8") as fh:
        tree = ast.parse(fh.read())
    fn = next(
        n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "get_token"
    )
    assigned = {
        t.id
        for n in ast.walk(fn)
        if isinstance(n, ast.Assign)
        for t in n.targets
        if isinstance(t, ast.Name)
    }
    used = {
        n.id
        for n in ast.walk(fn)
        if isinstance(n, ast.Name) and isinstance(n.ctx, ast.Load)
    }
    assert "env_file" not in (used - assigned), (
        "env_file utilisé sans définition (code après exit(1))"
    )
