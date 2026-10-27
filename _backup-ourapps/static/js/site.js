/* NikPak web site: menu, language picker, reveal effects, the animated app window, charts */
(function () {
  'use strict';
  var doc = document.documentElement;
  doc.classList.remove('no-js');
  var lang = document.body.getAttribute('data-lang');
  try { localStorage.setItem('nikpak_lang', lang); } catch (e) {}
  document.cookie = 'nikpak_lang=' + lang + ';path=/;max-age=31536000;SameSite=Lax';

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
    document.addEventListener('click', function () { lp.classList.remove('open'); b.setAttribute('aria-expanded', 'false'); });
    document.addEventListener('keydown', function (e) { if (e.key === 'Escape') { lp.classList.remove('open'); b.setAttribute('aria-expanded', 'false'); } });
    // keep the query string (seats, topic...) and the anchor when switching language
    lp.querySelectorAll('.lang-menu a').forEach(function (a) {
      a.addEventListener('click', function () {
        try { localStorage.setItem('nikpak_lang', a.getAttribute('data-lang')); } catch (e) {}
        a.href = a.getAttribute('href').split('?')[0].split('#')[0] + location.search + location.hash;
      });
    });
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

  // downloads: highlight the visitor's system (Windows is the default)
  var ua = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || navigator.userAgent || '';
  var os = /mac|iphone|ipad/i.test(ua) ? 'mac' : /linux|x11|bsd|cros|unix/i.test(ua) && !/android/i.test(navigator.userAgent) ? 'linux' : /win/i.test(ua) ? 'win' : '';
  if (os) document.querySelectorAll('.dl-row').forEach(function (row) {
    row.querySelectorAll('.dl-btn').forEach(function (a) { a.classList.toggle('btn-primary', a.getAttribute('data-os') === os); });
  });
})();
