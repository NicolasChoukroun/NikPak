/* ---------------------------------------------------------------------------
 * Nikkoid - counted downloads and the running total in the footer.
 *
 * One file, dropped into every product's site as assets/nk-downloads.js and loaded
 * with a single <script> tag. It does two things, and nothing else:
 *
 *   1. rewrites every link to downloads/<file> so it goes through download.php,
 *      which counts the file and then redirects to it;
 *   2. fills #nk-dl-total (or, if the page has none, appends the number to the
 *      footer) from stats.php.
 *
 * Doing it here rather than in each page's markup is deliberate: five sites generate
 * their HTML five different ways, and a change that has to be made in five generators
 * is a change that will be made in four. Without JavaScript the links still work and
 * the download still starts - it is only not counted, and the number is not shown.
 * ------------------------------------------------------------------------- */
(function () {
  'use strict';

  var script = document.currentScript;
  if (!script) return;

  /* The site root, from this script's own URL: .../assets/nk-downloads.js -> .../ */
  var base = script.src.replace(/[?#].*$/, '').replace(/assets\/nk-downloads\.js$/, '');

  /* ---------------------------------------------------------------- links */
  function countLinks() {
    var links = document.querySelectorAll('a[href]');
    for (var i = 0; i < links.length; i++) {
      var a = links[i];
      if (a.getAttribute('data-nk-counted')) continue;
      var href = a.getAttribute('href');
      if (!href || /^(https?:|mailto:|#|\/)/i.test(href)) continue;
      /* A direct link to one of our files: downloads/<name> or documents/<name>,
         possibly behind a relative prefix (../downloads/<name> on a subpage). */
      var m = /(?:^|\/)(downloads|documents)\/([^/?#]+)$/.exec(href);
      if (!m) continue;
      a.setAttribute('data-nk-counted', '1');
      a.setAttribute('href', base + 'download.php?f=' + encodeURIComponent(m[2]));
    }
  }

  /* ---------------------------------------------------------------- total */
  function format(n) {
    var lang = (document.documentElement.getAttribute('lang') || 'en').split('-')[0];
    try { return new Intl.NumberFormat(lang).format(n); } catch (e) { return String(n); }
  }

  function paint(n) {
    var box = document.getElementById('nk-dl-total');
    if (!box) {
      /* No placeholder in the page: add one to the footer, where a visitor expects
         this kind of number. A site with no footer gets nothing - better than a
         floating line of text at the end of the document. */
      var footer = document.querySelector('footer') || document.querySelector('.foot');
      if (!footer) return;
      box = document.createElement('span');
      box.id = 'nk-dl-total';
      box.className = 'nk-dl-total';
      footer.appendChild(box);
    }
    var label = box.getAttribute('data-label') || 'downloads';
    box.textContent = format(n) + ' ' + label;
    box.title = n + ' files downloaded';
  }

  function total() {
    if (!window.fetch) return;
    fetch(base + 'stats.php', { cache: 'no-store', credentials: 'omit' })
      .then(function (r) { return r.ok ? r.json() : null; })
      .then(function (d) {
        if (!d || typeof d.total !== 'number' || d.total < 1) return;
        window.__nkDlTotal = d.total;
        paint(d.total);
      })
      .catch(function () { /* the number is a nicety; never an error on the page */ });
  }

  function run() {
    countLinks();
    total();
    /* the language can change without a reload: re-format the same number */
    if (window.MutationObserver) {
      new MutationObserver(function () {
        if (typeof window.__nkDlTotal === 'number') paint(window.__nkDlTotal);
      }).observe(document.documentElement, { attributes: true, attributeFilter: ['lang'] });
    }
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', run);
  else run();
})();
