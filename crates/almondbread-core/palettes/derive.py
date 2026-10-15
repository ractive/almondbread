#!/usr/bin/env python3
"""Writes palettes/derived.json: the palette-library entries taken from open colour-map data
(kb: synthesis/fractal-palettes, techniques/palette-design).

Usage: derive.py DOWNLOAD_DIR > derived.json   (run with python3 -I; no third-party modules)

DOWNLOAD_DIR holds the unchanged source files (fetch them with the URLs below):
  mpl_listed.py      matplotlib lib/matplotlib/_cm_listed.py (viridis family: the same lists as
                     BIDS/colormap colormaps.py, CC0; cividis, turbo, twilight)
  cmo_<name>.txt     cmocean cmocean/rgb/<name>-rgb.txt (MIT)
  cram_<name>.txt    cmcrameri cmcrameri/cmaps/<name>.txt (Crameri Scientific colour maps, MIT)
  colorcet_init.py   holoviz colorcet colorcet/__init__.py (Kovesi CET maps, CC BY 4.0)

Each 256-entry map is subsampled to the fewest evenly spaced stops (12..32) whose piecewise
linear OKLab interpolation stays within 0.006 of every original entry; the library joins
stops with a Catmull-Rom spline, which is closer still. Stops are stored as sRGB hex.
"""

import ast
import json
import math
import re
import sys

GH = "https://raw.githubusercontent.com"
MPL = ("matplotlib", f"{GH}/matplotlib/matplotlib/main/lib/matplotlib/_cm_listed.py")
CMO = f"{GH}/matplotlib/cmocean/main/cmocean/rgb/"
CRAM = f"{GH}/callumrollo/cmcrameri/master/cmcrameri/cmaps/"
CET = f"{GH}/holoviz/colorcet/main/colorcet/__init__.py"

LIC_MPL_CC0 = "CC0-1.0"
LIC_MIT = "MIT"
LIC_CCBY = "CC-BY-4.0"
LIC_APACHE = "Apache-2.0"

# (id, name, category, source kind, source name, cyclic, density, attribution, licence)
ENTRIES = [
    # Scientific, perceptually uniform (sequential; the library mirrors them so they cycle).
    ("viridis", "Viridis", "scientific", "mpl", "viridis", False, 0.02,
     "viridis by Stéfan van der Walt, Nathaniel Smith and Eric Firing (matplotlib)", LIC_MPL_CC0),
    ("magma", "Magma", "scientific", "mpl", "magma", False, 0.02,
     "magma by Stéfan van der Walt and Nathaniel Smith (matplotlib)", LIC_MPL_CC0),
    ("inferno", "Inferno", "scientific", "mpl", "inferno", False, 0.02,
     "inferno by Stéfan van der Walt and Nathaniel Smith (matplotlib)", LIC_MPL_CC0),
    ("plasma", "Plasma", "scientific", "mpl", "plasma", False, 0.02,
     "plasma by Stéfan van der Walt and Nathaniel Smith (matplotlib)", LIC_MPL_CC0),
    ("cividis", "Cividis", "scientific", "mpl", "cividis", False, 0.02,
     "cividis by Jamie R. Nuñez, Christopher R. Anderton and Ryan S. Renslow (PLOS ONE 2018, CC0); "
     "data as shipped in matplotlib, from PNNL cmaputil (Copyright Battelle Memorial Institute)",
     "BSD-style (Battelle/PNNL cmaputil; matplotlib licence)"),
    ("turbo", "Turbo", "scientific", "mpl", "turbo", False, 0.015,
     "Turbo by Anton Mikhailov, Google LLC (2019)", LIC_APACHE),
    ("batlow", "Batlow", "scientific", "cram", "batlow", False, 0.02,
     "batlow, Scientific colour maps by Fabio Crameri (doi:10.5281/zenodo.1243862)", LIC_MIT),
    ("hawaii", "Hawaii", "scientific", "cram", "hawaii", False, 0.02,
     "hawaii, Scientific colour maps by Fabio Crameri (doi:10.5281/zenodo.1243862)", LIC_MIT),
    ("lajolla", "La Jolla", "scientific", "cram", "lajolla", False, 0.02,
     "lajolla, Scientific colour maps by Fabio Crameri (doi:10.5281/zenodo.1243862)", LIC_MIT),
    ("oslo", "Oslo", "scientific", "cram", "oslo", False, 0.02,
     "oslo, Scientific colour maps by Fabio Crameri (doi:10.5281/zenodo.1243862)", LIC_MIT),
    ("tokyo", "Tokyo", "scientific", "cram", "tokyo", False, 0.02,
     "tokyo, Scientific colour maps by Fabio Crameri (doi:10.5281/zenodo.1243862)", LIC_MIT),
    ("haline", "Haline", "scientific", "cmo", "haline", False, 0.02,
     "haline, cmocean by Kristen Thyng et al. (Oceanography 2016)", LIC_MIT),
    ("dense", "Dense", "scientific", "cmo", "dense", False, 0.02,
     "dense, cmocean by Kristen Thyng et al. (Oceanography 2016)", LIC_MIT),
    ("solar", "Solar", "scientific", "cmo", "solar", False, 0.02,
     "solar, cmocean by Kristen Thyng et al. (Oceanography 2016)", LIC_MIT),
    ("algae", "Algae", "scientific", "cmo", "algae", False, 0.02,
     "algae, cmocean by Kristen Thyng et al. (Oceanography 2016)", LIC_MIT),
    # Cyclic: seamless by construction.
    ("twilight", "Twilight", "cyclic", "mpl", "twilight", True, 0.02,
     "twilight by Bastian Bechtold, data as shipped in matplotlib", "matplotlib licence (PSF-style, BSD-compatible)"),
    ("roma-o", "Roma O", "cyclic", "cram", "romaO", True, 0.02,
     "romaO, Scientific colour maps by Fabio Crameri (doi:10.5281/zenodo.1243862)", LIC_MIT),
    ("vik-o", "Vik O", "cyclic", "cram", "vikO", True, 0.02,
     "vikO, Scientific colour maps by Fabio Crameri (doi:10.5281/zenodo.1243862)", LIC_MIT),
    ("bam-o", "Bam O", "cyclic", "cram", "bamO", True, 0.02,
     "bamO, Scientific colour maps by Fabio Crameri (doi:10.5281/zenodo.1243862)", LIC_MIT),
    ("cet-c1", "Magenta Wheel (CET-C1)", "cyclic", "cet", "cyclic_mrybm_35_75_c68", True, 0.02,
     "CET-C1 by Peter Kovesi, Good Colour Maps (arXiv:1509.03700), via colorcet", LIC_CCBY),
    ("cet-c2", "Colour Wheel (CET-C2)", "cyclic", "cet", "cyclic_mygbm_30_95_c78", True, 0.02,
     "CET-C2 by Peter Kovesi, Good Colour Maps (arXiv:1509.03700), via colorcet", LIC_CCBY),
    ("cet-c3", "Red-Black-Blue (CET-C3)", "cyclic", "cet", "cyclic_wrkbw_10_90_c43", True, 0.02,
     "CET-C3 by Peter Kovesi, Good Colour Maps (arXiv:1509.03700), via colorcet", LIC_CCBY),
    ("cet-c6", "Six Hues (CET-C6)", "cyclic", "cet", "cyclic_rygcbmr_50_90_c64", True, 0.02,
     "CET-C6 by Peter Kovesi, Good Colour Maps (arXiv:1509.03700), via colorcet", LIC_CCBY),
    ("cet-c9", "Dark Wheel (CET-C9)", "cyclic", "cet", "cyclic_mybm_20_100_c48", True, 0.02,
     "CET-C9 by Peter Kovesi, Good Colour Maps (arXiv:1509.03700), via colorcet", LIC_CCBY),
]


# Cosine palettes (technique: Inigo Quilez, "palettes", iquilezles.org): colour(t) = a + b cos(2 pi (c t + d))
# per sRGB channel. Integer c keeps them seamless. Parameters are our own.
# (id, name, category, a, b, c, d, density)
COSINE = [
    ("cosine-candy", "Cosine Candy", "neon", (0.6, 0.45, 0.6), (0.4, 0.45, 0.4), (1, 1, 1), (0.0, 0.15, 0.3), 0.02),
    ("cosine-copper", "Cosine Copper", "classic", (0.45, 0.32, 0.22), (0.45, 0.32, 0.22), (1, 1, 1), (0.5, 0.52, 0.55), 0.025),
    ("cosine-teal-amber", "Teal and Amber", "classic", (0.5, 0.5, 0.45), (0.5, 0.4, 0.4), (1, 1, 1), (0.0, 0.85, 0.65), 0.025),
]


def cosine_stops(a, b, c, d, n=24):
    out = []
    for i in range(n):
        t = i / n
        out.append([min(max(a[k] + b[k] * math.cos(2 * math.pi * (c[k] * t + d[k])), 0.0), 1.0) for k in range(3)])
    return out


def py_list(text, name):
    """The list literal assigned to `name` in a Python source (parsed, never executed)."""
    m = re.search(r"^" + re.escape(name) + r"\s*=\s*\[", text, re.M)
    if not m:
        raise SystemExit(f"{name} not found")
    i = m.end() - 1
    depth = 0
    for j in range(i, len(text)):
        ch = text[j]
        if ch == "[":
            depth += 1
        elif ch == "]":
            depth -= 1
            if depth == 0:
                body = re.sub(r"#[^\n]*", "", text[i : j + 1])
                return ast.literal_eval(body)
    raise SystemExit(f"{name}: unbalanced")


def table_rows(path):
    with open(path) as f:
        return [[float(v) for v in line.split()] for line in f if line.strip()]


def to_linear(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def oklab(rgb):
    r, g, b = (to_linear(min(max(c, 0.0), 1.0)) for c in rgb)
    l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b) ** (1 / 3)
    m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b) ** (1 / 3)
    s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b) ** (1 / 3)
    return (
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    )


def subsample(rows, cyclic, tol=0.006):
    n = len(rows)
    lab = [oklab(r) for r in rows]
    for k in range(12, 33):
        # Stop indices: cyclic maps take k stops over the circle, sequential ones include both ends.
        idx = [round(i * n / k) % n for i in range(k)] if cyclic else [round(i * (n - 1) / (k - 1)) for i in range(k)]
        worst = 0.0
        segs = list(zip(idx, idx[1:] + ([idx[0] + n] if cyclic else [])))
        for a, b in segs:
            for j in range(a, b + 1):
                u = (j - a) / max(b - a, 1)
                pa, pb, pj = lab[a % n], lab[b % n], lab[j % n]
                d = math.dist([pa[q] + (pb[q] - pa[q]) * u for q in range(3)], pj)
                worst = max(worst, d)
        if worst <= tol:
            break
    return [rows[i] for i in idx]


def hexs(rgb):
    return "#" + "".join(f"{min(max(round(c * 255), 0), 255):02X}" for c in rgb)


def main():
    d = sys.argv[1]
    mpl = open(f"{d}/mpl_listed.py").read()
    cet = open(f"{d}/colorcet_init.py").read()
    out = []
    for pid, name, cat, kind, src, cyclic, density, attribution, licence in ENTRIES:
        if kind == "mpl":
            rows, url = py_list(mpl, f"_{src}_data"), MPL[1]
        elif kind == "cmo":
            rows, url = table_rows(f"{d}/cmo_{src}.txt"), f"{CMO}{src}-rgb.txt"
        elif kind == "cram":
            rows, url = table_rows(f"{d}/cram_{src}.txt"), f"{CRAM}{src}.txt"
        else:
            rows, url = py_list(cet, src), CET
        if not cyclic and oklab(rows[0])[0] > oklab(rows[-1])[0]:
            # Dark end first: the far exterior (few iterations, t near 0) stays dark.
            rows = rows[::-1]
        stops = subsample(rows, cyclic)
        # Cyclic maps start at their darkest stop via the offset: the far exterior stays dark.
        lightness = [oklab(s)[0] for s in stops]
        offset = round(lightness.index(min(lightness)) / len(stops), 4) if cyclic else 0.0
        out.append(
            {
                "id": pid,
                "name": name,
                "category": cat,
                "cyclic": cyclic,
                "density": density,
                "offset": offset,
                "stops": [hexs(s) for s in stops],
                "attribution": attribution,
                "license": licence,
                "source": url,
            }
        )
    for pid, name, cat, a, b, c, dd, density in COSINE:
        stops = cosine_stops(a, b, c, dd)
        lightness = [oklab(s)[0] for s in stops]
        offset = round(lightness.index(min(lightness)) / len(stops), 4)
        out.append(
            {
                "id": pid,
                "name": name,
                "category": cat,
                "cyclic": True,
                "density": density,
                "offset": offset,
                "stops": [hexs(s) for s in stops],
                "attribution": "own parameters; cosine palette technique by Inigo Quilez",
                "license": "CC0-1.0",
                "source": "https://iquilezles.org/articles/palettes/",
            }
        )
    doc = {
        "almondbread_palettes": 1,
        "comment": "Generated by derive.py from open colour-map data; do not edit by hand.",
        "palettes": out,
    }
    json.dump(doc, sys.stdout, indent=1, ensure_ascii=False)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
