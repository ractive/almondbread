# /// script
# dependencies = ["numpy==2.4.6", "scikit-image==0.26.0"]  # pinned: another contour version rewrites the SVGs
# ///
# The app icon: seahorse valley (near c = -0.7514 + 0.1225i, the view of the ractive.ch almondbread
# card, ractive.ch scripts/og-fractal.py) with its escape-time bands as flat vector shapes, mint and
# forest green on almond over a dot grid. Writes the Icon Composer document Apps/AppIcon.icon
# (both apps' icon; Xcode compiles it, and makes the flat images for older systems), one SVG per
# layer, a dark variant with the set in almond on forest, and previews in build/icon/ rendered
# by ictool.
#
# uv run scripts/app-icon.py
import json
import os
import shutil
import subprocess
from pathlib import Path

import numpy as np
from skimage import measure

ROOT = Path(__file__).resolve().parents[1]
ICON = ROOT / "Apps/AppIcon.icon"
PREVIEW = ROOT / "build/icon"
SIZE = 1024
SS = 2  # escape counts on a 2x grid, contours back in icon pixels
MAX = 600
MIN_AREA = 3.0  # icon px^2

# A crop inside the card's view, turned so the set's edge runs along the bottom. Not mirrored like
# the card (so the seahorses curl the other way), and counting from z1 where the card counts z0.
CENTRE, WIDTH, TURN = (-0.7475, 0.1205), 0.024, 50
ALMOND, DOTS, FOREST, DOTS_DARK = "#efe3cf", "#c9b796", "#00362a", "#0f5a48"
# Outer to inner, as on the card: escape-time level, colour, opacity. The set is n >= MAX.
BANDS = [(30, "#a0d1bf", .07), (36, "#a0d1bf", .13), (48, "#a0d1bf", .22), (70, "#7abfa8", .36), (120, "#3f9a8c", .55)]


def escape_counts():
    n_px = SIZE * SS
    t = np.radians(TURN)
    px = (np.arange(n_px) + 0.5) / SS
    x, y = np.meshgrid(px, px)
    u = (x - SIZE / 2) / SIZE * WIDTH
    v = (SIZE / 2 - y) / SIZE * WIDTH
    c = (CENTRE[0] + u * np.cos(t) - v * np.sin(t)) + 1j * (CENTRE[1] + u * np.sin(t) + v * np.cos(t))
    c = c.ravel()
    n = np.zeros(c.shape, dtype=np.int32)
    live, z, cl = np.arange(c.size), np.zeros_like(c), c
    for _ in range(MAX):
        z = z * z + cl
        inside = (z.real * z.real + z.imag * z.imag) <= 4
        n[live[inside]] += 1
        live, z, cl = live[inside], z[inside], cl[inside]
        if live.size == 0:
            break
    return n.reshape(n_px, n_px)


def outline(n, level):
    # The edge row repeated once beyond the border, then a zero frame: a region that touches the
    # border closes half a sample outside the canvas, so nothing shows a seam at the edge. Grid
    # index i is at icon pixel (i + 0.5) / SS, so padded index i at (i - 1.5) / SS.
    padded = np.pad(np.pad(n, 1, mode="edge"), 1)
    ds = []
    for cont in measure.find_contours(padded, level):
        cont = measure.approximate_polygon(cont, 0.8)
        if len(cont) < 4:
            continue
        pts = (cont - 1.5) / SS
        # Specks under MIN_AREA are counts at the limit that escape soon after it (nearly all
        # escape within 20000 iterations), not islands of the set.
        area = 0.5 * abs(np.dot(pts[:, 0], np.roll(pts[:, 1], 1)) - np.dot(pts[:, 1], np.roll(pts[:, 0], 1)))
        if area < MIN_AREA:
            continue
        ds.append("M" + " ".join(f"{p[1]:.1f} {p[0]:.1f}" for p in pts) + "Z")
    return "".join(ds)


def svg(*elements):
    head = f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {SIZE} {SIZE}" width="{SIZE}" height="{SIZE}">'
    return "\n".join([head, *elements, "</svg>\n"])


def dots(colour, opacity):
    return svg(
        '<defs><pattern id="dots" width="32" height="32" patternUnits="userSpaceOnUse">'
        f'<circle cx="16" cy="16" r="2.6" fill="{colour}"/></pattern></defs>',
        f'<rect width="{SIZE}" height="{SIZE}" fill="url(#dots)" opacity="{opacity}"/>',
    )


def srgb(hex_colour):
    r, g, b = (int(hex_colour[i:i + 2], 16) / 255 for i in (1, 3, 5))
    return f"srgb:{r:.5f},{g:.5f},{b:.5f},1.00000"


def layer(name, image, dark_image=None):
    # A plain value next to its specializations wins over them: the light one goes in the list.
    if dark_image:
        return {"name": name, "glass": False,
                "image-name-specializations": [{"value": image}, {"appearance": "dark", "value": dark_image}]}
    return {"name": name, "glass": False, "image-name": image}


def ictool():
    # Icon Composer ships inside Xcode; the Makefile's DEVELOPER_DIR may name another Xcode.
    developer = os.environ.get("DEVELOPER_DIR") or subprocess.run(
        ["xcode-select", "-p"], capture_output=True, text=True, check=True).stdout.strip()
    tool = Path(developer).parent / "Applications/Icon Composer.app/Contents/Executables/ictool"
    if not tool.exists():
        raise SystemExit(f"no ictool at {tool} (Xcode 26 or later)")
    return tool


def main():
    tool = ictool()
    n = escape_counts()
    assets = ICON / "Assets"
    shutil.rmtree(ICON, ignore_errors=True)
    assets.mkdir(parents=True)
    bands = [f'<path fill="{col}" fill-opacity="{op}" fill-rule="evenodd" d="{outline(n, lvl)}"/>'
             for lvl, col, op in BANDS]
    the_set = outline(n, MAX - 0.5)
    files = {
        "dots.svg": dots(DOTS, 0.55),
        "dots-dark.svg": dots(DOTS_DARK, 0.6),
        "bands.svg": svg(*bands),
        "set.svg": svg(f'<path fill="{FOREST}" fill-rule="evenodd" d="{the_set}"/>'),
        "set-dark.svg": svg(f'<path fill="{ALMOND}" fill-rule="evenodd" d="{the_set}"/>'),
    }
    for name, text in files.items():
        (assets / name).write_text(text)
    doc = {
        "fill-specializations": [{"value": {"solid": srgb(ALMOND)}},
                                 {"appearance": "dark", "value": {"solid": srgb(FOREST)}}],
        # Front to back.
        "groups": [
            {"layers": [layer("set", "set.svg", "set-dark.svg")]},
            {"layers": [layer("bands", "bands.svg")]},
            {"layers": [layer("dots", "dots.svg", "dots-dark.svg")]},
        ],
        "supported-platforms": {"squares": "shared"},
    }
    (ICON / "icon.json").write_text(json.dumps(doc, indent=2) + "\n")
    print(f"{ICON.relative_to(ROOT)}: " + ", ".join(f"{k} {len(v) // 1024} KB" for k, v in files.items()))

    PREVIEW.mkdir(parents=True, exist_ok=True)
    for platform in ("macOS", "iOS"):
        for rendition in ("Default", "Dark", "TintedLight", "ClearLight"):
            out = PREVIEW / f"{platform}-{rendition}.png"
            subprocess.run([tool, ICON, "--export-image", "--output-file", out, "--platform", platform,
                            "--rendition", rendition, "--width", "1024", "--height", "1024", "--scale", "1"],
                           check=True)
            print(out)


if __name__ == "__main__":
    main()
