# Shared helper: builds the "Our Apps" markup for a given current app
APPS = [("Nikkomander","https://nikkomander.nikkoid.com/index.html"),
        ("NikEDIT","https://nikedit.nikkoid.com/index.html"),
        ("NikSQL","https://niksql.nikkoid.com/index.html"),
        ("NikPak","https://nikpak.nikkoid.com/en/index.html"),
        ("Hexawar","https://hexawar.nikkoid.com/website/")]
CHEV = '<svg class="chev" viewBox="0 0 12 12" width="10" height="10" aria-hidden="true"><path d="M2 4l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>'
ICO = '<svg class="apps-ico" viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><rect x="1.5" y="1.5" width="5" height="5" rx="1.3" fill="currentColor"/><rect x="9.5" y="1.5" width="5" height="5" rx="1.3" fill="currentColor"/><rect x="1.5" y="9.5" width="5" height="5" rx="1.3" fill="currentColor"/><rect x="9.5" y="9.5" width="5" height="5" rx="1.3" fill="currentColor"/></svg>'
def snippet(current, indent='    '):
    cur = ' aria-current="true"'
    links = ''.join('<a href="%s"%s>%s</a>' % (u, cur if n == current else '', n) for n, u in APPS)
    return (f'{indent}<details class="apps">\n'
            f'{indent}  <summary aria-label="Our Apps">{ICO}<span class="apps-name">Our Apps</span>{CHEV}</summary>\n'
            f'{indent}  <div class="apps-menu" role="list"><a class="apps-home" href="https://nikkoid.com">Nikkoid Network</a><hr>{links}</div>\n'
            f'{indent}</details>')
JS = """
// our apps menu: close on a click outside or Esc
(function () {
  var apps = document.querySelector('details.apps');
  if (!apps) return;
  document.addEventListener('click', function (e) { if (apps.open && !apps.contains(e.target)) apps.open = false; }, true);
  document.addEventListener('keydown', function (e) { if (e.key === 'Escape' && apps.open) { apps.open = false; apps.querySelector('summary').focus(); } });
})();
"""
def insert_after_brand(s, current, indent='    '):
    """Insert the menu right after the closing </a> of the first class="brand" link."""
    if 'class="apps"' in s: return s
    i = s.index('class="brand"')
    j = s.index('</a>', i) + 4
    return s[:j] + '\n' + snippet(current, indent) + s[j:]
