/* NikPak web site: language, menu, reveal effects, the animated app window, charts.
   One set of pages, eleven languages: the text is English in the HTML and carries
   data-i18n, and this file swaps it from assets/i18n/<lang>.json - the same model as
   the NikEdit site. */
(function () {
  'use strict';
  var doc = document.documentElement;
  doc.classList.remove('no-js');

  var S = window.NP_SITE || { langs: [{ code: 'en', native: 'English', html: 'en' }], flags: {}, cats: {}, version: '', email: '' };
  var LANGS = S.langs, base = location.pathname.replace(/[^/]*$/, '');
  var LSK = 'nikpak_lang';
  var lang = 'en', dict = null, english = null;

  function get(k) { try { return localStorage.getItem(k); } catch (e) { return null; } }
  function set(k, v) { try { localStorage.setItem(k, v); } catch (e) {} }

  function pick() {
    var v = get(LSK);
    if (v && LANGS.some(function (l) { return l.code === v; })) return v;
    var ls = navigator.languages || [navigator.language || 'en'];
    for (var i = 0; i < ls.length; i++) {
      var c = String(ls[i]).toLowerCase().split('-')[0];
      if (LANGS.some(function (l) { return l.code === c; })) return c;
    }
    return 'en';
  }

  /* a.b[3].c -> value */
  function lookup(d, key) {
    var parts = key.replace(/\[(\d+)\]/g, '.$1').split('.'), v = d;
    for (var i = 0; i < parts.length; i++) {
      if (v == null) return null;
      v = v[parts[i]];
    }
    return v;
  }

  function t(key, vars) {
    var src = dict || english || {};
    var v = lookup(src, key);
    if (v == null && english) v = lookup(english, key);
    if (v == null) return null;
    v = String(v);
    if (vars) Object.keys(vars).forEach(function (k) { v = v.split('{' + k + '}').join(vars[k]); });
    v = v.split('{}').join(vars && vars['0'] != null ? vars['0'] : '');
    v = v.split('{v}').join(S.version || '');
    v = v.split('{email}').join(S.email || '');
    return v;
  }

  function varsOf(el) {
    var raw = el.getAttribute('data-i18n-vars');
    if (!raw) return null;
    try { return JSON.parse(raw); } catch (e) { return null; }
  }

  function apply() {
    document.documentElement.lang = (LANGS.filter(function (l) { return l.code === lang; })[0] || {}).html || lang;
    document.documentElement.dir = lang === 'ar' ? 'rtl' : 'ltr';
    document.body.setAttribute('data-lang', lang);

    document.querySelectorAll('[data-i18n]').forEach(function (el) {
      var v = t(el.getAttribute('data-i18n'), varsOf(el));
      if (v == null) return;
      if (el.tagName === 'TITLE') {
        el.textContent = v + (el.getAttribute('data-suffix') || '');
        return;
      }
      el.innerHTML = v;
    });
    // attributes: data-i18n-content, data-i18n-placeholder, data-i18n-aria-label ...
    document.querySelectorAll('*').forEach(function (el) {
      if (!el.attributes) return;
      for (var i = 0; i < el.attributes.length; i++) {
        var a = el.attributes[i];
        if (a.name.indexOf('data-i18n-') !== 0 || a.name === 'data-i18n-vars' || a.name === 'data-i18n') continue;
        var v = t(a.value);
        if (v != null) el.setAttribute(a.name.slice(10), v);
      }
    });
    var cats = (S.cats && S.cats[lang]) || (S.cats && S.cats.en) || [];
    document.querySelectorAll('.apps-menu .apps-cat').forEach(function (el, i) {
      if (cats[i]) el.textContent = cats[i];
    });
    var fl = document.getElementById('langFlag');
    if (fl && S.flags[lang]) fl.innerHTML = S.flags[lang];
    var nm = document.getElementById('langName');
    if (nm) nm.textContent = (LANGS.filter(function (l) { return l.code === lang; })[0] || {}).native || lang;
    document.querySelectorAll('.lang-item').forEach(function (b) {
      b.classList.toggle('active', b.getAttribute('data-code') === lang);
      b.setAttribute('aria-current', b.getAttribute('data-code') === lang ? 'true' : 'false');
    });
    if (document.body.getAttribute('data-page') === 'tech') loadTech();
    document.dispatchEvent(new CustomEvent('nikpak:lang'));
  }

  /* the technical guide: English is in tech.html, the other ten are fetched */
  function loadTech() {
    var box = document.querySelector('.tech-content');
    if (!box) return;
    if (lang === 'en') {
      if (box.getAttribute('data-en') != null) box.innerHTML = box.getAttribute('data-en');
      return;
    }
    if (box.getAttribute('data-en') == null) box.setAttribute('data-en', box.innerHTML);
    fetch(base + 'assets/tech/' + lang + '.html').then(function (r) {
      if (!r.ok) throw 0; return r.text();
    }).then(function (html) {
      box.innerHTML = html;
      var aside = document.querySelector('aside.toc');
      if (aside) {
        var label = aside.querySelector('.label');
        return fetch(base + 'assets/tech/' + lang + '-toc.html').then(function (r) {
          if (!r.ok) throw 0; return r.text();
        }).then(function (toc) {
          aside.innerHTML = (label ? label.outerHTML : '') + toc;
        });
      }
    }).catch(function () { /* keep the English text */ });
  }

  function buildMenu() {
    var menu = document.getElementById('langMenu');
    if (!menu) return;
    LANGS.forEach(function (l) {
      var b = document.createElement('button');
      b.type = 'button';
      b.className = 'lang-item';
      b.setAttribute('data-code', l.code);
      b.setAttribute('role', 'menuitem');
      b.innerHTML = (S.flags[l.code] || '') + '<span>' + l.native + '</span>';
      b.addEventListener('click', function () {
        lang = l.code; set(LSK, lang); apply(); closeLang();
      });
      menu.appendChild(b);
    });
  }

  function closeLang() {
    var lp = document.getElementById('lang');
    if (!lp) return;
    lp.classList.remove('open');
    var b = lp.querySelector('button');
    if (b) b.setAttribute('aria-expanded', 'false');
  }

  window.NP = {
    t: t,
    lang: function () { return lang; },
    dict: function (keys) {
      // {name: "buy.pay_btn"} -> {name: "<the text in the current language>"}
      var o = {};
      Object.keys(keys || {}).forEach(function (k) {
        Object.defineProperty(o, k, { get: function () { return t(keys[k]) || ''; }, enumerable: true });
      });
      return o;
    }
  };

  function start() {
    buildMenu();
    lang = pick();
    try { localStorage.setItem(LSK, lang); } catch (e) {}
    var path = base + 'assets/i18n/' + lang + '.json';
    if (lang === 'en') { apply(); return; }
    fetch(path).then(function (r) { return r.ok ? r.json() : null; })
      .then(function (d) { dict = d; apply(); })
      .catch(function () { apply(); });
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start); else start();

  // mobile menu
  var top = document.getElementById('top');
  var mt = top && top.querySelector('.menu-toggle');
  if (mt) mt.addEventListener('click', function () {
    var open = top.classList.toggle('open');
    mt.setAttribute('aria-expanded', open ? 'true' : 'false');
  });

  // language picker
  var lp = document.getElementById('lang');
  if (lp) {
    var b = lp.querySelector('button');
    b.addEventListener('click', function (e) {
      e.stopPropagation();
      var open = lp.classList.toggle('open');
      b.setAttribute('aria-expanded', open ? 'true' : 'false');
    });
    document.addEventListener('click', function () { closeLang(); });
    document.addEventListener('keydown', function (e) { if (e.key === 'Escape') closeLang(); });
  }

  // toast
  window.nikpakToast = function (msg) {
    var t = document.getElementById('toast');
    if (!t) return;
    t.textContent = msg; t.classList.add('show');
    clearTimeout(t._h); t._h = setTimeout(function () { t.classList.remove('show'); }, 1800);
  };

  // reveal on scroll + bar charts
  var reduce = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  function fillBars(root) {
    root.querySelectorAll('.f[data-w]').forEach(function (f) { f.style.width = f.getAttribute('data-w') + '%'; });
  }
  if ('IntersectionObserver' in window && !reduce) {
    var io = new IntersectionObserver(function (es) {
      es.forEach(function (e) {
        if (e.isIntersecting) { e.target.classList.add('in'); fillBars(e.target); io.unobserve(e.target); }
      });
    }, { threshold: 0.12 });
    document.querySelectorAll('.reveal, .chart').forEach(function (el) { io.observe(el); });
  } else {
    document.querySelectorAll('.reveal').forEach(function (el) { el.classList.add('in'); });
    document.querySelectorAll('.chart').forEach(fillBars);
  }

  // table of contents: highlight the current section
  var toc = document.querySelectorAll('.toc a');
  if (toc.length && 'IntersectionObserver' in window) {
    var map = {};
    toc.forEach(function (a) { map[a.getAttribute('href').slice(1)] = a; });
    var so = new IntersectionObserver(function (es) {
      es.forEach(function (e) {
        if (e.isIntersecting) {
          toc.forEach(function (a) { a.classList.remove('active'); });
          var a = map[e.target.id]; if (a) a.classList.add('active');
        }
      });
    }, { rootMargin: '-30% 0px -60% 0px' });
    document.querySelectorAll('.doc section[id]').forEach(function (s) { so.observe(s); });
  }

  // thanks page: ?status=ok|pending|cancel|error
  var th = document.getElementById('thanks');
  if (th) {
    var st = new URLSearchParams(location.search).get('status') || 'ok';
    if (['ok', 'pending', 'cancel', 'error'].indexOf(st) < 0) st = 'ok';
    th.querySelectorAll('[data-show]').forEach(function (d) { d.hidden = d.getAttribute('data-show') !== st; });
    var col = st === 'ok' ? '#3DDC97' : st === 'pending' ? '#5FC4FF' : st === 'cancel' ? '#FFC45C' : '#FF6B7A';
    var ring = document.getElementById('th-ring'), mark = document.getElementById('th-mark');
    ring.setAttribute('stroke', col); mark.setAttribute('stroke', col);
    if (st === 'pending') { ring.setAttribute('stroke-dasharray', '120 327'); mark.setAttribute('d', 'M60 38v26l16 10'); }
    if (st === 'cancel' || st === 'error') mark.setAttribute('d', 'M45 45l30 30M75 45l-30 30');
  }

  // the NikPak window: idle -> compressing -> done, like the real program
  var win = document.getElementById('appwin');
  if (win && !reduce) {
    var C = 2 * Math.PI * 86;
    var prog = win.querySelector('.prog'), track = win.querySelector('.track'), dash = win.querySelector('.dash');
    var pct = win.querySelector('.pct'), pctN = pct.querySelector('b'), arrow = win.querySelector('.arrow'), check = win.querySelector('.check');
    var amt = win.querySelector('.amt'), ofTpl = win.getAttribute('data-of') || '{} of {}';
    var total = 7.0;
    function setP(p) {
      prog.setAttribute('stroke-dasharray', (C * p) + ' ' + C);
      pctN.textContent = Math.round(p * 100);
      amt.textContent = ofTpl.replace('{}', (total * p).toFixed(1) + ' MB').replace('{}', total.toFixed(1) + ' MB');
    }
    function state(s) {
      win.setAttribute('data-state', s);
      var run = s === 'run', done = s === 'done';
      track.setAttribute('opacity', run || done ? '1' : '0');
      dash.style.opacity = run || done ? '0' : '1';
      arrow.style.opacity = s === 'idle' ? '1' : '0';
      pct.style.opacity = run ? '1' : '0';
      check.setAttribute('opacity', done ? '1' : '0');
      prog.setAttribute('stroke', done ? '#3DDC97' : 'url(#rg)');
      prog.style.filter = done ? 'drop-shadow(0 0 8px rgba(61,220,151,.7))' : 'drop-shadow(0 0 8px rgba(30,155,255,.7))';
      if (s === 'idle') prog.setAttribute('stroke-dasharray', '0 ' + C);
      if (done) prog.setAttribute('stroke-dasharray', C + ' ' + C);
    }
    function cycle() {
      state('idle');
      setTimeout(function () {
        state('run');
        var t0 = performance.now(), dur = 3600;
        (function step(now) {
          var x = Math.min(1, (now - t0) / dur);
          setP(1 - Math.pow(1 - x, 1.6));
          if (x < 1) requestAnimationFrame(step);
          else { state('done'); setTimeout(cycle, 3200); }
        })(t0);
      }, 2600);
    }
    cycle();
  }

  // skins gallery: filter (all / dark / bright) and the full-size viewer
  var grid = document.getElementById('skin-grid');
  if (grid) {
    var tiles = Array.prototype.slice.call(grid.querySelectorAll('.skin-tile'));
    document.querySelectorAll('.chip[data-filter]').forEach(function (c) {
      c.addEventListener('click', function () {
        var f = c.getAttribute('data-filter');
        document.querySelectorAll('.chip[data-filter]').forEach(function (o) { o.setAttribute('aria-pressed', o === c ? 'true' : 'false'); });
        tiles.forEach(function (t) { t.hidden = f !== 'all' && t.getAttribute('data-dark') !== f; });
      });
    });
    var dlg = document.getElementById('skin-view');
    if (dlg && typeof dlg.showModal === 'function') {
      var img = dlg.querySelector('img'), cap = dlg.querySelector('figcaption'), cur = 0;
      var shown = function () { return tiles.filter(function (t) { return !t.hidden; }); };
      var show = function (i) {
        var list = shown();
        if (!list.length) return;
        cur = (i + list.length) % list.length;
        var a = list[cur].querySelector('a');
        img.src = a.getAttribute('href');
        img.alt = cap.textContent = a.getAttribute('data-name');
      };
      tiles.forEach(function (t) {
        t.querySelector('a').addEventListener('click', function (e) {
          e.preventDefault();
          show(shown().indexOf(t));
          dlg.showModal();
        });
      });
      dlg.addEventListener('click', function (e) {
        var b = e.target.closest('button');
        if (b && b.hasAttribute('data-step')) { show(cur + parseInt(b.getAttribute('data-step'), 10)); return; }
        if ((b && b.hasAttribute('data-close')) || e.target === dlg) dlg.close();
      });
      dlg.addEventListener('keydown', function (e) {
        if (e.key === 'ArrowRight') show(cur + 1);
        if (e.key === 'ArrowLeft') show(cur - 1);
      });
    }
  }

  // downloads: highlight the visitor's system's group (Windows is the default)
  var ua = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || navigator.userAgent || '';
  var os = /mac|iphone|ipad/i.test(ua) ? 'mac' : /linux|x11|bsd|cros|unix/i.test(ua) && !/android/i.test(navigator.userAgent) ? 'linux' : /win/i.test(ua) ? 'win' : 'win';
  document.querySelectorAll('.dl-group[data-os]').forEach(function (g) {
    g.classList.toggle('os-current', g.getAttribute('data-os') === os);
  });
})();

// our apps menu: close on a click outside or Esc
(function () {
  var apps = document.querySelector('details.apps');
  if (!apps) return;
  document.addEventListener('click', function (e) { if (apps.open && !apps.contains(e.target)) apps.open = false; }, true);
  document.addEventListener('keydown', function (e) { if (e.key === 'Escape' && apps.open) { apps.open = false; apps.querySelector('summary').focus(); } });
})();

// nav dropdowns (Guide › Technical, Contact › Changelog): one open at a time,
// close on a click outside or Esc
(function () {
  var drops = document.querySelectorAll('nav .navdrop');
  if (!drops.length) return;
  drops.forEach(function (d) {
    d.querySelector('summary').addEventListener('click', function () {
      drops.forEach(function (o) { if (o !== d) o.open = false; });
    });
  });
  document.addEventListener('click', function (e) {
    drops.forEach(function (d) { if (d.open && !d.contains(e.target)) d.open = false; });
  }, true);
  document.addEventListener('keydown', function (e) {
    if (e.key === 'Escape') drops.forEach(function (d) { d.open = false; });
  });
})();
