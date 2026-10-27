# /// script
# dependencies = ["numpy", "scikit-image"]
# ///
# The app icon: seahorse valley (c = -0.7514 + 0.1225i, as on the ractive.ch almondbread card,
# ractive.ch scripts/og-fractal.py) with its escape-time bands as flat vector shapes, mint and
# forest green on almond over a dot grid. Writes the Icon Composer document Apps/AppIcon.icon
# (both apps' icon; Xcode compiles it, and makes the flat images for older systems), one SVG per
# layer, a dark variant with the set in almond on forest, and previews in build/icon/ rendered
# by ictool.
#
# uv run scripts/app-icon.py
import json
import shutil
import subprocess
from pathlib import Path

import numpy as np
from skimage import measure

ICON = Path("Apps/AppIcon.icon")
PREVIEW = Path("build/icon")
ICTOOL = Path("/Applications/Xcode.app/Contents/Applications/Icon Composer.app/Contents/Executables/ictool")
SIZE = 1024
SS = 2  # escape counts on a 2x grid, contours back in icon pixels
MAX = 600

CENTRE, WIDTH, TURN = (-0.7475, 0.1205), 0.024, 50  # the card's valley, its edge along the bottom
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
    ds = []
    for cont in measure.find_contours(np.pad(n, 1), level):
        cont = measure.approximate_polygon(cont, 0.8)
        if len(cont) < 4:
            continue
        pts = (cont - 1) / SS
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


def main():
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
    print(f"{ICON}: " + ", ".join(f"{k} {len(v) // 1024} KB" for k, v in files.items()))

    PREVIEW.mkdir(parents=True, exist_ok=True)
    for platform in ("macOS", "iOS"):
        for rendition in ("Default", "Dark", "TintedLight", "ClearLight"):
            out = PREVIEW / f"{platform}-{rendition}.png"
            subprocess.run([ICTOOL, ICON, "--export-image", "--output-file", out, "--platform", platform,
                            "--rendition", rendition, "--width", "1024", "--height", "1024", "--scale", "1"],
                           check=True)
            print(out)


if __name__ == "__main__":
    main()
