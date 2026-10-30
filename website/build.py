#!/usr/bin/env python3
"""Builds the NikPak web site into dist/.

    python build.py

Sources: site.json (public settings), i18n/<lang>.json (texts), static/ (css, js,
images), php/ (contact form, shop, redirects). Every link points to a page file
(en/index.html, fr/buy.html ...), never to a bare folder.
"""
import html
import json
import os
import re
import shutil
import sys

ROOT = os.path.dirname(os.path.abspath(__file__))
DIST = os.path.join(ROOT, "dist")

# Same order as the language menu of the app (src/i18n.rs)
LANGS = ["en", "zh", "hi", "es", "ar", "fr", "bn", "pt", "ru", "id"]
HTML_LANG = {"en": "en", "zh": "zh-Hans", "hi": "hi", "es": "es", "ar": "ar", "fr": "fr", "bn": "bn", "pt": "pt", "ru": "ru", "id": "id"}
FONTS = {
    "zh": "&family=Noto+Sans+SC:wght@300;400;500",
    "hi": "&family=Noto+Sans+Devanagari:wght@300;400;500;600",
    "bn": "&family=Noto+Sans+Bengali:wght@300;400;500;600",
    "ar": "&family=Noto+Sans+Arabic:wght@300;400;500;600",
}
PAGES = ["index", "features", "benchmarks", "guide", "tech", "skins", "buy", "contact", "changelog", "thanks"]
# Top navigation. A string is a plain link; a tuple is a dropdown:
# ("guide", ["guide", "tech"]) -> a "User guide" dropdown with "User guide" and "Technical".
# "buy" is left out of the top menu on purpose (it stays in the footer and the CTAs).
NAV = ["index", "features", "benchmarks", ("guide", ["guide", "tech"]), "skins", ("contact", ["contact", "changelog"])]
NAV_KEY = {"index": "home", "features": "features", "benchmarks": "benchmarks", "guide": "guide", "tech": "tech", "skins": "skins", "buy": "buy", "contact": "contact", "changelog": "changelog"}
TITLE_KEY = {"index": "t_home", "features": "t_features", "benchmarks": "t_benchmarks", "guide": "t_guide", "tech": "t_tech", "skins": "t_skins", "buy": "t_buy", "contact": "t_contact", "changelog": "t_changelog", "thanks": "t_thanks"}

# Number format per language: (thousands separator, decimal separator)
NUMFMT = {"en": (",", "."), "zh": (",", "."), "hi": (",", "."), "bn": (",", "."), "ar": (",", "."),
          "es": (".", ","), "pt": (".", ","), "id": (".", ","), "fr": ("\u202f", ","), "ru": ("\u202f", ",")}

TIERS = [(1, 19), (10, 15), (25, 12), (50, 9), (100, 7), (200, 6), (500, 5)]

# ---- Benchmarks (technical guide, section 11.7) ----
SETS = ["bmp", "mod", "wav", "m44", "m22", "m22b", "m48", "txt", "office", "gen"]
ORIGINAL = {"bmp": 6987840, "mod": 2252604, "wav": 2991820, "m44": 17760408, "m22": 4440136, "m22b": 2220090,
            "m48": 28996592, "txt": 11793318, "office": 39100915, "gen": 190454632}
MAXSIZE = {  # nikpak -m5, RAR 7 -m5, 7-Zip -mx9, ZIP -9
    "bmp": (3557350, 4394310, 4195043, 4974680), "mod": (1127607, 1264245, 1166855, 1408085),
    "wav": (1346484, 1596971, 1484879, 1852279), "m44": (8731989, 10460042, 10666922, 15225764),
    "m22": (2458867, 2797026, 2784080, 3736722), "m22b": (606840, 725416, 642341, 836989),
    "m48": (18198056, 21229465, 25542036, 26980770), "txt": (2741898, 3519622, 3259868, 4223579),
    "office": (6887729, 11186217, 10085779, 17399998), "gen": (28175618, 33606893, 28879398, 62198510)}
MAX_TOOLS = ["NikPak -m5", "RAR 7 -m5", "7-Zip -mx9", "ZIP -9"]
FAST = {  # (% of original, seconds) for nikpak -m2, RAR -m3, 7-Zip -mx5, ZIP -6
    "bmp": ((54.0, .39), (63.0, 2.09), (60.1, 3.51), (71.2, .35)), "mod": ((52.4, .61), (56.2, .43), (51.9, .69), (62.7, .10)),
    "wav": ((51.1, .41), (53.6, .71), (49.6, 1.96), (61.9, .24)), "m44": ((49.2, 1.72), (59.9, 3.60), (60.1, 14.03), (85.7, .82)),
    "m22": ((55.4, .58), (64.1, .95), (62.7, 2.14), (84.2, .22)), "m22b": ((27.4, .77), (33.3, .52), (28.9, 2.13), (38.2, .32)),
    "m48": ((62.8, 2.06), (73.6, 7.54), (88.1, 13.54), (93.0, 1.19)), "txt": ((35.2, .11), (30.4, 5.53), (27.7, 11.76), (36.0, .89)),
    "office": ((38.0, .22), (29.5, 8.25), (26.2, 21.85), (44.7, 1.83)), "gen": ((25.3, 1.67), (18.0, 48.19), (17.0, 93.71), (32.9, 7.32))}
FAST_TOOLS = ["NikPak -m2", "RAR 7 -m3", "7-Zip -mx5", "ZIP -6"]
GENERAL = [  # tool, size, %, creation s, extraction s, is nikpak
    ("NikPak -m5", 28175618, 14.8, 154.3, 5.6, True), ("7-Zip -mx9", 28879398, 15.2, 120.9, 6.0, False),
    ("RAR 7 -m5 -s -md256m", 32350575, 17.0, 73.6, 7.0, False), ("7-Zip -mx5", 32375802, 17.0, 93.7, 4.5, False),
    ("NikPak -m4", 33295133, 17.5, 37.8, 3.3, True), ("RAR 7 -m5 -s", 33606893, 17.6, 72.4, 6.7, False),
    ("RAR 7 -m3 -s", 34236198, 18.0, 48.2, 5.9, False), ("NikPak -m2", 48229284, 25.3, 1.7, .9, True),
    ("RAR 7 -m3", 56038351, 29.4, 20.2, 6.9, False), ("ZIP -9", 62198510, 32.7, 28.2, 6.9, False),
    ("ZIP -6", 62672415, 32.9, 7.3, 5.4, False)]

# ---- Changelog (CHANGELOG.md, newest first): version, date, one-line summary ----
CHANGELOG = [
    ("1.6.3", "30 October 2026", "Fast, Medium and Best compression in the window, with live size and time estimates."),
    ("1.6.2", "24 October 2026", "The fourteen painted skins step behind the interface, so the window stays readable."),
    ("1.6.1", "24 October 2026", "The storm and the mountains, painted again."),
    ("1.6.0", "24 October 2026", "Fourteen painted skins, a real thunderstorm and better mountains."),
    ("1.5.6", "24 October 2026", "Skins: the 41 skins of Nikkomander in NikPak and unNik."),
    ("1.5.5", "23 October 2026", "Right-click menu “NikPak” in Explorer, Dolphin, Nemo, Nautilus and Finder."),
    ("1.5.4", "23 October 2026", "nikpak.exe and unNik.exe get their icons."),
    ("1.5.3", "23 October 2026", "Packing folders with hundreds of thousands of files starts at once."),
    ("1.5.2", "22 October 2026", "The setup installs both programs and registers .nik files."),
    ("1.5.1", "22 October 2026", "Switch between NikPak and unNik, shared language, “open folder” buttons, sound effects."),
    ("1.5.0", "22 October 2026", "unNik, the free archive browser; selective extraction and delete."),
    ("1.4.5", "22 October 2026", "Ten languages, Cancel button, window sized for X posts."),
    ("1.4.4", "22 October 2026", "Gentler volume prices (RarLab-style steps)."),
    ("1.4.3", "22 October 2026", "New prices, new copyright, About box."),
    ("1.4.2", "22 October 2026", "First test on Windows; dialogs in front."),
    ("1.4.1", "22 October 2026", "Windows installer."),
    ("1.4.0", "22 October 2026", "Shareware edition: registration, licence keys, licence server kit."),
    ("1.3.1", "21 October 2026", "Drag and drop fix, choose files, brand-coloured links."),
    ("1.3.0", "21 October 2026", "Drop window and community links."),
    ("1.2.1", "21 October 2026", "Best compression by default."),
    ("1.2.0", "21 October 2026", "Beats 7-Zip on office files, text and source code."),
    ("1.1.0", "October 2026", "Media intelligence."),
    ("1.0.0", "October 2026", "First version."),
]


def load(p):
    with open(os.path.join(ROOT, p), encoding="utf-8") as f:
        return json.load(f)


SITE = load("site.json")
# the skins of NikPak / unNik (same as Nikkomander): id, name, dark; pictures in static/img/skins/
SKINS = load("static/img/skins/skins.json")
EN = load("i18n/en.json")
MISSING = []


class Lang:
    def __init__(self, code):
        self.code = code
        path = os.path.join(ROOT, "i18n", code + ".json")
        self.d = load("i18n/" + code + ".json") if os.path.exists(path) else {}
        self.th, self.dec = NUMFMT[code]

    def raw(self, key):
        def get(d):
            for part in key.split("."):
                if not isinstance(d, dict) or part not in d:
                    return None
                d = d[part]
            return d
        v = get(self.d)
        if v is None:
            v = get(EN)
            if self.code != "en":
                MISSING.append((self.code, key))
        if v is None:
            raise KeyError(key)
        return v

    def __call__(self, key, **kw):
        v = self.raw(key)
        for k, val in kw.items():
            v = v.replace("{" + k + "}", str(val))
        return v

    def n(self, x, digits=0):
        s = f"{x:,.{digits}f}"
        return s.replace(",", "\x00").replace(".", self.dec).replace("\x00", self.th)

    def pct(self, x, digits=1):
        return self.n(x, digits) + "\u00a0%"

    def sec(self, x):
        return self.n(x, 2 if x < 1 else 1) + "\u00a0s"


def esc(s):
    return html.escape(str(s), quote=True)


# ---------------------------------------------------------------- icons
ICON = {
    "wave": '<path d="M2 12h2m3-5v10m4-14v18m4-12v6m4-9v12m3-6h0" stroke-linecap="round"/>',
    "doc": '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5M9 13h6M9 17h6"/>',
    "layers": '<path d="m12 3 9 5-9 5-9-5z"/><path d="m3 13 9 5 9-5"/>',
    "bolt": '<path d="M13 2 4 14h7l-1 8 9-12h-7z" stroke-linejoin="round"/>',
    "lock": '<rect x="4" y="10" width="16" height="11" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/><circle cx="12" cy="15.5" r="1.4"/>',
    "atom": '<circle cx="12" cy="12" r="1.6"/><ellipse cx="12" cy="12" rx="10" ry="4"/><ellipse cx="12" cy="12" rx="10" ry="4" transform="rotate(60 12 12)"/><ellipse cx="12" cy="12" rx="10" ry="4" transform="rotate(120 12 12)"/>',
    "download": '<path d="M12 3v12m-5-5 5 5 5-5M4 19h16" stroke-linecap="round" stroke-linejoin="round"/>',
    "cart": '<circle cx="9" cy="20" r="1.5"/><circle cx="18" cy="20" r="1.5"/><path d="M2 3h3l2.5 12h11.5l2-8H6.2" stroke-linejoin="round"/>',
    "mail": '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="m3 7 9 6 9-6"/>',
    "shield": '<path d="M12 3 4 6v6c0 5 3.5 8 8 9 4.5-1 8-4 8-9V6z"/><path d="m9 12 2 2 4-4"/>',
    "menu": '<path d="M4 7h16M4 12h16M4 17h16" stroke-linecap="round"/>',
    "os_win": '<rect x="3" y="4" width="18" height="12" rx="1.5"/><path d="M8 20h8M12 16v4"/>',
    "os_mac": '<rect x="4" y="5" width="16" height="10.5" rx="1.5"/><path d="M2 19h20l-1.5-3.5h-17z" stroke-linejoin="round"/>',
    "os_linux": '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="m7 9 3 3-3 3M12.5 15h4.5" stroke-linecap="round" stroke-linejoin="round"/>',
    "copy": '<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2"/>',
}


def icon(name, cls=""):
    return f'<svg class="{cls}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" aria-hidden="true">{ICON[name]}</svg>'


SOCIAL = {
    "telegram": ("soc-tg", "Telegram", '<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="#fff" d="M20.7 4.3 2.9 11.2c-1.2.5-1.2 1.2-.2 1.5l4.6 1.4 1.7 5.4c.2.6.4.8.9.8.4 0 .6-.2.9-.5l2.2-2.1 4.6 3.4c.8.5 1.4.2 1.6-.8l3-14.1c.3-1.3-.5-1.8-1.5-1.4zM9.4 14.4l8.4-5.3c.4-.2.7-.1.4.2l-7 6.3-.3 3z"/></svg>'),
    "x": ("soc-x", "X", '<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="#fff" d="M17.8 3h3.1l-6.8 7.7L22 21h-6.2l-4.9-6.3L5.3 21H2.2l7.3-8.3L2 3h6.4l4.4 5.8zm-1.1 16.2h1.7L7.4 4.7H5.6z"/></svg>'),
    "discord": ("soc-dc", "Discord", '<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="#fff" d="M19.3 5.3A17 17 0 0 0 15 4l-.5 1a15.7 15.7 0 0 0-5 0L9 4a17 17 0 0 0-4.3 1.3C2 9.4 1.3 13.3 1.6 17.2A17.2 17.2 0 0 0 6.9 20l1.1-1.8c-.6-.2-1.2-.5-1.7-.9l.4-.3a12.3 12.3 0 0 0 10.6 0l.4.3c-.5.4-1.1.7-1.7.9l1.1 1.8a17.1 17.1 0 0 0 5.3-2.8c.4-4.5-.7-8.4-3.1-11.9zM8.5 14.8c-1 0-1.9-1-1.9-2.2s.8-2.2 1.9-2.2 1.9 1 1.9 2.2-.8 2.2-1.9 2.2zm7 0c-1 0-1.9-1-1.9-2.2s.8-2.2 1.9-2.2 1.9 1 1.9 2.2-.8 2.2-1.9 2.2z"/></svg>'),
}


def social_row(cls="soc-row"):
    out = [f'<div class="{cls}">']
    for k in ("telegram", "x", "discord"):
        c, name, svg = SOCIAL[k]
        out.append(f'<a class="soc-btn {c}" href="{esc(SITE["community"][k])}" target="_blank" rel="noopener" aria-label="{name}" title="{name}">{svg}</a>')
    out.append("</div>")
    return "".join(out)


FLAGS = {
    "en": '<rect width="30" height="20" fill="#012169"/><path d="M0 0 30 20M30 0 0 20" stroke="#fff" stroke-width="4"/><path d="M0 0 30 20M30 0 0 20" stroke="#C8102E" stroke-width="1.6"/><path d="M15 0v20M0 10h30" stroke="#fff" stroke-width="6"/><path d="M15 0v20M0 10h30" stroke="#C8102E" stroke-width="3.4"/>',
    "zh": '<rect width="30" height="20" fill="#DE2910"/><path fill="#FFDE00" d="m6 3 .9 2.8h2.9L7.5 7.5l.9 2.8L6 8.6l-2.4 1.7.9-2.8-2.3-1.7h2.9z"/><circle cx="11" cy="2.6" r=".8" fill="#FFDE00"/><circle cx="12.6" cy="4.6" r=".8" fill="#FFDE00"/><circle cx="12.6" cy="7.2" r=".8" fill="#FFDE00"/><circle cx="11" cy="9.2" r=".8" fill="#FFDE00"/>',
    "hi": '<rect width="30" height="20" fill="#fff"/><rect width="30" height="6.67" fill="#FF9933"/><rect y="13.33" width="30" height="6.67" fill="#138808"/><circle cx="15" cy="10" r="2.6" fill="none" stroke="#000080" stroke-width=".7"/><circle cx="15" cy="10" r=".6" fill="#000080"/>',
    "es": '<rect width="30" height="20" fill="#AA151B"/><rect y="5" width="30" height="10" fill="#F1BF00"/>',
    "ar": '<rect width="30" height="20" fill="#006C35"/><path d="M8 12.5h14" stroke="#fff" stroke-width="1.2"/><path d="M9 8.3h12" stroke="#fff" stroke-width="1.6" stroke-dasharray="1.2 .7"/>',
    "fr": '<rect width="30" height="20" fill="#fff"/><rect width="10" height="20" fill="#002395"/><rect x="20" width="10" height="20" fill="#ED2939"/>',
    "bn": '<rect width="30" height="20" fill="#006A4E"/><circle cx="13.3" cy="10" r="5.2" fill="#F42A41"/>',
    "pt": '<rect width="30" height="20" fill="#009C3B"/><path d="M15 2.5 27 10 15 17.5 3 10z" fill="#FFDF00"/><circle cx="15" cy="10" r="4.2" fill="#002776"/><path d="M11 9.4c2.6-.6 5.4-.2 8 1.2" stroke="#fff" stroke-width=".7" fill="none"/>',
    "ru": '<rect width="30" height="20" fill="#fff"/><rect y="6.67" width="30" height="6.67" fill="#0039A6"/><rect y="13.33" width="30" height="6.67" fill="#D52B1E"/>',
    "id": '<rect width="30" height="20" fill="#fff"/><rect width="30" height="10" fill="#CE1126"/>',
}


def flag(code):
    return f'<svg class="flag" viewBox="0 0 30 20" aria-hidden="true">{FLAGS[code]}</svg>'


LANG_NAMES = {}


# ---------------------------------------------------------------- layout
def layout(L, page, body, extra_head="", scripts=""):
    code = L.code
    title = L("meta." + TITLE_KEY[page])
    full_title = title if page == "index" else f"{title} · NikPak"
    fonts = "https://fonts.googleapis.com/css2?family=Inter:wght@200;300;400;500;600&family=JetBrains+Mono:wght@400" + FONTS.get(code, "") + "&display=swap"
    dom = SITE["domain"].rstrip("/")
    alts = "".join(f'<link rel="alternate" hreflang="{HTML_LANG[c]}" href="{dom}/{c}/{page}.html">' for c in LANGS)
    alts += f'<link rel="alternate" hreflang="x-default" href="{dom}/en/{page}.html">'
    nav = []
    for item in NAV:
        if isinstance(item, str):
            p = item
            cur = ' aria-current="page"' if p == page else ""
            nav.append(f'<a href="{p}.html"{cur}>{esc(L("nav." + NAV_KEY[p]))}</a>')
        else:
            label_p, children = item
            on = ' class="on"' if any(c == page for c in children) else ""
            links = []
            for c in children:
                cur = ' aria-current="page"' if c == page else ""
                links.append(f'<a href="{c}.html"{cur}>{esc(L("nav." + NAV_KEY[c]))}</a>')
            nav.append(
                f'<details class="navdrop"{on}><summary>{esc(L("nav." + NAV_KEY[label_p]))}<i class="caret"></i></summary>'
                f'<div class="navdrop-menu">{"".join(links)}</div></details>')
    langs = []
    for c in LANGS:
        cur = ' aria-current="true"' if c == code else ""
        langs.append(f'<a href="../{c}/{page}.html" hreflang="{HTML_LANG[c]}" lang="{HTML_LANG[c]}" data-lang="{c}"{cur}>{flag(c)}<span>{esc(LANG_NAMES[c])}</span></a>')
    robots = '<meta name="robots" content="noindex">' if page == "thanks" else ""
    return f"""<!doctype html>
<html lang="{HTML_LANG[code]}" dir="{L('meta.dir')}" class="no-js">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{esc(full_title)}</title>
<meta name="description" content="{esc(L('meta.desc'))}">
<meta name="theme-color" content="#102658">
{robots}<link rel="canonical" href="{dom}/{code}/{page}.html">
{alts}
<meta property="og:title" content="{esc(full_title)}">
<meta property="og:description" content="{esc(L('meta.desc'))}">
<meta property="og:type" content="website">
<meta property="og:url" content="{dom}/{code}/{page}.html">
<meta property="og:image" content="{dom}/assets/img/nikpak-drop.webp">
<link rel="icon" href="../assets/favicon.ico" sizes="any">
<link rel="icon" href="../assets/img/favicon.png" type="image/png">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="{fonts}">
<link rel="stylesheet" href="../assets/css/site.css">
{extra_head}
</head>
<body data-page="{page}" data-lang="{code}">
<a class="skip" href="#main">{esc(L('nav.skip'))}</a>
<header class="top" id="top">
  <div class="wrap">
    <a class="brand" href="index.html"><img src="../assets/img/favicon.png" alt="" width="30" height="30">NikPak</a>
    <details class="apps">
      <summary aria-label="Our Apps"><svg class="apps-ico" viewBox="0 0 16 16" width="15" height="15" aria-hidden="true"><rect x="1.5" y="1.5" width="5" height="5" rx="1.3" fill="currentColor"/><rect x="9.5" y="1.5" width="5" height="5" rx="1.3" fill="currentColor"/><rect x="1.5" y="9.5" width="5" height="5" rx="1.3" fill="currentColor"/><rect x="9.5" y="9.5" width="5" height="5" rx="1.3" fill="currentColor"/></svg><span class="apps-name">Our Apps</span><svg class="chev" viewBox="0 0 12 12" width="10" height="10" aria-hidden="true"><path d="M2 4l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg></summary>
      <div class="apps-menu" role="list"><a class="apps-home" href="https://nikkoid.com">Nikkoid Network</a><hr><a href="https://nikkomander.nikkoid.com/index.html">Nikkomander</a><a href="https://nikedit.nikkoid.com/index.html">NikEDIT</a><a href="https://niksql.nikkoid.com/index.html">NikSQL</a><a href="https://nikpak.nikkoid.com/en/index.html" aria-current="true">NikPak</a><a href="https://hexawar.nikkoid.com/website/">Hexawar</a></div>
    </details>
    <nav class="nav" aria-label="{esc(L('nav.menu'))}">{''.join(nav)}</nav>
    <div class="lang" id="lang">
      <button type="button" aria-haspopup="true" aria-expanded="false" aria-label="{esc(L('nav.language'))}">{flag(code)}<span>{esc(LANG_NAMES[code])}</span><i class="caret"></i></button>
      <div class="lang-menu" role="menu"><span class="label">{esc(L('nav.language'))}</span>{''.join(langs)}</div>
    </div>
    <button type="button" class="menu-toggle" aria-label="{esc(L('nav.menu'))}" aria-expanded="false">{icon('menu')}</button>
  </div>
</header>
<main id="main">
{body}
</main>
{footer(L)}
<div class="toast" id="toast" role="status" aria-live="polite"></div>
<script src="../assets/js/site.js" defer></script>
{scripts}
</body>
</html>
"""


def footer(L):
    return f"""<footer>
  <div class="wrap">
    <div class="cols">
      <div>
        <a class="brand" href="index.html"><img src="../assets/img/favicon.png" alt="" width="30" height="30">NikPak</a>
        <p class="dim" style="margin-top:12px">{esc(L('common.tagline'))}</p>
        <p class="dim">{esc(L('common.made_with'))}</p>
      </div>
      <div>
        <span class="label">{esc(L('common.product'))}</span>
        <a href="features.html">{esc(L('nav.features'))}</a>
        <a href="benchmarks.html">{esc(L('nav.benchmarks'))}</a>
        <a href="index.html#download">{esc(L('common.download'))}</a>
        <a href="buy.html">{esc(L('common.buy'))}</a>
      </div>
      <div>
        <span class="label">{esc(L('common.support'))}</span>
        <a href="guide.html">{esc(L('nav.guide'))}</a>
        <a href="tech.html">{esc(L('nav.tech_full'))}</a>
        <a href="changelog.html">{esc(L('nav.changelog'))}</a>
        <a href="contact.html">{esc(L('nav.contact'))}</a>
        <a href="../LICENSE.txt">{esc(L('common.licence_terms'))}</a>
      </div>
      <div>
        <span class="label">{esc(L('common.community'))}</span>
        <p class="dim">{esc(L('common.community_text'))}</p>
        {social_row()}
      </div>
    </div>
    <div class="copy"><span>{esc(L('common.copyright'))}</span><span>nikpak.nikkoid.com</span></div>
  </div>
</footer>"""


# downloads, grouped by operating system: (os icon, group title, [(key, label, format), ...])
# "key" is the entry in SITE["downloads"]; file names and sizes come from site.json.
DL_GROUPS = [
    ("os_win", "Windows", [
        ("windows", "NikPak", "Installer (.exe)"),
        ("windows_unnik", "unNik", "Installer (.exe)"),
    ]),
    ("os_mac", "macOS", [
        ("macos", "NikPak + unNik", "ZIP archive (.zip)"),
    ]),
    ("os_linux", "Linux / Unix", [
        ("linux", "NikPak", "Tarball (.tar.gz)"),
        ("deb", "NikPak", "Debian / Ubuntu (.deb)"),
        ("deb_unnik", "unNik", "Debian / Ubuntu (.deb)"),
    ]),
]


def download_file(os_key):
    return SITE["downloads"][os_key].replace("{v}", SITE["version"])


def download_size(os_key):
    return SITE.get("download_sizes", {}).get(os_key, "").strip()


def download_btn(L):
    """The download list: one panel per OS, each file with its name, file name and size."""
    groups = []
    for os_icon, os_title, items in DL_GROUPS:
        rows = []
        for key, label, fmt in items:
            fn = download_file(key)
            meta = os.path.basename(fn)
            size = download_size(key)
            if size:
                meta += " · " + size
            rows.append(
                f'<a class="dl" href="../{esc(fn)}" download>'
                f'<span class="dl-what"><span>{esc(label)} · {esc(fmt)}</span><small>{esc(meta)}</small></span>'
                f'<span class="dl-get">{icon("download")}</span></a>')
        os_code = "win" if os_icon == "os_win" else ("mac" if os_icon == "os_mac" else "linux")
        groups.append(
            f'<section class="dl-group" data-os="{os_code}" aria-label="{esc(os_title)}">'
            f'<h3 class="dl-os-h">{icon(os_icon)}<span>{esc(os_title)}</span></h3>'
            f'<div class="dl-grid">{"".join(rows)}</div></section>')
    return f'<div class="dls">{"".join(groups)}</div>'


def hero_cta(L):
    """Two buttons in the hero: Free download (to #download) and Buy a licence."""
    return (f'<div class="btn-row hero-cta">'
            f'<a class="btn btn-primary" href="#download">{icon("download")}<span>{esc(L("common.download"))}</span></a>'
            f'<a class="btn" href="buy.html">{icon("cart")}<span>{esc(L("common.buy"))}</span></a>'
            f'</div>')


def buy_btn(L, primary=False):
    cls = "btn btn-primary" if primary else "btn"
    return f'<a class="{cls}" href="buy.html">{icon("cart")}{esc(L("common.buy"))}</a>'


def checks(items):
    return '<ul class="checks">' + "".join(f"<li>{i}</li>" for i in items) + "</ul>"


def ul(items, cls=""):
    c = f' class="{cls}"' if cls else ""
    return f"<ul{c}>" + "".join(f"<li>{i}</li>" for i in items) + "</ul>"


def ol_steps(items):
    return '<ol class="steps">' + "".join(f"<li>{i}</li>" for i in items) + "</ol>"


# ---------------------------------------------------------------- app window mock-up
def app_window(L):
    a = lambda k: esc(L("app." + k))
    social = "".join(f'<span class="{SOCIAL[k][0]}">{SOCIAL[k][2]}</span>' for k in ("telegram", "x", "discord"))
    return f"""<div class="appwin" data-state="idle" id="appwin" aria-hidden="true"
  data-of="{a('of')}">
  <div class="bar"><span class="label">{a('eval')}</span><span class="wbtns"><i>i</i><span>–</span><span>×</span></span></div>
  <div class="ttl">NikPak</div>
  <div class="ring">
    <svg viewBox="0 0 200 200">
      <defs><linearGradient id="rg" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#5FC4FF"/><stop offset="1" stop-color="#1E9BFF"/></linearGradient></defs>
      <circle cx="100" cy="100" r="96" fill="none" stroke="rgba(95,196,255,.14)" stroke-width="1.5"/>
      <circle class="track" cx="100" cy="100" r="86" fill="none" stroke="#122244" stroke-width="10" opacity="0"/>
      <circle class="dash" cx="100" cy="100" r="78" fill="rgba(4,11,34,.55)" stroke="rgba(95,196,255,.55)" stroke-width="1.6" stroke-dasharray="7 6"/>
      <circle class="prog" cx="100" cy="100" r="86" fill="none" stroke="url(#rg)" stroke-width="10" stroke-linecap="round" stroke-dasharray="0 541" transform="rotate(-90 100 100)"/>
      <g class="arrow" stroke="#5FC4FF" stroke-width="5" fill="none" stroke-linecap="round" stroke-linejoin="round" style="filter:drop-shadow(0 0 6px rgba(30,155,255,.9))">
        <path d="M100 58v54M80 94l20 20 20-20"/><path d="M70 112v18h60v-18"/></g>
      <g class="check" stroke="#3DDC97" stroke-width="7" fill="none" stroke-linecap="round" stroke-linejoin="round" opacity="0"><path d="m74 102 18 18 36-38"/></g>
    </svg>
    <div class="pct"><span><b>0</b><small>%</small></span></div>
  </div>
  <div class="msg">
    <div class="state s-idle"><div class="big">{a('drop')}</div><div class="links">{a('or')} <u>{a('choose_files')}</u> · <u>{a('choose_folder')}</u></div></div>
    <div class="state s-run"><div class="phase">{a('compressing')}</div><div class="file">holiday-photos</div><div class="meta"><span class="amt"></span></div></div>
    <div class="state s-done"><div class="phase ok">{a('done')}</div><div class="file">holiday-photos.nik</div><div class="meta">3.6 MB / 7.0 MB · 0.4 s</div></div>
  </div>
  <div class="save"><span class="label">{a('save_to')}</span>
    <div class="row"><div class="fld">C:\\Users\\Jane\\Desktop</div><div class="bbtn">{a('browse')}</div></div></div>
  <div class="foot"><span class="pill">{flag(L.code)}{esc(LANG_NAMES[L.code])} <i class="caret" style="width:0;height:0;border-inline:4px solid transparent;border-top:5px solid #1E9BFF"></i></span><span class="soc">{social}</span></div>
</div>"""


# ---------------------------------------------------------------- charts and tables
def bar_chart(L, sets):
    out = ['<div class="chart">']
    for s in sets:
        sizes = MAXSIZE[s]
        best = min(sizes)
        out.append(f'<div class="set"><h4>{esc(L("bench.sets." + s))}</h4><div class="bars">')
        for i, (tool, size) in enumerate(zip(MAX_TOOLS, sizes)):
            p = size / ORIGINAL[s] * 100
            me = " me" if i == 0 else ""
            out.append(f'<div class="bar{me}"><span class="n">{esc(tool)}</span><span class="t"><span class="f" data-w="{p:.1f}"></span></span><span class="v">{L.pct(p)}</span></div>')
        out.append("</div></div>")
    out.append("</div>")
    return "".join(out)


def table_max(L):
    head = f'<tr><th>{esc(L("bench.col_set"))}</th><th class="num">{esc(L("bench.col_original"))}</th>' + "".join(f'<th class="num">{esc(t)}</th>' for t in MAX_TOOLS) + "</tr>"
    rows = []
    for s in SETS:
        sizes = MAXSIZE[s]
        best = min(sizes)
        cells = "".join(f'<td class="num{" best" if v == best else ""}">{L.n(v)}</td>' for v in sizes)
        rows.append(f'<tr><td>{esc(L("bench.sets." + s))}</td><td class="num">{L.n(ORIGINAL[s])}</td>{cells}</tr>')
    return f'<div class="table-wrap"><table><thead>{head}</thead><tbody>{"".join(rows)}</tbody></table></div>'


def table_fast(L):
    head = f'<tr><th>{esc(L("bench.col_set"))}</th>' + "".join(f'<th class="num">{esc(t)}</th>' for t in FAST_TOOLS) + "</tr>"
    rows = []
    for s in SETS:
        vals = FAST[s]
        best = min(v[0] for v in vals)
        cells = "".join(f'<td class="num{" best" if v[0] == best else ""}">{L.pct(v[0])} · {L.sec(v[1])}</td>' for v in vals)
        rows.append(f'<tr><td>{esc(L("bench.sets." + s))}</td>{cells}</tr>')
    return f'<div class="table-wrap"><table><thead>{head}</thead><tbody>{"".join(rows)}</tbody></table></div>'


def table_general(L):
    head = f'<tr><th>{esc(L("bench.col_tool"))}</th><th class="num">{esc(L("bench.col_size"))}</th><th class="num">{esc(L("bench.col_pct"))}</th><th class="num">{esc(L("bench.col_create"))}</th><th class="num">{esc(L("bench.col_extract"))}</th></tr>'
    rows = []
    for tool, size, p, c, x, me in GENERAL:
        cls = ' class="me"' if me else ""
        rows.append(f'<tr{cls}><td>{esc(tool)}</td><td class="num">{L.n(size)}</td><td class="num">{L.pct(p)}</td><td class="num">{L.sec(c)}</td><td class="num">{L.sec(x)}</td></tr>')
    return f'<div class="table-wrap"><table><thead>{head}</thead><tbody>{"".join(rows)}</tbody></table></div>'


def tier_table(L):
    rows = []
    for i, (mn, price) in enumerate(TIERS):
        if i + 1 < len(TIERS):
            rng = f"{mn}–{TIERS[i + 1][0] - 1}"
        else:
            rng = L("buy.and_more").replace("{}", str(mn))
        rows.append(f'<tr data-min="{mn}"><td>{esc(rng)}</td><td class="price">${price}</td></tr>')
    return f'<div class="table-wrap"><table class="tiers" id="tiers"><thead><tr><th>{esc(L("buy.tier_seats"))}</th><th>{esc(L("buy.tier_price"))}</th></tr></thead><tbody>{"".join(rows)}</tbody></table></div>'


# ---------------------------------------------------------------- pages
def page_index(L):
    stats = "".join(f'<div class="panel stat reveal"><b>{esc(s["v"])}</b><span>{esc(s["l"])}</span></div>' for s in L.raw("home.stats"))
    icons = ["atom", "wave", "doc", "layers", "bolt", "lock"]  # quantum resistance first
    cards = "".join(f'<div class="panel card reveal"><div class="ico">{icon(icons[i])}</div><h3>{esc(c["t"])}</h3><p>{esc(c["d"])}</p></div>' for i, c in enumerate(L.raw("home.cards")))
    v = SITE["version"]
    body = f"""
<section class="hero">
  <div class="wrap">
    <div>
      <span class="label">{esc(L('home.kicker'))}</span>
      <h1>{L('home.title')}</h1>
      <p class="lead">{esc(L('home.lead'))}</p>
      {hero_cta(L)}
      <p class="small">{esc(L('common.download_note', v=v))}<br>{esc(L('home.free_note'))}</p>
    </div>
    <div>{app_window(L)}</div>
  </div>
</section>
<section id="download" class="downloads">
  <div class="wrap">
    <div class="section-head"><h2>{esc(L('common.download'))}</h2><p>{esc(L('common.download_note', v=v))}</p></div>
    {download_btn(L)}
  </div>
</section>
<section class="stats"><div class="wrap grid g4">{stats}</div></section>
<section>
  <div class="wrap">
    <div class="section-head"><h2>{esc(L('home.why_title'))}</h2><p>{esc(L('home.why_sub'))}</p></div>
    <div class="grid g3">{cards}</div>
  </div>
</section>
<section>
  <div class="wrap split">
    <div>
      <h2>{esc(L('home.chart_title'))}</h2>
      <p>{esc(L('home.chart_sub'))}</p>
      <p><a class="btn" href="benchmarks.html">{esc(L('common.see_benchmarks'))}</a></p>
    </div>
    <div class="panel">{bar_chart(L, ["bmp", "m48", "office", "gen"])}</div>
  </div>
</section>
<section>
  <div class="wrap split rev">
    <div>
      <h2>{esc(L('home.win_title'))}</h2>
      <p>{esc(L('home.win_text'))}</p>
      {checks([esc(x) for x in L.raw('home.win_points')])}
    </div>
    <div class="shots"><img class="shot" src="../assets/img/nikpak-drop.webp" alt="NikPak" width="500" height="560" loading="lazy"><img class="shot" src="../assets/img/nikpak-compress.webp" alt="NikPak" width="500" height="560" loading="lazy"></div>
  </div>
</section>
<section>
  <div class="wrap split">
    <div>
      <h2>{esc(L('home.unnik_title'))}</h2>
      <p>{esc(L('home.unnik_text'))}</p>
      {checks([esc(x) for x in L.raw('home.unnik_points')])}
    </div>
    <figure><img class="shot" src="../assets/img/unnik-browser.webp" alt="unNik" width="1120" height="720" loading="lazy"></figure>
  </div>
</section>
<section>
  <div class="wrap split rev">
    <div>
      <h2>{esc(L('home.skins_title'))}</h2>
      <p>{esc(L('home.skins_text'))}</p>
      <p><a class="btn" href="skins.html">{esc(L('home.skins_link'))}</a></p>
    </div>
    <a class="skin-strip" href="skins.html" aria-label="{esc(L('home.skins_link'))}">{skin_strip()}</a>
  </div>
</section>
<section>
  <div class="wrap split rev">
    <div>
      <h2>{esc(L('home.lang_title'))}</h2>
      <p>{esc(L('home.lang_text'))}</p>
      <p class="dim" style="font-size:1.05rem;line-height:2">{' · '.join(f'<span lang="{HTML_LANG[c]}">{esc(LANG_NAMES[c])}</span>' for c in LANGS)}</p>
    </div>
    <figure><img class="shot" src="../assets/img/lang-quad.webp" alt="" width="1066" height="1186" loading="lazy" style="max-width:520px"></figure>
  </div>
</section>
<section>
  <div class="wrap split">
    <div>
      <h2>{esc(L('home.cli_title'))}</h2>
      <p>{L('home.cli_text')}</p>
      <p><a class="btn" href="guide.html#cli">{esc(L('common.read_guide'))}</a></p>
    </div>
    <pre>nikpak a backup.nik C:\\Docs\\ Photos\\
nikpak a -p secret.nik Taxes2026\\
nikpak x backup.nik C:\\Restore\\
nikpak l backup.nik *.pdf
nikpak t secret.nik
nikpak mt song.wav tracker.mod photo.bmp</pre>
  </div>
</section>
<section>
  <div class="wrap">
    <div class="section-head"><h2>{esc(L('home.price_title'))}</h2><p>{esc(L('home.price_text'))}</p></div>
    <div class="grid g2" style="max-width:820px;margin:0 auto">
      <a class="panel price-card" href="buy.html?type=personal" style="color:inherit;text-decoration:none"><span class="label">{esc(L('home.price_personal'))}</span><div class="amount">$19</div><span class="dim">{esc(L('home.price_personal_note'))}</span></a>
      <a class="panel price-card featured" href="buy.html?type=company" style="color:inherit;text-decoration:none"><span class="label">{esc(L('home.price_company'))}</span><div class="amount"><small>{esc(L('common.from'))}</small> $5 <small>{esc(L('common.per_seat'))}</small></div><span class="dim">{esc(L('home.price_company_note'))}</span></a>
    </div>
  </div>
</section>
<section>
  <div class="wrap">
    <div class="panel-flat" style="max-width:820px;margin:0 auto">
      <h3>{esc(L('home.honest_title'))}</h3>
      <p class="dim" style="margin:0">{esc(L('home.honest_text'))} <a href="features.html#roadmap">{esc(L('features.roadmap_t'))} →</a></p>
    </div>
  </div>
</section>
<section>
  <div class="wrap">
    <div class="panel cta">
      <h2 class="glow-title">{esc(L('home.cta_title'))}</h2>
      <p class="dim">{esc(L('home.cta_text'))}</p>
      {hero_cta(L)}
    </div>
  </div>
</section>"""
    return layout(L, "index", body)


def page_head(L, t, sub):
    return f'<div class="page-head"><div class="wrap"><h1 class="glow-title">{esc(t)}</h1><div class="rule"></div><p style="margin-top:18px">{esc(sub)}</p></div></div>'


def doc_layout(L, sections):
    toc = "".join(f'<a href="#{sid}">{esc(t)}</a>' for sid, t, _ in sections)
    secs = "".join(f'<section id="{sid}"><h2>{esc(t)}</h2>{content}</section>' for sid, t, content in sections)
    return f'<div class="wrap doc"><aside class="toc"><span class="label">{esc(L("guide.toc"))}</span>{toc}</aside><div>{secs}</div></div>'


def page_features(L):
    f = lambda k: L("features." + k)
    levels = "".join(f'<tr><td><code>{esc(a)}</code></td><td>{esc(b)}</td></tr>' for a, b in L.raw("features.levels"))
    only = lambda s: esc(f("only_with").replace("{}", s))
    cmp = f"""<div class="table-wrap"><table>
<thead><tr><th></th><th>NikPak</th><th>RAR 5</th><th>7-Zip</th></tr></thead>
<tbody>
<tr><td>{esc(f('cmp_cipher'))}</td><td class="best">XChaCha20-Poly1305</td><td>AES-256-CBC</td><td>AES-256</td></tr>
<tr><td>{esc(f('cmp_auth'))}</td><td class="best">{esc(f('yes'))}</td><td>{esc(f('no'))}</td><td>{esc(f('no'))}</td></tr>
<tr><td>{esc(f('cmp_kdf'))}</td><td class="best">Argon2id, {esc(f('memory_hard'))}</td><td>PBKDF2-HMAC-SHA256</td><td>SHA-256, {esc(f('not_memory_hard'))}</td></tr>
<tr><td>{esc(f('cmp_names'))}</td><td class="best">{esc(f('always'))}</td><td>{only('-hp')}</td><td>{only('-mhe')}</td></tr>
</tbody></table></div>"""
    sections = [
        ("quantum", f("quantum_t"), f'<p>{esc(f("quantum_p1"))}</p><p>{esc(f("quantum_p2"))}</p>'),
        ("media", f("media_t"), f'<p>{esc(f("media_p1"))}</p><p>{esc(f("media_p2"))}</p>{checks(L.raw("features.media_list"))}'),
        ("office", f("office_t"), f'<p>{esc(f("office_p"))}</p>'),
        ("levels", f("engine_t"), f'<p>{esc(f("engine_p"))}</p><div class="table-wrap"><table><thead><tr><th>{esc(f("lv_col_level"))}</th><th>{esc(f("lv_col_use"))}</th></tr></thead><tbody>{levels}</tbody></table></div>'),
        ("encryption", f("crypto_t"), f'<p>{esc(f("crypto_p"))}</p>{checks([esc(x) for x in L.raw("features.crypto_list")])}{cmp}'),
        ("safety", f("safety_t"), checks([esc(x) for x in L.raw("features.safety_list")])),
        ("apps", f("apps_t"), f'<p>{esc(f("apps_p"))}</p><div class="shots"><img class="shot" src="../assets/img/nikpak-choose.webp" alt="NikPak" width="500" height="560" loading="lazy"><img class="shot" src="../assets/img/nikpak-about.webp" alt="NikPak" width="500" height="560" loading="lazy"></div><figure style="margin-top:24px"><img class="shot" src="../assets/img/unnik-dialogs.webp" alt="unNik" width="1393" height="560" loading="lazy"></figure>'),
        ("menu", f("menu_t"), f'<p>{esc(f("menu_p"))}</p>'),
        ("cli", f("cli_t"), f'<p>{esc(f("cli_p"))} <a href="tech.html#3-command-line-reference">{esc(L("nav.tech"))} →</a></p>'),
        ("skins", f("skins_t"), f'<p>{esc(f("skins_p"))} <a href="skins.html">{esc(L("home.skins_link"))} →</a></p><a class="skin-strip" href="skins.html" aria-label="{esc(L("home.skins_link"))}">{skin_strip()}</a>'),
        ("limits", f("limits_t"), ul([esc(x) for x in L.raw("features.limits_list")])),
        ("roadmap", f("roadmap_t"), ul(L.raw("features.roadmap_list"))),
    ]
    body = page_head(L, f("title"), f("sub")) + doc_layout(L, sections) + f'<section><div class="wrap"><div class="panel cta"><h2 class="glow-title">{esc(L("home.cta_title"))}</h2>{download_btn(L)}<div class="btn-row">{buy_btn(L)}</div></div></div></section>'
    return layout(L, "features", body)


def page_benchmarks(L):
    b = lambda k: L("bench." + k)
    sections = [
        ("method", b("method_t"), ul([esc(x) for x in L.raw("bench.method_list")])),
        ("chart", b("chart_t"), f'<p class="dim">{esc(b("chart_sub"))}</p><div class="panel">{bar_chart(L, SETS)}</div>'),
        ("maximum", b("max_t"), f'<p class="dim">{esc(b("max_note"))}</p>{table_max(L)}'),
        ("fast", b("fast_t"), f'<p class="dim">{esc(b("fast_note"))}</p>{table_fast(L)}'),
        ("general", b("gen_t"), f'<p class="dim">{esc(b("gen_note"))}</p>{table_general(L)}'),
        ("reading", b("reading_t"), ul(L.raw("bench.reading_list")) + f'<p class="dim">{b("source_note")}</p>'),
    ]
    body = page_head(L, b("title"), b("sub")) + doc_layout(L, sections)
    return layout(L, "benchmarks", body)


EXAMPLES = [
    "nikpak a backup.nik C:\\Docs\\*.txt Photos\\",
    "nikpak a -m5 -mt8 big.nik Projects\\",
    "nikpak a -p secret.nik Taxes2026\\",
    "nikpak x backup.nik C:\\Restore\\",
    "nikpak e backup.nik *.jpg C:\\Pics\\",
    "nikpak x backup.nik Docs/report.pdf",
    "nikpak l backup.nik *.pdf",
    "nikpak t secret.nik",
    "nikpak d backup.nik Temp *.bak",
    "nikpak mt song.wav tracker.mod photo.bmp",
]


def kbd_keys(ks):
    """'Alt+Left / Alt+Right' -> <kbd>Alt</kbd>+<kbd>Left</kbd> / ..."""
    parts = [k for k in re.split(r"(\s/\s|\+)", ks) if k.strip()]
    return " ".join(k if k.strip() in ("/", "+") else "<kbd>" + esc(k) + "</kbd>" for k in parts)


def page_guide(L):
    g = lambda k: L("guide." + k)
    keys = "".join(f'<tr><td>{kbd_keys(ks)}</td><td>{esc(a)}</td></tr>' for ks, a in L.raw("guide.keys"))
    cmds = "".join(f'<tr><td><code>{esc(c)}</code></td><td>{m}</td></tr>' for c, m in L.raw("guide.commands"))
    opts = "".join(f'<tr><td><code>{r}</code></td><td>{("<code>" + z + "</code>") if z else ""}</td><td>{esc(m)}</td></tr>' for r, z, m in L.raw("guide.options"))
    exits = "".join(f'<tr><td class="num">{c}</td><td>{esc(m)}</td><td>{esc(x)}</td></tr>' for c, m, x in L.raw("guide.exits"))
    comments = L.raw("guide.ex_comments")
    w = max(len(e) for e in EXAMPLES) + 2
    ex = "\n".join(f'{esc(e.ljust(w))}<span class="c"># {esc(c)}</span>' for e, c in zip(EXAMPLES, comments))
    sections = [
        ("install", g("install_t"), f'<p>{g("install_p1")}</p><p>{esc(g("install_p2"))}</p><p>{g("install_unix")}</p>{download_btn(L)}'),
        ("nikpak", g("win_t"), f'{ol_steps(L.raw("guide.win_steps"))}<div class="shots" style="margin:26px 0"><img class="shot" src="../assets/img/nikpak-drop.webp" alt="" width="500" height="560" loading="lazy"><img class="shot" src="../assets/img/nikpak-compress.webp" alt="" width="500" height="560" loading="lazy"></div><h3>{esc(g("win_more_t"))}</h3>{ul(L.raw("guide.win_more"))}<div class="note">{g("admin_note")}</div>'),
        ("skins", g("skins_t"), f'<p>{esc(g("skins_p"))}</p>{ol_steps(L.raw("guide.skins_steps"))}<div class="shots wide" style="margin:22px 0"><img class="shot" src="../assets/img/skins-menu.webp" alt="" loading="lazy"><img class="shot" src="../assets/img/skins-menu-bright.webp" alt="" loading="lazy"></div><div class="note">{g("skins_note")}</div><p><a class="btn" href="skins.html">{esc(L("home.skins_link"))}</a></p>'),
        ("unnik", g("unnik_t"), f'<p>{esc(g("unnik_p"))}</p><figure style="margin:20px 0"><img class="shot" src="../assets/img/unnik-pair.webp" alt="unNik" width="1393" height="560" loading="lazy"></figure>{ul(L.raw("guide.unnik_list"))}<h3>{esc(g("keys_t"))}</h3><div class="table-wrap"><table><thead><tr><th>{esc(g("keys_col_key"))}</th><th>{esc(g("keys_col_action"))}</th></tr></thead><tbody>{keys}</tbody></table></div>'),
        ("menu", g("menu_t"), f'<p>{esc(g("menu_p"))}</p>{ul(L.raw("guide.menu_list"))}<div class="note">{g("menu_note")}</div>'),
        ("register", g("reg_t"), f'{ol_steps(L.raw("guide.reg_steps"))}<div class="shots" style="margin:22px 0"><img class="shot" src="../assets/img/nikpak-reminder.webp" alt="" width="500" height="560" loading="lazy"><img class="shot" src="../assets/img/nikpak-register.webp" alt="" width="500" height="560" loading="lazy"></div><p>{g("reg_p")}</p><pre>nikpak register "Jane Doe" jane@example.com NPK1-...\nnikpak license\nnikpak unregister</pre>'),
        ("cli", g("cli_t"), f'<p>{g("cli_p")}</p><h3>{esc(g("cmd_t"))}</h3><div class="table-wrap"><table><thead><tr><th>{esc(g("col_cmd"))}</th><th>{esc(g("col_meaning"))}</th></tr></thead><tbody>{cmds}</tbody></table></div><h3>{esc(g("opt_t"))}</h3><div class="table-wrap"><table><thead><tr><th>{esc(g("col_rar"))}</th><th>{esc(g("col_zip"))}</th><th>{esc(g("col_meaning"))}</th></tr></thead><tbody>{opts}</tbody></table></div><h3>{esc(g("ex_t"))}</h3><pre>{ex}</pre>'),
        ("passwords", g("pwd_t"), f'<p>{g("pwd_p")}</p>{ul(L.raw("guide.pwd_list"))}<div class="note">{g("pwd_note")}</div>'),
        ("exit-codes", g("exit_t"), f'<div class="table-wrap"><table><thead><tr><th class="num">{esc(g("col_code"))}</th><th>{esc(g("col_meaning"))}</th><th>{esc(g("col_cause"))}</th></tr></thead><tbody>{exits}</tbody></table></div>'),
        ("total-commander", g("tc_t"), f'<p>{g("tc_p")}</p><div class="note">{g("tc_note")}</div>'),
        ("more", g("more_t"), f'<p>{g("more_p")}</p>'),
    ]
    body = page_head(L, g("title"), g("sub")) + doc_layout(L, sections)
    return layout(L, "guide", body)


def page_tech(L):
    toc_path = os.path.join(ROOT, f"tech-toc.{L.code}.html")
    if not os.path.exists(toc_path):
        toc_path = os.path.join(ROOT, "tech-toc.html")
    toc = open(toc_path, encoding="utf-8").read()
    body_path = os.path.join(ROOT, f"tech-body.{L.code}.html")
    if not os.path.exists(body_path):
        body_path = os.path.join(ROOT, "tech-body.html")
    content = open(body_path, encoding="utf-8").read()
    head = page_head(L, L("nav.tech_full"), L("guide.tech_sub"))
    body = f'<div class="wrap doc tech-doc"><aside class="toc"><span class="label">{esc(L("guide.toc"))}</span>{toc}</aside><div class="tech-content">{content}</div></div>'
    return layout(L, "tech", head + body)


COIN_SVG = {
    "BTC": '<svg viewBox="0 0 32 32" aria-hidden="true"><circle cx="16" cy="16" r="16" fill="#F7931A"/><text x="16" y="22" text-anchor="middle" font-size="18" font-weight="700" fill="#fff" font-family="Arial">₿</text></svg>',
    "ETH": '<svg viewBox="0 0 32 32" aria-hidden="true"><circle cx="16" cy="16" r="16" fill="#627EEA"/><path fill="#fff" d="M16 5v8.1l6.9 3.1zm0 0-6.9 11.2 6.9-3.1zm0 16.5V27l6.9-9.6zm0 5.5v-5.5l-6.9-4.1zm0-6.8 6.9-4-6.9-3.1zm-6.9-4 6.9 4v-7.1z" opacity=".95"/></svg>',
    "USDT": '<svg viewBox="0 0 32 32" aria-hidden="true"><circle cx="16" cy="16" r="16" fill="#26A17B"/><path fill="#fff" d="M17.9 17.4v-.1c-.1 0-.7.1-1.9.1-1 0-1.6 0-1.9-.1-3.6-.2-6.3-.8-6.3-1.6s2.7-1.4 6.3-1.6v2.5c.2 0 .9.1 1.9.1 1.2 0 1.8-.1 1.9-.1v-2.5c3.6.2 6.3.8 6.3 1.6s-2.7 1.4-6.3 1.7zm0-3.4v-2.2h5V8.5H9.1v3.3h5V14c-4.1.2-7.1 1-7.1 2s3 1.8 7.1 2v7.1h3.8V18c4-.2 7-1 7-2s-3-1.8-7-2z"/></svg>',
    "BNB": '<svg viewBox="0 0 32 32" aria-hidden="true"><circle cx="16" cy="16" r="16" fill="#F3BA2F"/><path fill="#fff" d="M12.1 14.1 16 10.2l3.9 3.9 2.3-2.3L16 5.6l-6.2 6.2zm-6.5 1.9 2.3-2.3 2.3 2.3-2.3 2.3zm6.5 1.9 3.9 3.9 3.9-3.9 2.3 2.3-6.2 6.2-6.2-6.2zm9.7-1.9 2.3-2.3 2.3 2.3-2.3 2.3zM18.4 16 16 13.6 13.6 16 16 18.4z"/></svg>',
    "USDC": '<svg viewBox="0 0 32 32" aria-hidden="true"><circle cx="16" cy="16" r="16" fill="#2775CA"/><text x="16" y="21.5" text-anchor="middle" font-size="15" font-weight="700" fill="#fff" font-family="Arial">$</text><circle cx="16" cy="16" r="10.5" fill="none" stroke="#fff" stroke-width="1.6" stroke-dasharray="14 6"/></svg>',
}


def page_buy(L):
    b = lambda k, **kw: L("buy." + k, **kw)
    P = SITE["payments"]
    need = "".join(f'<div class="panel-flat"><h3>{esc(n["t"])}</h3><p class="dim" style="margin:0">{esc(n["d"])}</p></div>' for n in L.raw("buy.need"))
    methods = []
    def method(key, title, desc, logos, checked=False):
        if not P.get(key, {}).get("enabled"):
            return
        lg = "".join(f"<span>{esc(x)}</span>" for x in logos)
        methods.append(f'<label class="method{" on" if checked else ""}" data-method="{key}"><input type="radio" name="method" value="{key}"{" checked" if checked else ""}><span class="mi"><b>{esc(title)}</b><span>{esc(desc)}</span></span><span class="logos">{lg}</span></label>')
    # card, PayPal and Paddle checkouts stay in shop/ but are not offered on the page;
    # crypto transfers have their own section below the order form
    method("googlepay", b("m_gpay"), b("m_gpay_d"), ["G Pay"], True)
    method("binancepay", b("m_binance"), b("m_binance_d"), ["Binance Pay"])
    coins = []
    for i, (sym, c) in enumerate(SITE["crypto"].items()):
        coins.append(f'<label><input type="radio" name="coin" value="{esc(sym)}"{" checked" if i == 0 else ""}><span class="opt">{COIN_SVG[c["symbol"]]}<b>{esc(c["symbol"])}</b><span>{esc(c["network"])}</span></span></label>')
    faq = "".join(f'<details class="faq"><summary>{esc(q["q"])}</summary><div>{esc(q["a"])}</div></details>' for q in L.raw("buy.faq"))
    email = SITE["contact_email"]
    crypto_steps = [s.replace("{email}", esc(email)) for s in L.raw("buy.crypto_steps")]
    # strings and settings used by buy.js
    cfg = {
        "lang": L.code,
        "email": email,
        "payments": {k: v for k, v in P.items()},
        "crypto": SITE["crypto"],
        "tiers": TIERS,
        "t": {k: L("buy." + k) for k in ["pay_btn", "continue_btn", "redirecting", "fill_first", "pay_error", "gpay_unavailable",
                                         "rate_loading", "rate_note", "rate_error", "stable_note", "network_warn", "saving",
                                         "mail_subject", "mail_body", "personal", "company"]},
        "copied": L("common.copied"),
    }
    cfg_json = json.dumps(cfg, ensure_ascii=False).replace("</", "<\\/")
    body = page_head(L, b("title"), b("sub")) + f"""
<div class="wrap">
  <div class="grid g3" style="margin-bottom:40px">{need}</div>
  <form class="shop" id="shop" novalidate onsubmit="return false">
    <div class="panel">
      <span class="label">{esc(b('order_t'))}</span>
      <h2 style="margin-top:8px">{esc(b('type_label'))}</h2>
      <div class="seg" role="radiogroup">
        <label><input type="radio" name="type" value="personal" checked><span class="opt"><b>{esc(b('personal'))} · <bdi>$19</bdi></b><span>{esc(b('personal_d'))}</span></span></label>
        <label><input type="radio" name="type" value="company"><span class="opt"><b>{esc(b('company'))} · {esc(L('common.from'))} <bdi>$5</bdi></b><span>{esc(b('company_d'))}</span></span></label>
      </div>
      <div id="personal-box" style="margin-top:18px">{checks([esc(x) for x in L.raw('buy.personal_list')])}</div>
      <div id="company-box" hidden style="margin-top:22px">
        <label class="label" for="seats">{esc(b('seats_label'))}</label>
        <div style="margin-top:10px" class="stepper"><button type="button" data-step="-1" aria-label="−">−</button><input id="seats" name="seats" type="number" min="1" max="100000" value="10" inputmode="numeric"><button type="button" data-step="1" aria-label="+">+</button></div>
        <input type="range" id="seats-range" min="1" max="600" value="10" aria-label="{esc(b('seats_label'))}">
        <div class="presets">{''.join(f'<button type="button" data-seats="{n}">{n}</button>' for n in (5, 10, 25, 50, 100, 200, 500, 1000))}</div>
        {checks([esc(x) for x in L.raw('buy.company_list')])}
        <h3 style="margin-top:22px">{esc(b('tiers_t'))}</h3>
        {tier_table(L)}
      </div>
    </div>
    <div class="panel" id="checkout">
      <div class="summary" aria-live="polite">
        <div class="ln"><span>{esc(b('per_seat_l'))}</span><b id="sum-per">$19</b></div>
        <div class="ln"><span>{esc(b('seats_l'))}</span><b id="sum-seats">1</b></div>
        <div class="ln total"><span>{esc(b('total_l'))}</span><b id="sum-total">$19</b></div>
        <div class="saving" id="sum-saving"></div>
      </div>
      <div class="form">
        <div class="field"><label class="label" for="holder">{esc(b('holder_label'))}</label><input type="text" id="holder" name="name" maxlength="120" autocomplete="name" placeholder="{esc(b('holder_ph'))}" required></div>
        <div class="field"><label class="label" for="email">{esc(b('email_label'))}</label><input type="email" id="email" name="email" maxlength="190" autocomplete="email" placeholder="{esc(b('email_ph'))}" required><small>{esc(b('holder_hint'))}</small></div>
      </div>
      <h3 style="margin-top:24px"><span class="label">{esc(b('pay_t'))}</span></h3>
      <div class="methods" id="methods">{''.join(methods)}</div>
      <div class="paybox">
        <div class="alert err" id="pay-error" role="alert"></div>
        <div id="gpay-container" data-for="googlepay"></div>
        <button type="button" class="btn btn-primary btn-block" id="pay-btn" data-for="binancepay" hidden>{icon('lock')}<span></span></button>
      </div>
      <p class="dim" style="font-size:.85rem;margin-top:14px">{icon('shield', 'inline')} {esc(b('secure'))}<br>{esc(b('tax_note'))}</p>
    </div>
  </form>

  <section id="crypto" style="padding-top:40px">
    <div class="panel">
      <h2>{esc(b('crypto_t'))}</h2>
      <p class="dim">{esc(b('crypto_intro'))}</p>
      <div class="coins" role="radiogroup" aria-label="{esc(b('coin_l'))}">{''.join(coins)}</div>
      <div class="crypto-grid">
        <div>
          <div class="field" style="margin-bottom:14px"><span class="label">{esc(b('amount_l'))}</span><div class="crypto-amount" id="c-amount">—</div><small class="dim" id="c-rate"></small></div>
          <div class="field" style="margin-bottom:14px"><span class="label">{esc(b('network_l'))}</span><div id="c-network"></div></div>
          <div class="field"><span class="label">{esc(b('address_l'))}</span><div class="addr" style="margin-top:6px"><code id="c-address"></code><button type="button" class="btn" id="c-copy">{icon('copy')}{esc(b('copy'))}</button></div></div>
          <div class="note" id="c-warn" style="margin-top:16px"></div>
        </div>
        <div class="qr" id="c-qr" aria-hidden="true"></div>
      </div>
      <h3 style="margin-top:26px">{esc(b('crypto_steps_t'))}</h3>
      {ol_steps(crypto_steps)}
      <div class="btn-row"><a class="btn btn-primary" id="c-mail" href="mailto:{esc(email)}">{icon('mail')}{esc(b('mail_btn'))}</a><a class="btn" id="c-form" href="contact.html?topic=crypto">{esc(b('form_btn'))}</a></div>
    </div>
  </section>

  <section id="faq">
    <h2 class="center">{esc(b('faq_t'))}</h2>
    <div style="max-width:820px;margin:0 auto">{faq}</div>
  </section>
</div>
<script type="application/json" id="buy-config">{cfg_json}</script>"""
    scripts = '<script src="../assets/js/qrcode.min.js" defer></script>\n<script src="../assets/js/buy.js" defer></script>'
    if P.get("googlepay", {}).get("enabled"):
        scripts += '\n<script async src="https://pay.google.com/gp/p/js/pay.js" onload="window.nikpakGPayLoaded&&window.nikpakGPayLoaded()"></script>'
    return layout(L, "buy", body, scripts=scripts)


def page_contact(L):
    c = lambda k, **kw: L("contact." + k, **kw)
    email = SITE["contact_email"]
    topics = "".join(f'<option value="{k}">{esc(v)}</option>' for k, v in L.raw("contact.topics").items())
    coins = "".join(f'<option value="{esc(s)}">{esc(v["symbol"])} · {esc(v["network"])}</option>' for s, v in SITE["crypto"].items())
    cfg = {"lang": L.code, "email": email, "t": {k: c(k) for k in ["sending", "sent", "error", "invalid", "send"]}, "topics_en": EN["contact"]["topics"]}
    cfg_json = json.dumps(cfg, ensure_ascii=False).replace("</", "<\\/")
    body = page_head(L, c("title"), c("sub")) + f"""
<div class="wrap shop" style="padding-bottom:40px">
  <div class="panel">
    <div class="alert ok" id="c-ok" role="status">{esc(c('sent'))}</div>
    <div class="alert err" id="c-err" role="alert"></div>
    <form class="form" id="contact" method="post" action="../contact.php" novalidate>
      <input type="hidden" name="lang" value="{L.code}">
      <input type="hidden" name="ts" id="ts" value="">
      <div class="hp" aria-hidden="true"><label>Website <input type="text" name="website" tabindex="-1" autocomplete="off"></label></div>
      <div class="row2">
        <div class="field"><label class="label" for="f-name">{esc(c('name'))}</label><input type="text" id="f-name" name="name" maxlength="120" autocomplete="name" required></div>
        <div class="field"><label class="label" for="f-email">{esc(c('email'))}</label><input type="email" id="f-email" name="email" maxlength="190" autocomplete="email" required></div>
      </div>
      <div class="field"><label class="label" for="f-topic">{esc(c('topic'))}</label><select id="f-topic" name="topic">{topics}</select></div>
      <fieldset id="crypto-fields" hidden style="border:1px solid var(--line);border-radius:14px;padding:16px;margin:0;display:grid;gap:14px">
        <div class="row2">
          <div class="field"><label class="label" for="f-coin">{esc(c('coin'))}</label><select id="f-coin" name="coin">{coins}</select></div>
          <div class="field"><label class="label" for="f-amount">{esc(c('amount'))}</label><input type="text" id="f-amount" name="amount" maxlength="60"></div>
        </div>
        <div class="field"><label class="label" for="f-txid">{esc(c('txid'))}</label><input type="text" id="f-txid" name="txid" maxlength="200" spellcheck="false" style="font-family:var(--mono)"></div>
        <div class="row2">
          <div class="field"><label class="label" for="f-licence">{esc(c('licence'))}</label><select id="f-licence" name="licence"><option value="personal">{esc(L('buy.personal'))}</option><option value="company">{esc(L('buy.company'))}</option></select></div>
          <div class="field"><label class="label" for="f-seats">{esc(c('seats'))}</label><input type="number" id="f-seats" name="seats" min="1" value="1"></div>
        </div>
        <div class="field"><label class="label" for="f-holder">{esc(c('holder'))}</label><input type="text" id="f-holder" name="holder" maxlength="120"></div>
      </fieldset>
      <div class="field"><label class="label" for="f-subject">{esc(c('subject'))}</label><input type="text" id="f-subject" name="subject" maxlength="150" required>
        <small>{esc(c('subject_sent'))} <span class="subject-preview" id="subject-preview">NikPak</span></small></div>
      <div class="field"><label class="label" for="f-message">{esc(c('message'))}</label><textarea id="f-message" name="message" maxlength="8000" required></textarea></div>
      <button type="submit" class="btn btn-primary" id="f-send">{icon('mail')}<span>{esc(c('send'))}</span></button>
      <p class="dim" style="font-size:.85rem;margin:0">{esc(c('privacy'))}</p>
    </form>
  </div>
  <div style="display:grid;gap:22px">
    <div class="info"><h3 style="margin-bottom:.4em">{esc(c('rule_t'))}</h3><p style="margin:0">{c('rule', email=esc(email))}</p></div>
    <div class="panel-flat"><h3>{esc(c('crypto_t'))}</h3><p class="dim" style="margin:0">{c('crypto_p')}</p></div>
    <div class="panel-flat"><h3>{esc(c('direct_t'))}</h3><p style="margin:0"><a href="mailto:{esc(email)}?subject=NikPak%20-%20">{esc(email)}</a></p></div>
    <div class="panel-flat"><h3>{esc(c('community_t'))}</h3><p class="dim">{esc(c('community_p'))}</p>{social_row()}</div>
  </div>
</div>
<script type="application/json" id="contact-config">{cfg_json}</script>"""
    return layout(L, "contact", body, scripts='<script src="../assets/js/contact.js" defer></script>')


def page_thanks(L):
    t = lambda k: L("thanks." + k)
    body = f"""
<div class="wrap" style="padding:80px 16px 40px;max-width:760px">
  <div class="panel center" id="thanks" data-status="ok">
    <svg viewBox="0 0 120 120" width="120" height="120" style="margin:0 auto 18px" aria-hidden="true"><circle cx="60" cy="60" r="52" fill="none" stroke="#122244" stroke-width="8"/><circle id="th-ring" cx="60" cy="60" r="52" fill="none" stroke="#3DDC97" stroke-width="8" stroke-linecap="round" stroke-dasharray="327 327" transform="rotate(-90 60 60)"/><path id="th-mark" d="m40 62 13 13 27-29" fill="none" stroke="#3DDC97" stroke-width="8" stroke-linecap="round" stroke-linejoin="round"/></svg>
    <div data-show="ok"><h1 class="glow-title" style="font-size:2.2rem">{esc(t('ok_t'))}</h1><p class="dim">{esc(t('ok_p'))}</p><p>{t('next')}</p></div>
    <div data-show="pending" hidden><h1 class="glow-title" style="font-size:2.2rem">{esc(t('pending_t'))}</h1><p class="dim">{esc(t('pending_p'))}</p><p>{t('next')}</p></div>
    <div data-show="cancel" hidden><h1 class="glow-title" style="font-size:2.2rem">{esc(t('cancel_t'))}</h1><p class="dim">{esc(t('cancel_p'))}</p></div>
    <div data-show="error" hidden><h1 class="glow-title" style="font-size:2.2rem">{esc(t('error_t'))}</h1><p class="dim">{esc(t('error_p'))}</p></div>
    <div class="btn-row" style="justify-content:center;margin-top:24px"><a class="btn" href="buy.html">{esc(t('back'))}</a><a class="btn" href="contact.html">{esc(t('contact'))}</a></div>
  </div>
</div>"""
    return layout(L, "thanks", body)


def page_changelog(L):
    c = lambda k: L("changelog." + k)
    items = "".join(
        f'<div class="cl-item"><div class="cl-head"><span class="cl-ver">{esc(v)}</span><span class="cl-date">{esc(d)}</span></div><p>{esc(s)}</p></div>'
        for v, d, s in CHANGELOG)
    body = page_head(L, c("title"), c("sub")) + f'<div class="wrap"><div class="changelog">{items}</div></div>'
    return layout(L, "changelog", body)


# the strip on the home and features pages: the default pair, two painted
# skins of 1.6 and four classic ones
SKIN_STRIP = ["nikkoid", "nikkoid-light", "moon", "pattaya", "sushi", "chess", "dracula", "8bit"]


def skin_strip():
    names = {s["id"]: s["name"] for s in SKINS}
    return "".join(f'<img src="../assets/img/skins/{i}-thumb.webp" alt="{esc(names[i])}" title="{esc(names[i])}" width="250" height="280" loading="lazy">' for i in SKIN_STRIP)


SKIN_EXAMPLE = """{
  "id": "ocean-glass",
  "name": "Ocean Glass",
  "dark": true,
  "vars": {
    "bg": "#06121f",
    "panel": "#0b2236",
    "dialog": "#0d2740",
    "fg": "#d7e6f2",
    "fg-dim": "#7897ad",
    "accent": "#4cc2ff",
    "accent-fg": "#031521",
    "border": "#1b4160",
    "danger": "#ff6b6b",
    "ok": "#6be3a4",
    "radius": "14px"
  }
}"""


def page_skins(L):
    k = lambda key, **kw: L("skins." + key, **kw)
    n_dark = sum(1 for s in SKINS if s["dark"])
    tiles = []
    for s in SKINS:
        tag = k("tag_dark") if s["dark"] else k("tag_bright")
        new = f'<i class="tag n">{esc(k("tag_new"))}</i>' if s.get("new") else ""
        tiles.append(
            f'<figure class="skin-tile reveal" data-dark="{1 if s["dark"] else 0}">'
            f'<a href="../assets/img/skins/{s["id"]}.webp" data-name="{esc(s["name"])}">'
            f'<img src="../assets/img/skins/{s["id"]}-thumb.webp" alt="NikPak · {esc(s["name"])}" width="250" height="280" loading="lazy"></a>'
            f'<figcaption><span lang="en">{esc(s["name"])}</span><span class="tags">{new}<i class="tag {"d" if s["dark"] else "b"}">{esc(tag)}</i></span></figcaption></figure>')
    # the painted skins of 1.6, shown above the full gallery
    new_strip = "".join(
        f'<a href="#gallery"><img src="../assets/img/skins/{s["id"]}-thumb.webp" alt="NikPak · {esc(s["name"])}" width="250" height="280" loading="lazy"></a>'
        for s in [s for s in SKINS if s.get("new")][:8])
    band = f"""
<section>
  <div class="wrap">
    <div class="section-head"><h2>{esc(k('new_t'))}</h2><p>{esc(k('new_p'))}</p></div>
    <div class="skin-strip">{new_strip}</div>
  </div>
</section>"""
    chips = (f'<div class="chips" role="group" aria-label="{esc(k("gallery_t"))}">'
             f'<button type="button" class="chip" data-filter="all" aria-pressed="true">{esc(k("all"))} <b>{len(SKINS)}</b></button>'
             f'<button type="button" class="chip" data-filter="1" aria-pressed="false">{esc(k("dark"))} <b>{n_dark}</b></button>'
             f'<button type="button" class="chip" data-filter="0" aria-pressed="false">{esc(k("bright"))} <b>{len(SKINS) - n_dark}</b></button></div>')
    body = page_head(L, k("title"), k("sub")) + f"""
<section>
  <div class="wrap split">
    <div>
      <h2>{esc(k('how_t'))}</h2>
      {checks(L.raw('skins.how_list'))}
    </div>
    <div class="shots wide"><img class="shot" src="../assets/img/skins-menu.webp" alt="" loading="lazy"><img class="shot" src="../assets/img/skins-menu-bright.webp" alt="" loading="lazy"></div>
  </div>
</section>{band}
<section id="gallery">
  <div class="wrap">
    <div class="section-head"><h2>{esc(k('gallery_t'))}</h2><p>{esc(k('gallery_sub'))}</p></div>
    {chips}
    <div class="skin-grid" id="skin-grid">{''.join(tiles)}</div>
  </div>
</section>
<section>
  <div class="wrap split rev">
    <div>
      <h2>{esc(k('unnik_t'))}</h2>
      <p>{esc(k('unnik_p'))}</p>
    </div>
    <figure><img class="shot" src="../assets/img/unnik-skins.webp" alt="unNik" width="1120" height="720" loading="lazy"></figure>
  </div>
</section>
<section id="custom">
  <div class="wrap split">
    <div>
      <h2>{esc(k('custom_t'))}</h2>
      <p>{k('custom_p')}</p>
      <div class="note">{esc(k('limits'))}</div>
    </div>
    <pre lang="en" dir="ltr">{esc(SKIN_EXAMPLE)}</pre>
  </div>
</section>
<section>
  <div class="wrap">
    <div class="panel cta">
      <h2 class="glow-title">{esc(L('home.cta_title'))}</h2>
      <p class="dim">{esc(L('home.cta_text'))}</p>
      {download_btn(L)}<div class="btn-row">{buy_btn(L)}</div>
    </div>
  </div>
</section>
<dialog class="skin-view" id="skin-view" aria-label="{esc(k('gallery_t'))}">
  <button type="button" class="sv-close" data-close aria-label="{esc(k('close'))}">×</button>
  <button type="button" class="sv-nav sv-prev" data-step="-1" aria-label="←">‹</button>
  <figure><img alt="" width="500" height="560"><figcaption></figcaption></figure>
  <button type="button" class="sv-nav sv-next" data-step="1" aria-label="→">›</button>
</dialog>"""
    return layout(L, "skins", body)


BUILDERS = {"index": page_index, "features": page_features, "benchmarks": page_benchmarks, "guide": page_guide,
            "tech": page_tech, "skins": page_skins, "buy": page_buy, "contact": page_contact, "changelog": page_changelog,
            "thanks": page_thanks}


def root_index():
    links = "".join(f'<a href="{c}/index.html" hreflang="{HTML_LANG[c]}" lang="{HTML_LANG[c]}">{flag(c)}<span>{esc(LANG_NAMES[c])}</span></a>' for c in LANGS)
    codes = json.dumps(LANGS)
    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>NikPak</title>
<meta name="description" content="{esc(EN['meta']['desc'])}">
<link rel="icon" href="assets/favicon.ico" sizes="any">
<link rel="stylesheet" href="assets/css/site.css">
<script>
(function () {{
  var codes = {codes}, pick = null;
  try {{ pick = localStorage.getItem('nikpak_lang'); }} catch (e) {{}}
  if (codes.indexOf(pick) < 0) {{
    pick = 'en';
    var ls = navigator.languages || [navigator.language || 'en'];
    for (var i = 0; i < ls.length; i++) {{
      var c = String(ls[i]).toLowerCase().split('-')[0];
      if (codes.indexOf(c) >= 0) {{ pick = c; break; }}
    }}
  }}
  location.replace(pick + '/index.html' + location.search + location.hash);
}})();
</script>
</head>
<body>
<main class="wrap" style="padding:80px 16px;max-width:520px">
  <div class="panel"><h1 class="glow-title center" style="font-size:2.4rem">NikPak</h1>
  <div class="lang-menu" style="display:block;position:static;box-shadow:none">{links}</div></div>
</main>
</body>
</html>
"""


def paddle_page(pd):
    env = "sandbox" if pd.get("environment", "sandbox") == "sandbox" else "production"
    codes = json.dumps(LANGS)
    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>NikPak · Paddle</title>
<link rel="icon" href="assets/favicon.ico" sizes="any">
<link rel="stylesheet" href="assets/css/site.css">
</head>
<body>
<main class="wrap" style="padding:80px 16px;max-width:560px">
  <div class="panel center"><h1 class="glow-title" style="font-size:2.2rem">NikPak</h1><div class="rule"></div><p class="dim" style="margin-top:18px">Paddle…</p>
  <p><a class="btn" id="back" href="en/buy.html">←</a></p></div>
</main>
<script src="https://cdn.paddle.com/paddle/v2/paddle.js"></script>
<script>
(function () {{
  var q = new URLSearchParams(location.search), codes = {codes};
  var lang = codes.indexOf(q.get('lang')) >= 0 ? q.get('lang') : 'en';
  var base = location.href.split('pay.html')[0];
  document.getElementById('back').href = lang + '/buy.html';
  {"Paddle.Environment.set('sandbox');" if env == "sandbox" else ""}
  Paddle.Initialize({{
    token: {json.dumps(pd.get("client_token", ""))},
    checkout: {{ settings: {{ displayMode: 'overlay', theme: 'dark', locale: lang, successUrl: base + lang + '/thanks.html?status=ok&provider=paddle' }} }},
    eventCallback: function (e) {{ if (e && e.name === 'checkout.closed') location.href = base + lang + '/thanks.html?status=cancel&provider=paddle'; }}
  }});
}})();
</script>
</body>
</html>
"""


def main():
    global LANG_NAMES
    for c in LANGS:
        LANG_NAMES[c] = Lang(c)("meta.lang_name") if os.path.exists(os.path.join(ROOT, "i18n", c + ".json")) else c
    if os.path.isdir(DIST):
        shutil.rmtree(DIST)
    os.makedirs(DIST)
    shutil.copytree(os.path.join(ROOT, "static"), os.path.join(DIST, "assets"))
    shutil.copytree(os.path.join(ROOT, "php"), DIST, dirs_exist_ok=True)
    os.makedirs(os.path.join(DIST, "downloads"), exist_ok=True)
    with open(os.path.join(DIST, "downloads", "PUT-THE-SETUP-HERE.txt"), "w", encoding="utf-8") as f:
        f.write("Upload these files into this folder (see site.json, downloads):\n" + "".join(f"  {os.path.basename(download_file(k))}\n" for _, _, items in DL_GROUPS for k, _, _ in items))
    lic = os.path.join(ROOT, "..", "LICENSE.txt")
    if os.path.exists(lic):
        shutil.copy(lic, os.path.join(DIST, "LICENSE.txt"))
    # settings shared with the PHP scripts (languages, contact e-mail, prices)
    with open(os.path.join(DIST, "site-public.json"), "w", encoding="utf-8") as f:
        json.dump({"langs": LANGS, "contact_email": SITE["contact_email"], "mail_from": SITE.get("mail_from", ""), "domain": SITE["domain"],
                   "crypto": SITE["crypto"], "payments": {k: bool(v.get("enabled")) for k, v in SITE["payments"].items()}}, f, ensure_ascii=False, indent=1)
    count = 0
    for c in LANGS:
        if not os.path.exists(os.path.join(ROOT, "i18n", c + ".json")):
            print(f"warning: i18n/{c}.json missing, using English", file=sys.stderr)
        L = Lang(c)
        os.makedirs(os.path.join(DIST, c), exist_ok=True)
        for p in PAGES:
            with open(os.path.join(DIST, c, p + ".html"), "w", encoding="utf-8") as f:
                f.write(BUILDERS[p](L))
            count += 1
    with open(os.path.join(DIST, "index.html"), "w", encoding="utf-8") as f:
        f.write(root_index())
    # Paddle "default payment link": Paddle opens its checkout here for ?_ptxn=txn_...
    pd = SITE["payments"].get("paddle", {})
    if pd.get("enabled"):
        with open(os.path.join(DIST, "pay.html"), "w", encoding="utf-8") as f:
            f.write(paddle_page(pd))
    # sitemap
    dom = SITE["domain"].rstrip("/")
    urls = "".join(f"<url><loc>{dom}/{c}/{p}.html</loc></url>" for c in LANGS for p in PAGES if p != "thanks")
    with open(os.path.join(DIST, "sitemap.xml"), "w", encoding="utf-8") as f:
        f.write(f'<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">{urls}</urlset>\n')
    with open(os.path.join(DIST, "robots.txt"), "w", encoding="utf-8") as f:
        f.write(f"User-agent: *\nDisallow: /shop/\nSitemap: {dom}/sitemap.xml\n")
    miss = sorted(set(MISSING))
    if miss:
        print(f"{len(miss)} texts fall back to English:", file=sys.stderr)
        for c, k in miss[:40]:
            print(f"  {c}: {k}", file=sys.stderr)
    print(f"built {count} pages in {DIST}")


if __name__ == "__main__":
    main()
