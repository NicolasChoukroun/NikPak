#!/usr/bin/env python3
"""
Build the NikPak documentation: one PDF per language, holding the user manual and
the technical guide in a single book.

    python build_docs.py                  all eleven languages
    python build_docs.py -l fr de         just those

Each language's text lives in documents/text/<code>.py, defining MANUAL and TECH,
so a translation can be edited without scrolling through the others.

Requirements: Python 3.9+, python-docx, Pillow, LibreOffice (soffice) for the PDF.
The figures are in documents/images.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOC = Path(__file__).resolve().parent
IMAGES = DOC / "images"
TEXT = DOC / "text"

# The book engine is shared by every Nikkoid project, like NikPak's library is:
# D:\Claude Projects\nkdocs\nkdocs.py. One layout, one fix, seven manuals.
_ENGINE = ROOT.parent / "nkdocs"
if not (_ENGINE / "nkdocs.py").exists():
    raise SystemExit(f"The documentation engine is missing: {_ENGINE / 'nkdocs.py'}")
sys.path.insert(0, str(_ENGINE))
from nkdocs import Book, LANGS  # noqa: E402

PRODUCT = "NikPak"
VERSION = "1.13.2"

TITLES = {
    "en": ("Part I · User manual", "Everything NikEdit does, window by window.",
           "Part II · Technical guide", "How it is built, what it stores, and how to rebuild it."),
    "fr": ("Partie I · Manuel de l'utilisateur", "Tout ce que fait NikEdit, fenêtre par fenêtre.",
           "Partie II · Guide technique", "Comment il est construit, ce qu'il enregistre, et comment le reconstruire."),
    "de": ("Teil I · Benutzerhandbuch", "Alles, was NikEdit tut, Fenster für Fenster.",
           "Teil II · Technischer Leitfaden", "Wie es gebaut ist, was es speichert und wie man es neu baut."),
    "es": ("Parte I · Manual del usuario", "Todo lo que hace NikEdit, ventana por ventana.",
           "Parte II · Guía técnica", "Cómo está construido, qué guarda y cómo volver a compilarlo."),
    "pt": ("Parte I · Manual do utilizador", "Tudo o que o NikEdit faz, janela a janela.",
           "Parte II · Guia técnica", "Como é construído, o que guarda e como o reconstruir."),
    "ru": ("Часть I · Руководство пользователя", "Всё, что делает NikEdit, окно за окном.",
           "Часть II · Техническое руководство", "Как он устроен, что хранит и как его собрать заново."),
    "ja": ("第 I 部 · ユーザーマニュアル", "NikEdit のすべての機能を、画面ごとに。",
           "第 II 部 · テクニカルガイド", "仕組み、保存するもの、再ビルドの方法。"),
    "zh": ("第一部分 · 用户手册", "NikEdit 的每一项功能，逐个窗口说明。",
           "第二部分 · 技术指南", "如何构建、保存了什么，以及如何重新编译。"),
    "hi": ("भाग I · उपयोगकर्ता पुस्तिका", "NikEdit जो कुछ करता है, विंडो दर विंडो।",
           "भाग II · तकनीकी मार्गदर्शिका", "यह कैसे बना है, क्या सहेजता है, और इसे फिर से कैसे बनाएँ।"),
    "ar": ("الجزء الأول · دليل المستخدم", "كل ما يفعله NikEdit، نافذة بنافذة.",
           "الجزء الثاني · الدليل التقني", "كيف بُني، وما يحفظه، وكيف يُعاد بناؤه."),
    "th": ("ภาคที่ 1 · คู่มือผู้ใช้", "ทุกสิ่งที่ NikEdit ทำได้ ทีละหน้าต่าง",
           "ภาคที่ 2 · คู่มือทางเทคนิค", "สร้างอย่างไร เก็บอะไร และจะสร้างใหม่ได้อย่างไร"),
}

MANUAL: dict[str, list] = {}
TECH: dict[str, list] = {}


def read_version() -> str:
    for rel, pat in (("Cargo.toml", r'^version\s*=\s*"([^"]+)"'),):
        try:
            m = re.search(pat, (ROOT / rel).read_text(encoding="utf-8"))
            if m:
                return m.group(1)
        except OSError:
            pass
    return VERSION


def load_texts() -> None:
    """documents/text/<code>.py -> MANUAL[code], TECH[code]."""
    if not TEXT.is_dir():
        raise SystemExit(f"There is no {TEXT}")
    for f in sorted(TEXT.glob("*.py")):
        if f.stem.startswith("_"):
            continue
        spec = importlib.util.spec_from_file_location(f"nkedit_text_{f.stem}", f)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        if getattr(mod, "MANUAL", None):
            MANUAL[f.stem] = mod.MANUAL
        if getattr(mod, "TECH", None):
            TECH[f.stem] = mod.TECH


def build_part(code: str) -> list:
    t = TITLES[code]
    return ([("part", t[0], t[1])] + MANUAL[code] +
            [("part", t[2], t[3])] + TECH[code])


def main() -> int:
    ap = argparse.ArgumentParser(description="Build the NikEdit manuals.")
    ap.add_argument("-l", "--langs", nargs="*", default=None)
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--out", default=str(DOC / "manuals"))
    args = ap.parse_args()

    load_texts()
    codes = [c for c, _, _ in LANGS]
    if args.list:
        for c in codes:
            print(f"{c:3s} manual: {'yes' if c in MANUAL else 'no ':3s} "
                  f"technical: {'yes' if c in TECH else 'no'}")
        return 0

    wanted = args.langs or codes
    for name, table in (("manual", MANUAL), ("technical guide", TECH)):
        missing = [c for c in wanted if c not in table]
        if missing:
            print(f"No {name} for: {', '.join(missing)}", file=sys.stderr)
            return 2
        shape = [b[0] for b in table["en"]]
        for c in wanted:
            got = [b[0] for b in table[c]]
            if got != shape:
                print(f"{c}: the {name} does not follow the English structure "
                      f"({len(got)} blocks against {len(shape)}).", file=sys.stderr)
                return 2

    version = read_version()
    book = Book(product=PRODUCT,
                title="User Manual and Technical Guide",
                version=version,
                out_dir=args.out,
                website="nikpak.nikkoid.com",
                footer_note=f"NikPak {version} · Nikkoid LLC")

    made = 0
    for code in wanted:
        out = book.build(code, build_part(code), images=IMAGES, quiet=True)
        if out:
            made += 1
            print(f"   {out.name:40s} {out.stat().st_size / 1024:7.0f} KB")
    print(f"{made} document(s) in {Path(args.out)}")
    return 0 if made == len(wanted) else 1


if __name__ == "__main__":
    raise SystemExit(main())
