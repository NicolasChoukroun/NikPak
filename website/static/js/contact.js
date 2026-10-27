/* NikPak contact form: crypto payment fields, subject preview (always starts with NikPak), sending with fetch */
(function () {
  'use strict';
  var cfgEl = document.getElementById('contact-config');
  if (!cfgEl) return;
  var cfg = JSON.parse(cfgEl.textContent), T = cfg.t;
  var f = document.getElementById('contact');
  var $ = function (id) { return document.getElementById(id); };
  $('ts').value = String(Math.floor(Date.now() / 1000));

  // prefill from the shop: contact.html?topic=crypto&coin=BTC&amount=...
  var q = new URLSearchParams(location.search);
  ['topic', 'coin', 'amount', 'licence', 'seats', 'holder', 'email'].forEach(function (k) {
    var v = q.get(k); if (v === null) return;
    var el = k === 'email' ? $('f-email') : $('f-' + k);
    if (el && el.tagName === 'SELECT') { if (el.querySelector('option[value="' + v.replace(/"/g, '') + '"]')) el.value = v; }
    else if (el) el.value = v;
  });

  // must stay identical to contact.php (nikpak_contact_subject)
  function subject() {
    var topic = $('f-topic').value, s = $('f-subject').value.trim().replace(/\s+/g, ' ');
    var label = cfg.topics_en[topic] || cfg.topics_en.general;
    var out = 'NikPak - ' + label;
    if (topic === 'crypto') {
      var coin = $('f-coin').value, tx = $('f-txid').value.trim();
      out += ' ' + coin + (tx ? ' ' + tx.slice(0, 14) + (tx.length > 14 ? '…' : '') : '');
    }
    if (s) out += ' - ' + s;
    return out.slice(0, 180);
  }
  function refresh() {
    var crypto = $('f-topic').value === 'crypto';
    $('crypto-fields').hidden = !crypto;
    $('crypto-fields').style.display = crypto ? 'grid' : 'none';
    $('f-txid').required = crypto;
    $('f-subject').required = !crypto;
    $('subject-preview').textContent = subject();
  }
  ['f-topic', 'f-subject', 'f-coin', 'f-txid'].forEach(function (id) { $(id).addEventListener('input', refresh); $(id).addEventListener('change', refresh); });
  refresh();

  if (q.get('sent') === '1') $('c-ok').classList.add('show');
  if (q.get('sent') === '0') { $('c-err').textContent = T.error.replace('{email}', cfg.email); $('c-err').classList.add('show'); }

  f.addEventListener('submit', function (e) {
    e.preventDefault();
    $('c-ok').classList.remove('show'); $('c-err').classList.remove('show');
    var crypto = $('f-topic').value === 'crypto';
    var okForm = $('f-name').value.trim().length > 1 && /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test($('f-email').value.trim()) &&
      (crypto ? $('f-txid').value.trim().length > 5 : ($('f-message').value.trim().length > 2 && $('f-subject').value.trim().length > 0));
    if (!okForm) { $('c-err').textContent = T.invalid; $('c-err').classList.add('show'); return; }
    var btn = $('f-send'); btn.disabled = true; btn.querySelector('span').textContent = T.sending;
    fetch(f.action, { method: 'POST', body: new FormData(f), headers: { 'Accept': 'application/json' } })
      .then(function (r) { return r.json(); })
      .then(function (j) {
        if (j && j.ok) { f.reset(); $('ts').value = String(Math.floor(Date.now() / 1000)); refresh(); $('c-ok').classList.add('show'); $('c-ok').scrollIntoView({ block: 'center', behavior: 'smooth' }); }
        else throw new Error(j && j.error);
      })
      .catch(function (err) {
        $('c-err').textContent = (err && err.message === 'invalid') ? T.invalid : T.error.replace('{email}', cfg.email);
        $('c-err').classList.add('show');
      })
      .then(function () { btn.disabled = false; btn.querySelector('span').textContent = T.send; });
  });
})();
