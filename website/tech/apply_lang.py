# -*- coding: utf-8 -*-
"""Apply a per-language translation (tech/tr-<lang>.json: english_text -> translated marker)
onto tech-body.html, writing tech-body.<lang>.html ."""
import os, re, sys, json
from lxml import etree, html as lxml_html

HERE = os.path.dirname(os.path.abspath(__file__))

KINDS = ('h2','h3','h4','p','li','th','td','figcaption')

def inner_html(el):
    parts = []
    if el.text: parts.append(el.text)
    for c in el:
        parts.append(etree.tostring(c, encoding='unicode', method='html', with_tail=False))
        if c.tail: parts.append(c.tail)
    return ''.join(parts)

def set_inner_html(el, html):
    """Replace the element's children with a small HTML fragment.

    The fragment is parsed by libxml2's *fragment* parser, not by wrapping it in
    <x>…</x> and calling the document parser: that one always builds a full
    <html><body> tree, so `<x>` landed inside a <body> and every translated
    element came out as `<body><x>…</x></body>` (490 of them per language).
    """
    for c in list(el):
        el.remove(c)
    el.text = None
    if not html:
        return
    nodes = lxml_html.fragments_fromstring(html, parser=etree.HTMLParser(recover=True))
    if not nodes:
        return
    if isinstance(nodes[0], str):
        el.text = nodes.pop(0)
    for c in nodes:
        el.append(c)

def marker_to_html(m):
    m = m.replace('&','&amp;').replace('<','&lt;').replace('>','&gt;')
    m = re.sub(r'`([^`]*)`', r'<code>\1</code>', m)
    m = re.sub(r'\*\*([^*]+)\*\*', r'<b>\1</b>', m)
    m = re.sub(r'\*([^*]+)\*', r'<i>\1</i>', m)
    m = m.replace('\n', '<br>')
    return m

def norm(s): return re.sub(r'\s+',' ',s).strip()

def main(lang):
    data = open(os.path.join(HERE,'tech-body.html'),'rb').read()
    parser = etree.HTMLParser(encoding='utf-8', recover=True)
    doc = etree.fromstring(data, parser)
    tr = {}
    if lang != 'en':
        tr = json.load(open(os.path.join(HERE,f'tr-{lang}.json'), encoding='utf-8'))
    applied = 0
    missing = []
    todo = []
    for el in doc.iter():
        if el.tag not in KINDS: continue
        text = norm(''.join(el.itertext()))
        if not text: continue
        if text in tr:
            todo.append((el, marker_to_html(tr[text])))
        elif lang != 'en':
            missing.append(text[:60])
    for el, html in todo:
        set_inner_html(el, html)
    applied = len(todo)
    body = doc.find('body')
    if body is None:
        body = doc
    html = ''.join(etree.tostring(c, encoding='unicode', method='html') for c in body)
    open(os.path.join(HERE,f'tech-body.{lang}.html'),'w',encoding='utf-8').write(html)
    print(f'{lang}: applied {applied} translations, missing {len(missing)}')
    if missing:
        for m in missing[:20]:
            print('   MISSING:', m)

if __name__ == '__main__':
    main(sys.argv[1])
