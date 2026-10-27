#!/usr/bin/env python3
"""Checks i18n/<lang>.json against i18n/en.json: same keys, same list lengths,
same {placeholders}, same HTML tags and links. Usage: python check_i18n.py [lang ...]"""
import json, os, re, sys
ROOT = os.path.dirname(os.path.abspath(__file__))
en = json.load(open(os.path.join(ROOT, "i18n", "en.json"), encoding="utf-8"))
PH = re.compile(r"\{[a-z]*\}")
TAG = re.compile(r"</?(?:b|em|code|kbd|a)\b[^>]*>")


def walk(a, b, path, errs):
    if isinstance(a, dict):
        if not isinstance(b, dict):
            errs.append(f"{path}: not an object"); return
        for k in a:
            if k not in b: errs.append(f"{path}.{k}: missing")
            else: walk(a[k], b[k], f"{path}.{k}", errs)
        for k in b:
            if k not in a: errs.append(f"{path}.{k}: extra")
    elif isinstance(a, list):
        if not isinstance(b, list) or len(a) != len(b):
            errs.append(f"{path}: list length {len(a)} != {len(b) if isinstance(b, list) else '?'}"); return
        for i, (x, y) in enumerate(zip(a, b)): walk(x, y, f"{path}[{i}]", errs)
    elif isinstance(a, str):
        if not isinstance(b, str): errs.append(f"{path}: not a string"); return
        if sorted(PH.findall(a)) != sorted(PH.findall(b)): errs.append(f"{path}: placeholders {PH.findall(a)} != {PH.findall(b)}")
        ta = sorted(re.sub(r'\s.*', '', t) for t in TAG.findall(a)); tb = sorted(re.sub(r'\s.*', '', t) for t in TAG.findall(b))
        if ta != tb: errs.append(f"{path}: tags {ta} != {tb}")
        if re.findall(r'href="([^"]+)"', a) != re.findall(r'href="([^"]+)"', b): errs.append(f"{path}: links differ")


langs = sys.argv[1:] or [f[:-5] for f in sorted(os.listdir(os.path.join(ROOT, "i18n"))) if f.endswith(".json") and f != "en.json"]
bad = 0
for l in langs:
    errs = []
    walk(en, json.load(open(os.path.join(ROOT, "i18n", l + ".json"), encoding="utf-8")), l, errs)
    print(f"{l}: {'OK' if not errs else str(len(errs)) + ' problems'}")
    for e in errs[:30]: print("   ", e)
    bad += len(errs)
sys.exit(1 if bad else 0)
