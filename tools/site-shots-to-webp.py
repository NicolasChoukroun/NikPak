#!/usr/bin/env python3
"""Turn website/shots-raw/*.png (from tools/site-shots.ps1) into the site images.

    python tools/site-shots-to-webp.py

Reads the PNG captures of the current build and writes the .webp files of
website/static/img/, including the two composite pictures:
  lang-quad.webp  - Chinese, Hindi (reminder), Arabic, English (language menu)
  unnik-pair.webp - unNik waiting + unNik showing an archive
"""
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent          # nikpak/
RAW = ROOT / "website" / "shots-raw"
IMG = ROOT / "website" / "static" / "img"

GUTTER = 22
WHITE = (255, 255, 255)

# The site shows each window at its logical size, whatever the DPI of the
# capture screen. site-shots.ps1 is DPI-aware, so a 150 % screen yields 1.5x
# captures (750 x 840, …); every image is normalised to these sizes here.
NOMINAL = {
    "nikpak-drop": (500, 560),
    "nikpak-compress": (500, 560),
    "nikpak-choose": (500, 560),
    "nikpak-about": (500, 560),
    "nikpak-reminder": (500, 560),
    "nikpak-register": (500, 560),
    "nikpak-split": (500, 560),
    "nikpak-extract": (500, 560),
    "nikpak-package": (500, 560),
    "skins-menu": (1060, 560),
    "skins-menu-bright": (1060, 560),
    "unnik-idle": (500, 560),
    "unnik-browser": (1120, 720),
    "unnik-skins": (1120, 720),
    "unnik-password": (500, 560),
    "unnik-extract": (1120, 720),
    "lang-zh": (500, 560),
    "lang-hi-nag": (500, 560),
    "lang-ar": (500, 560),
    "lang-en-menu": (500, 560),
}


def load(name):
    p = RAW / f"{name}.png"
    if not p.exists():
        print(f"  missing {p.name}")
        return None
    return Image.open(p).convert("RGBA")


def fit(im, name):
    """Rescale a capture to the window's logical size."""
    size = NOMINAL.get(name)
    if size and im.size != size:
        im = im.resize(size, Image.LANCZOS)
    return im


def save(im, name, quality=92):
    im.save(IMG / f"{name}.webp", "WEBP", quality=quality, method=6)
    print(f"  {name}.webp {im.size[0]}x{im.size[1]}")


def single(name):
    im = load(name)
    if im is not None:
        save(fit(im, name), name)


def grid(names, out):
    """The four captures, two by two, on white - the language quadrant."""
    ims = [load(n) for n in names]
    if any(i is None for i in ims):
        return
    ims = [fit(im, n) for im, n in zip(ims, names)]
    w = max(i.size[0] for i in ims)
    h = max(i.size[1] for i in ims)
    sheet = Image.new("RGB", (w * 2 + GUTTER * 3, h * 2 + GUTTER * 3), WHITE)
    for k, im in enumerate(ims):
        x = GUTTER + (k % 2) * (w + GUTTER)
        y = GUTTER + (k // 2) * (h + GUTTER)
        sheet.paste(im, (x, y), im)
    save(sheet, out, 90)


def pair(idle_name, browser_name, out):
    """unNik waiting for an archive, next to unNik showing one."""
    idle = load(idle_name)
    browser = load(browser_name)
    if idle is None or browser is None:
        return
    idle = fit(idle, idle_name)
    browser = fit(browser, browser_name)
    # bring the browser window to the height of the idle one, as before
    scale = idle.size[1] / browser.size[1]
    bw = int(browser.size[0] * scale)
    browser = browser.resize((bw, idle.size[1]), Image.LANCZOS)
    sheet = Image.new("RGB", (idle.size[0] + GUTTER + bw, idle.size[1]), WHITE)
    sheet.paste(idle, (0, 0), idle)
    sheet.paste(browser, (idle.size[0] + GUTTER, 0), browser)
    save(sheet, out, 90)


# unNik's two dialogs are drawn at their nominal window sizes, so the composite
# does not depend on the DPI of the screen the captures were taken on.
PASSWORD_WIN = (500, 560)
EXTRACT_WIN = (1120, 720)


def dialogs(password_name, extract_name, out):
    """unNik's Password box next to its Extract box (the "apps" picture)."""
    pw = load(password_name)
    ex = load(extract_name)
    if pw is None or ex is None:
        return
    pw = pw.resize(PASSWORD_WIN, Image.LANCZOS)
    ex = ex.resize(EXTRACT_WIN, Image.LANCZOS)
    scale = PASSWORD_WIN[1] / EXTRACT_WIN[1]
    ex = ex.resize((round(EXTRACT_WIN[0] * scale), PASSWORD_WIN[1]), Image.LANCZOS)
    # transparent gutter, so the gap between the two boxes shows the page behind
    sheet = Image.new("RGBA", (PASSWORD_WIN[0] + GUTTER + ex.size[0], PASSWORD_WIN[1]), (0, 0, 0, 0))
    sheet.paste(pw, (0, 0), pw)
    sheet.paste(ex, (PASSWORD_WIN[0] + GUTTER, 0), ex)
    save(sheet, out, 90)


def main():
    if not RAW.exists():
        sys.exit(f"no captures: run tools/site-shots.ps1 first ({RAW})")
    print(f"reading {RAW}")
    for n in ["nikpak-drop", "nikpak-compress", "nikpak-choose", "nikpak-about",
              "nikpak-reminder", "nikpak-register", "nikpak-split", "nikpak-extract",
              "nikpak-package", "skins-menu", "skins-menu-bright",
              "unnik-browser", "unnik-skins"]:
        single(n)
    grid(["lang-zh", "lang-hi-nag", "lang-ar", "lang-en-menu"], "lang-quad")
    pair("unnik-idle", "unnik-browser", "unnik-pair")
    dialogs("unnik-password", "unnik-extract", "unnik-dialogs2")
    print("done")


if __name__ == "__main__":
    main()
