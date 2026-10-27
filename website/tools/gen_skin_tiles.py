#!/usr/bin/env python3
"""Make the skin-menu tiles of the 14 painted skins for the web site.

The gallery shows the NikPak window in each skin, 500 x 560. The painted
skins exist as backgrounds only (src/skin_scenes.rs draws them), so the
window's ink - text, icons, the drop ring, the fields - is lifted from the
screenshot of a flat skin (tcblack, whose background is one colour) and the
accent-coloured parts are found in the Nikkoid screenshot. That ink is then
laid over each new background in the skin's own colours.

    python tools/gen_skin_tiles.py [backgrounds dir] [--raw]

Pass --raw when the backgrounds come from a build whose dump already carries
the app's UI veil (the veil is applied inside src/skin.rs::paint, so dumps of
1.6.2 and newer are already discreet). Without it the veil is applied here.

Backgrounds come from the app:
    NIKPAK_SKIN_DUMP=<dir> cargo test --release --lib dump_backgrounds -- --ignored
    (tools/build-shots.ps1 does this, then converts the PPM files to PNG)
"""
import re
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

ROOT = Path(__file__).resolve().parent.parent          # website/
IMG = ROOT / "static" / "img" / "skins"
SRC = ROOT.parent / "src" / "skins_data.rs"

FULL = (500, 560)
THUMB = (250, 280)


def extra_skins():
    """(id, name, dark, fg, accent, bg) of the hand-written skins in skins_data.rs."""
    text = SRC.read_text(encoding="utf-8")
    tail = text[text.index("// NikPak's own skins"):]
    out = []
    for m in re.finditer(r'\("([a-z0-9-]+)", "([^"]+)", (true|false), &\[(.*?)\n    \]\),', tail, re.S):
        sid, name, dark, body = m.groups()
        v = dict(re.findall(r'\("([\w-]+)", "([^"]*)"\)', body))
        # the same corrections the app applies (TWEAKS at the end of the file)
        for sid2, k, val in re.findall(r'\("([a-z0-9-]+)", "([\w-]+)", "([^"]+)"\)', text[text.index("pub(crate) const TWEAKS"):]):
            if sid2 == sid:
                v[k] = val
        out.append((sid, name, dark == "true", v["fg"], v.get("accent", v["fg"]), v["bg"]))
    return out


def hexcolour(s):
    s = s.strip().lstrip("#")
    return np.array([int(s[i:i + 2], 16) for i in (0, 2, 4)], dtype=np.float32)


def load_rgba(path):
    return np.asarray(Image.open(path).convert("RGBA")).astype(np.float32)


def build_ink():
    """Coverage of the window's ink, and the part of it drawn in the accent."""
    flat = load_rgba(IMG / "tcblack.webp")
    lum = flat[..., :3].mean(2)
    bg = 26.0                      # tcblack's background is one flat colour
    cov = np.clip((lum - bg) / (235.0 - bg), 0.0, 1.0)
    # drop the soft washes (panels, glows): keep the crisp ink
    soft = np.asarray(
        Image.fromarray((cov * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(6))
    ).astype(np.float32) / 255.0
    ink = np.clip(cov - soft * 0.85, 0.0, 1.0)
    ink = np.clip(ink * 1.25, 0.0, 1.0)

    # the accent: blue in the Nikkoid screenshot (the layout is identical)
    blue = load_rgba(IMG / "nikkoid.webp")[..., :3]
    accent = np.clip((blue[..., 2] - blue[..., 0]) / 70.0, 0.0, 1.0)
    accent = np.clip(accent * cov, 0.0, 1.0)
    accent = np.asarray(
        Image.fromarray((accent * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(0.6))
    ).astype(np.float32) / 255.0
    window_alpha = flat[..., 3] / 255.0
    return ink, np.clip(accent * 1.4, 0.0, 1.0), window_alpha


def compose(bg, ink, accent, fg, acc, alpha):
    out = bg * (1.0 - ink[..., None]) + fg * ink[..., None]
    # accent ink sits on top of the plain ink
    a = (ink * accent)[..., None]
    out = out * (1.0 - a) + acc * a
    out = np.clip(out, 0, 255)
    rgba = np.dstack([out, alpha * 255.0]).astype(np.uint8)
    return Image.fromarray(rgba, "RGBA")


# the three community buttons keep their brand colours in every skin: take
# them, disc by disc, from the reference screenshot
BRAND = [(338.0, 526.0), (396.5, 526.0), (455.0, 526.0)]
BRAND_R = 18.6


def paste_brand(img):
    ref = Image.open(IMG / "nikkoid.webp").convert("RGBA")
    yy, xx = np.mgrid[0:FULL[1], 0:FULL[0]]
    mask = np.zeros((FULL[1], FULL[0]), dtype=np.float32)
    for cx, cy in BRAND:
        d = np.sqrt((xx - cx) ** 2 + (yy - cy) ** 2)
        mask = np.maximum(mask, np.clip((BRAND_R - d) * 1.6, 0.0, 1.0))
    m = Image.fromarray((mask * 255).astype(np.uint8))
    out = Image.composite(ref, img, m)
    return out


# the veil of src/skin.rs (keep the numbers in step with it)
VEIL_BASE, VEIL_EXTRA, VEIL_TOP = 0.58, 0.30, 0.12
ART_LOCAL, ART_MEAN_R = 0.32, 16


def discreet(px, bg):
    """Flatten the art toward its local mean, then fade it toward the skin's
    background where the text and the drop ring sit."""
    from PIL import ImageFilter
    mean = np.stack([
        np.asarray(Image.fromarray(px[..., i].astype(np.uint8)).filter(ImageFilter.GaussianBlur(ART_MEAN_R)))
        for i in range(3)], axis=2).astype(np.float32)
    px = mean + (px - mean) * ART_LOCAL
    for _ in range(2):
        px = (px + np.roll(px, 1, 0) * .5 + np.roll(px, -1, 0) * .5
              + np.roll(px, 1, 1) * .5 + np.roll(px, -1, 1) * .5) / 3.0
    h, w = px.shape[:2]
    ys = (np.arange(h) + .5) / h
    xs = (np.arange(w) + .5) / w
    r = np.sqrt(((xs[None, :] - .5) / .52) ** 2 + ((ys[:, None] - .42) / .55) ** 2)
    k = VEIL_BASE + VEIL_EXTRA * np.clip(1 - r, 0, 1) ** 1.2
    k = k + VEIL_TOP * np.clip((.22 - ys) / .22, 0, 1)[:, None]
    return px * (1 - k[..., None]) + np.asarray(bg, np.float32) * k[..., None]


def read_background(path):
    im = Image.open(path).convert("RGB")
    if im.size != FULL:
        im = im.resize(FULL, Image.LANCZOS)
    return np.asarray(im).astype(np.float32)


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    raw = "--raw" in sys.argv
    bgdir = Path(args[0]) if args else ROOT.parent / "screenshots" / "1.6.2" / "backgrounds_png"
    ink, accent, alpha = build_ink()
    skins = extra_skins()
    print(f"{len(skins)} painted skins, backgrounds from {bgdir}")
    for sid, name, dark, fg, acc, bgc in skins:
        src = bgdir / f"{sid}.png"
        if not src.exists():
            print(f"  {sid}: no background at {src} - skipped")
            continue
        bg = read_background(src)
        if not raw:
            bg = discreet(bg, hexcolour(bgc))
        img = compose(bg, ink, accent, hexcolour(fg), hexcolour(acc), alpha)
        img = paste_brand(img)
        img.save(IMG / f"{sid}.webp", "WEBP", quality=92, method=6)
        img.resize(THUMB, Image.LANCZOS).save(IMG / f"{sid}-thumb.webp", "WEBP", quality=90, method=6)
        print(f"  {sid:14} {name:16} {'dark' if dark else 'bright'}")
    print("done")


if __name__ == "__main__":
    main()
