import sys, subprocess
sys.path.insert(0, sys.argv[0].rsplit('/',1)[0])
from ourapps import insert_after_brand, JS
CSS = """
/* our apps menu (pill like the language picker) */
.apps { position: relative; }
.apps summary { list-style: none; display: inline-flex; align-items: center; gap: 7px; height: 38px; padding: 0 13px; background: var(--field); color: var(--dim); border: 1px solid var(--line-strong); border-radius: 999px; font-size: .9rem; cursor: pointer; white-space: nowrap; user-select: none; transition: color .15s, border-color .15s, box-shadow .15s; }
.apps summary::-webkit-details-marker { display: none; }
.apps summary:hover, .apps[open] summary { color: var(--text); border-color: var(--accent-soft); box-shadow: 0 0 16px rgba(95, 196, 255, .22); }
.apps .chev { color: var(--accent); transition: transform .15s; }
.apps[open] .chev { transform: rotate(180deg); }
.apps-menu { position: absolute; inset-inline-start: 0; top: calc(100% + 8px); z-index: 60; display: grid; gap: 2px; min-width: 210px; padding: 8px; background: linear-gradient(180deg, var(--panel-top), var(--panel-bottom)); border: 1px solid var(--line-strong); border-radius: 14px; box-shadow: 0 24px 60px rgba(0, 0, 0, .55); }
.apps-menu a { display: block; padding: 8px 12px; border-radius: 9px; color: var(--text); font-size: .95rem; white-space: nowrap; }
.apps-menu a:hover, .apps-menu a:focus-visible { background: rgba(30, 155, 255, .14); text-decoration: none; outline: none; }
.apps-menu a[aria-current] { background: var(--accent); color: #fff; }
.apps-menu .apps-home { font-weight: 600; letter-spacing: .02em; }
.apps-menu hr { border: 0; height: 1px; margin: 4px 8px; background: linear-gradient(90deg, transparent, var(--accent-soft), transparent); opacity: .55; }
@media (max-width: 600px) { .apps summary { padding: 0 11px; } .apps-name { display: none; } }
"""
pages = subprocess.check_output(['grep', '-rl', 'class="brand"', 'dist']).decode().split()
n = 0
for f in ['build.py'] + pages:
    s = open(f, encoding='utf-8').read(); o = insert_after_brand(s, 'NikPak')
    if o != s: open(f, 'w', encoding='utf-8', newline='').write(o); n += 1
print('markup patched', n)
for f, add in [('static/css/site.css', CSS), ('dist/assets/css/site.css', CSS), ('static/js/site.js', JS), ('dist/assets/js/site.js', JS)]:
    s = open(f, encoding='utf-8').read()
    if 'our apps menu' not in s:
        open(f, 'w', encoding='utf-8', newline='').write(s.rstrip('\n') + '\n' + add); print('patched', f)
