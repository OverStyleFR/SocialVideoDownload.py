#!/usr/bin/env python3
"""Genere les graphes SVG (et le bloc Mermaid) avant/apres a partir des JSON de mesures.

Stdlib seulement. Aucun chiffre n'est saisi ici : tout vient de
docs/refonte-rust/mesures/{avant,apres}.json. Sortie deterministe (relancer ne change rien).
    python bench/graphes.py            # ecrit docs/refonte-rust/graphes/*.svg et mermaid.md
    python bench/graphes.py --table    # affiche en plus le tableau Markdown des ecarts
"""

import json
import os
import sys
from xml.sax.saxutils import escape

ROOT = os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "..", "docs", "refonte-rust"
)
OUT = os.path.join(ROOT, "graphes")
AV = json.load(open(os.path.join(ROOT, "mesures", "avant.json"), encoding="utf-8"))
AP = json.load(open(os.path.join(ROOT, "mesures", "apres.json"), encoding="utf-8"))

# Palette Okabe-Ito (sure pour le daltonisme) ; avant = bleu, apres = orange ; texte en encre fonce.
C_AV, C_AP, INK, MUTED, GRID, BG = (
    "#0072B2",
    "#D55E00",
    "#1a1a1a",
    "#4a4a4a",
    "#d9d9d9",
    "#ffffff",
)

# (fichier, titre, unite, plus_petit_est_mieux, [(libelle, cle_avant, cle_apres)])
CHARTS = [
    (
        "lignes-de-code",
        "Lignes de code applicatif (hors tests)",
        "lignes",
        True,
        [("src/", "lignes_code_python", "lignes_code_rust_src")],
    ),
    (
        "dependances",
        "Dependances",
        "paquets",
        True,
        [
            ("directes", "dependances_directes", "dependances_directes"),
            (
                "transitives (arbre complet)",
                "dependances_totales_venv",
                "dependances_totales",
            ),
        ],
    ),
    (
        "image-docker",
        "Taille de l'image Docker",
        "Mio",
        True,
        [("image", "image_docker_octets", "image_docker_octets")],
    ),
    (
        "memoire-repos",
        "Memoire au repos (conteneur)",
        "Mio",
        True,
        [("RSS 30 s apres lancement", "memoire_repos_mib", "memoire_repos_mib")],
    ),
    (
        "demarrage",
        "Temps de demarrage",
        "s",
        True,
        [("lancement -> premier getUpdates", "demarrage_s", "demarrage_s")],
    ),
    (
        "latence",
        "Latence sous 10 telechargements simultanes",
        "s",
        True,
        [
            ("p50", "latence_p50_s", "latence_p50_s"),
            ("p95 (= max, 10 echantillons)", "latence_p95_s", "latence_p95_s"),
        ],
    ),
    (
        "debit",
        "Debit sous 10 telechargements simultanes",
        "Mo/s",
        False,
        [("debit", "debit_mo_par_s", "debit_mo_par_s")],
    ),
]


def val(d, k, unit):
    v = d[k]
    return v / 1048576 if k == "image_docker_octets" else v


def fmt(v):
    if abs(v) >= 100:
        return f"{v:,.0f}".replace(",", " ")
    return f"{v:.3f}".rstrip("0").rstrip(".") if v != int(v) else f"{int(v)}"


def ecart(a, b):
    return (b - a) / a * 100 if a else float("nan")


def svg(name, title, unit, lower, rows):
    W, PADL, PADR, TOP = 720, 220, 80, 64
    BH, GAP, GG = 22, 6, 22  # hauteur barre, ecart entre barres, ecart entre groupes
    data = [(lab, val(AV, ka, unit), val(AP, kb, unit)) for lab, ka, kb in rows]
    vmax = max(max(a, b) for _, a, b in data) or 1
    plot = W - PADL - PADR
    H = TOP + len(data) * (2 * BH + GAP + GG) + 40
    o = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" role="img" aria-labelledby="t d" font-family="system-ui,Segoe UI,Arial,sans-serif">',
        f'<title id="t">{escape(title)}</title>',
        f'<desc id="d">Avant (Python) contre apres (Rust), unite {escape(unit)}, {"plus petit = mieux" if lower else "plus grand = mieux"}.</desc>',
        f'<rect width="{W}" height="{H}" fill="{BG}"/>',
        f'<text x="16" y="26" font-size="16" font-weight="600" fill="{INK}">{escape(title)}</text>',
        f'<text x="16" y="46" font-size="12" fill="{MUTED}">{escape(unit)} - {"plus petit = mieux" if lower else "plus grand = mieux"}</text>',
        f'<rect x="{W - 230}" y="14" width="12" height="12" fill="{C_AV}"/><text x="{W - 212}" y="25" font-size="12" fill="{INK}">avant (Python)</text>',
        f'<rect x="{W - 110}" y="14" width="12" height="12" fill="{C_AP}"/><text x="{W - 92}" y="25" font-size="12" fill="{INK}">apres (Rust)</text>',
    ]
    for i in range(5):  # grille 0..vmax, meme echelle pour avant et apres
        x = PADL + plot * i / 4
        o.append(
            f'<line x1="{x:.1f}" y1="{TOP - 6}" x2="{x:.1f}" y2="{H - 30}" stroke="{GRID}" stroke-width="1"/>'
        )
        o.append(
            f'<text x="{x:.1f}" y="{H - 14}" font-size="11" text-anchor="middle" fill="{MUTED}">{fmt(vmax * i / 4)}</text>'
        )
    y = TOP
    for lab, a, b in data:
        o.append(
            f'<text x="{PADL - 10}" y="{y + BH + GAP / 2 + 4}" font-size="12" text-anchor="end" fill="{INK}">{escape(lab)}</text>'
        )
        for k, (v, c, tag) in enumerate(((a, C_AV, "avant"), (b, C_AP, "apres"))):
            w = max(plot * v / vmax, 1.5)
            yy = y + k * (BH + GAP)
            o.append(
                f'<rect x="{PADL}" y="{yy}" width="{w:.1f}" height="{BH}" rx="3" fill="{c}"><title>{tag} : {fmt(v)} {escape(unit)}</title></rect>'
            )
            o.append(
                f'<text x="{PADL + w + 6:.1f}" y="{yy + BH - 6}" font-size="12" fill="{INK}">{fmt(v)}</text>'
            )
        y += 2 * BH + GAP + GG
    o.append("</svg>")
    return "\n".join(o) + "\n"


def mermaid(name, title, unit, rows):
    cats = [lab.split(" (")[0] for lab, _, _ in rows]
    av = [fmt_m(val(AV, ka, unit)) for _, ka, _ in rows]
    ap = [fmt_m(val(AP, kb, unit)) for _, _, kb in rows]
    mx = max(float(x) for x in av + ap)
    q = lambda s: '"' + s.replace('"', "'") + '"'
    return (
        f"```mermaid\nxychart-beta\n    title {q(title + ' (' + unit + ')')}\n    x-axis [{', '.join(q(c) for c in cats)}]\n"
        f"    y-axis {q(unit)} 0 --> {fmt_m(mx * 1.1)}\n    bar [{', '.join(av)}]\n    bar [{', '.join(ap)}]\n```\n"
        "Premiere serie : avant (Python). Seconde serie : apres (Rust).\n"
    )


def fmt_m(v):
    return f"{v:.3f}".rstrip("0").rstrip(".")


def main():
    os.makedirs(OUT, exist_ok=True)
    md = ["# Blocs Mermaid (generes par bench/graphes.py, ne pas editer)\n"]
    table = []
    for name, title, unit, lower, rows in CHARTS:
        with open(
            os.path.join(OUT, name + ".svg"), "w", encoding="utf-8", newline="\n"
        ) as f:
            f.write(svg(name, title, unit, lower, rows))
        md.append(f"## {title}\n\n" + mermaid(name, title, unit, rows))
        for lab, ka, kb in rows:
            a, b = val(AV, ka, unit), val(AP, kb, unit)
            table.append((title, lab, unit, a, b, ecart(a, b)))
    with open(
        os.path.join(OUT, "mermaid.md"), "w", encoding="utf-8", newline="\n"
    ) as f:
        f.write("\n".join(md))
    if "--table" in sys.argv:
        for t, lab, u, a, b, e in table:
            print(f"| {t} / {lab} | {fmt(a)} | {fmt(b)} | {e:+.1f} % | {u} |")


if __name__ == "__main__":
    main()
