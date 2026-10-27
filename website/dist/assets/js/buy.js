/* NikPak shop: seat calculator, payment methods (Google Pay, Stripe, PayPal, Paddle, Binance Pay), crypto transfers.
   Prices shown here are for display only: the server computes the amount again (shop/lib.php, nikpak_price). */
(function () {
  'use strict';
  var cfgEl = document.getElementById('buy-config');
  if (!cfgEl) return;
  var cfg = JSON.parse(cfgEl.textContent);
  var T = cfg.t, lang = cfg.lang;
  var form = document.getElementById('shop');
  var $ = function (id) { return document.getElementById(id); };
  var seatsIn = $('seats'), range = $('seats-range');
  var fmt;
  try { fmt = new Intl.NumberFormat(lang === 'ar' ? 'en-US' : lang, { style: 'currency', currency: 'USD', maximumFractionDigits: 0, minimumFractionDigits: 0 }); }
  catch (e) { fmt = { format: function (n) { return '$' + n; } }; }
  var money = function (n) { return fmt.format(n); };

  function price(type, seats) {
    if (type !== 'company') return { per: 19, seats: 1, total: 19 };
    var per = 19;
    for (var i = 0; i < cfg.tiers.length; i++) if (seats >= cfg.tiers[i][0]) per = cfg.tiers[i][1];
    return { per: per, seats: seats, total: per * seats };
  }
  function order() {
    var type = form.querySelector('input[name="type"]:checked').value;
    var seats = Math.max(1, Math.min(100000, parseInt(seatsIn.value, 10) || 1));
    var p = price(type, seats);
    return { type: type, seats: p.seats, per: p.per, total: p.total, name: $('holder').value.trim(), email: $('email').value.trim(), lang: lang };
  }
  function method() { var m = form.querySelector('input[name="method"]:checked'); return m ? m.value : ''; }

  // ---------- calculator ----------
  function update() {
    var o = order();
    $('personal-box').hidden = o.type === 'company';
    $('company-box').hidden = o.type !== 'company';
    $('sum-per').textContent = money(o.per);
    $('sum-seats').textContent = o.seats.toLocaleString(lang === 'ar' ? 'ar-u-nu-latn' : lang);
    $('sum-total').textContent = money(o.total);
    var save = o.type === 'company' ? 19 * o.seats - o.total : 0;
    $('sum-saving').textContent = save > 0 ? T.saving.replace('{s}', money(save)) : '';
    document.querySelectorAll('#tiers tbody tr').forEach(function (tr, i, all) {
      var min = +tr.getAttribute('data-min'), next = all[i + 1] ? +all[i + 1].getAttribute('data-min') : Infinity;
      tr.classList.toggle('on', o.type === 'company' && o.seats >= min && o.seats < next);
    });
    var btn = $('pay-btn');
    btn.querySelector('span').textContent = T.continue_btn.replace('{amount}', money(o.total));
    updateCrypto();
  }
  function setSeats(n) {
    n = Math.max(1, Math.min(100000, n | 0));
    seatsIn.value = n; range.value = Math.min(600, n); update();
  }
  form.querySelectorAll('input[name="type"]').forEach(function (r) { r.addEventListener('change', update); });
  seatsIn.addEventListener('input', function () { range.value = Math.min(600, parseInt(seatsIn.value, 10) || 1); update(); });
  seatsIn.addEventListener('blur', function () { setSeats(parseInt(seatsIn.value, 10) || 1); });
  range.addEventListener('input', function () { setSeats(+range.value); });
  form.querySelectorAll('[data-step]').forEach(function (b) { b.addEventListener('click', function () { setSeats((parseInt(seatsIn.value, 10) || 1) + +b.getAttribute('data-step')); }); });
  form.querySelectorAll('[data-seats]').forEach(function (b) { b.addEventListener('click', function () { setSeats(+b.getAttribute('data-seats')); }); });

  // prefill from the link: buy.html?type=company&seats=25
  var q = new URLSearchParams(location.search);
  if (q.get('type') === 'company') form.querySelector('input[name="type"][value="company"]').checked = true;
  if (q.get('seats')) setSeats(parseInt(q.get('seats'), 10));
  if (q.get('method') === 'crypto') { setTimeout(function () { $('crypto').scrollIntoView({ block: 'start' }); }, 0); }
  else if (q.get('method')) { var mm = form.querySelector('input[name="method"][value="' + q.get('method').replace(/[^a-z]/g, '') + '"]'); if (mm) mm.checked = true; }

  // ---------- payment methods ----------
  var errBox = $('pay-error');
  function showError(msg) { errBox.textContent = msg; errBox.classList.add('show'); }
  function clearError() { errBox.classList.remove('show'); }
  function valid(o) {
    var ok = o.name.length >= 2 && /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(o.email);
    if (!ok) { showError(T.fill_first); (o.name.length < 2 ? $('holder') : $('email')).focus(); }
    return ok;
  }
  function selectMethod() {
    var m = method();
    clearError();
    form.querySelectorAll('.method').forEach(function (l) { l.classList.toggle('on', l.getAttribute('data-method') === m); });
    document.querySelectorAll('[data-for]').forEach(function (el) {
      el.hidden = el.getAttribute('data-for').split(' ').indexOf(m) < 0;
    });
  }
  form.querySelectorAll('input[name="method"]').forEach(function (r) {
    r.addEventListener('change', selectMethod);
  });

  function post(url, data) {
    return fetch(url, { method: 'POST', headers: { 'Content-Type': 'application/json', 'Accept': 'application/json' }, body: JSON.stringify(data), credentials: 'same-origin' })
      .then(function (r) { return r.json().catch(function () { return { ok: false }; }); });
  }

  // Stripe Checkout, PayPal, Paddle, Binance Pay: the server creates the payment and returns the provider's page
  var payBtn = $('pay-btn');
  payBtn.addEventListener('click', function () {
    clearError();
    var o = order();
    if (!valid(o)) return;
    o.method = method();
    payBtn.disabled = true;
    payBtn.querySelector('span').textContent = T.redirecting;
    post('../shop/checkout.php', o).then(function (r) {
      if (r && r.ok && r.url) { location.href = r.url; return; }
      throw new Error(r && r.error || 'error');
    }).catch(function () {
      showError(T.pay_error);
      payBtn.disabled = false; update();
    });
  });

  // ---------- Google Pay ----------
  var gp = cfg.payments.googlepay || {};
  var gpClient = null;
  var baseReq = { apiVersion: 2, apiVersionMinor: 0 };
  var cardMethod = {
    type: 'CARD',
    parameters: { allowedAuthMethods: ['PAN_ONLY', 'CRYPTOGRAM_3DS'], allowedCardNetworks: ['AMEX', 'DISCOVER', 'MASTERCARD', 'VISA'] },
    tokenizationSpecification: {
      type: 'PAYMENT_GATEWAY',
      parameters: { gateway: gp.gateway || 'stripe', 'stripe:version': gp.stripe_version || '2018-10-31', 'stripe:publishableKey': gp.stripe_publishable_key || '' }
    }
  };
  var GP_LOCALES = ['ar', 'bg', 'ca', 'cs', 'da', 'de', 'el', 'en', 'es', 'et', 'fi', 'fr', 'hr', 'id', 'it', 'ja', 'ko', 'ms', 'nl', 'no', 'pl', 'pt', 'ru', 'sk', 'sl', 'sr', 'sv', 'th', 'tr', 'uk', 'zh'];
  function gpayStart() {
    if (!gp.enabled || !window.google || !google.payments) return;
    gpClient = new google.payments.api.PaymentsClient({ environment: gp.environment === 'PRODUCTION' ? 'PRODUCTION' : 'TEST' });
    gpClient.isReadyToPay(Object.assign({}, baseReq, { allowedPaymentMethods: [{ type: 'CARD', parameters: cardMethod.parameters }] }))
      .then(function (r) {
        var box = $('gpay-container');
        if (!r.result) { box.innerHTML = '<p class="dim">' + T.gpay_unavailable + '</p>'; return; }
        box.innerHTML = '';
        box.appendChild(gpClient.createButton({
          onClick: gpayClick, buttonColor: 'white', buttonType: 'pay', buttonSizeMode: 'fill',
          buttonLocale: GP_LOCALES.indexOf(lang) >= 0 ? lang : 'en'
        }));
      }).catch(function () { $('gpay-container').innerHTML = '<p class="dim">' + T.gpay_unavailable + '</p>'; });
  }
  function gpayClick() {
    clearError();
    var o = order();
    if (!valid(o)) return;
    var req = Object.assign({}, baseReq, {
      allowedPaymentMethods: [cardMethod],
      merchantInfo: { merchantName: gp.merchant_name || 'Nikkoid LLC', merchantId: gp.merchant_id || undefined },
      transactionInfo: {
        totalPriceStatus: 'FINAL', totalPriceLabel: 'NikPak', totalPrice: o.total.toFixed(2), currencyCode: 'USD', countryCode: 'US',
        displayItems: [{ label: (o.type === 'company' ? T.company + ' × ' + o.seats : T.personal), type: 'LINE_ITEM', price: o.total.toFixed(2) }]
      }
    });
    gpClient.loadPaymentData(req).then(function (pd) {
      o.method = 'googlepay';
      o.token = pd.paymentMethodData.tokenizationData.token;
      return post('../shop/gpay.php', o);
    }).then(function (r) {
      if (r && r.ok && r.url) location.href = r.url;
      else showError(T.pay_error);
    }).catch(function (e) {
      if (e && e.statusCode === 'CANCELED') return;
      showError(T.pay_error);
    });
  }
  window.nikpakGPayLoaded = gpayStart;
  if (window.google && window.google.payments) gpayStart();

  // ---------- crypto transfer ----------
  var rates = {}, ratesState = 'idle';
  function coin() { var c = document.querySelector('input[name="coin"]:checked'); return c ? c.value : null; }
  function fetchRates() {
    if (ratesState !== 'idle') return;
    var ids = [];
    Object.keys(cfg.crypto).forEach(function (k) { var c = cfg.crypto[k]; if (!c.stable && c.coingecko && ids.indexOf(c.coingecko) < 0) ids.push(c.coingecko); });
    if (!ids.length) return;
    ratesState = 'loading';
    fetch('https://api.coingecko.com/api/v3/simple/price?vs_currencies=usd&ids=' + ids.join(','))
      .then(function (r) { return r.json(); })
      .then(function (j) { ids.forEach(function (id) { if (j[id] && j[id].usd) rates[id] = j[id].usd; }); ratesState = 'done'; updateCrypto(); })
      .catch(function () { ratesState = 'error'; updateCrypto(); });
  }
  function updateCrypto() {
    var sec = $('crypto');
    if (!sec || sec.hidden) return;
    var key = coin(); if (!key) return;
    var c = cfg.crypto[key], o = order();
    var sym = c.symbol || key;
    var amount = null, rateTxt = '';
    if (c.stable) {
      amount = o.total.toFixed(c.decimals || 2);
      rateTxt = T.stable_note.replace('{coin}', sym);
    } else {
      fetchRates();
      var r = rates[c.coingecko];
      if (r) {
        var a = o.total / r, d = c.decimals || 6, m = Math.pow(10, d);
        amount = (Math.ceil(a * m) / m).toFixed(d);
        rateTxt = T.rate_note.replace('{coin}', sym).replace('{rate}', money(Math.round(r)));
      } else rateTxt = ratesState === 'error' ? T.rate_error : T.rate_loading;
    }
    $('c-amount').textContent = amount ? amount + ' ' + sym : '…';
    $('c-rate').textContent = rateTxt;
    $('c-network').textContent = c.network;
    $('c-address').textContent = c.address;
    $('c-warn').innerHTML = T.network_warn.replace('{coin}', sym).replace('{network}', c.network);
    // QR code: payment URI when the wallet understands it, else the plain address
    var uri = c.address;
    if (c.uri === 'bitcoin' && amount) uri = 'bitcoin:' + c.address + '?amount=' + amount;
    if (window.qrcode) {
      var qr = qrcode(0, 'M'); qr.addData(uri); qr.make();
      $('c-qr').innerHTML = qr.createSvgTag({ cellSize: 4, margin: 0, scalable: true });
    }
    // e-mail with NikPak in the subject
    var licence = o.type === 'company' ? T.company + ', ' + o.seats : T.personal;
    var body = T.mail_body.replace('{coin}', sym).replace('{network}', c.network).replace('{amount}', amount ? amount + ' ' + sym : '')
      .replace('{licence}', licence).replace('{holder}', o.name).replace('{email}', o.email);
    $('c-mail').href = 'mailto:' + cfg.email + '?subject=' + encodeURIComponent(T.mail_subject.replace('{coin}', sym + ' ' + c.network)) + '&body=' + encodeURIComponent(body);
    var p = new URLSearchParams({ topic: 'crypto', coin: key, amount: amount ? amount + ' ' + sym : '', licence: o.type, seats: String(o.seats), holder: o.name, email: o.email });
    $('c-form').href = 'contact.html?' + p.toString();
  }
  document.querySelectorAll('input[name="coin"]').forEach(function (r) { r.addEventListener('change', updateCrypto); });
  ['holder', 'email'].forEach(function (id) { $(id).addEventListener('input', updateCrypto); });
  $('c-copy') && $('c-copy').addEventListener('click', function () {
    var t = $('c-address').textContent;
    (navigator.clipboard ? navigator.clipboard.writeText(t) : Promise.reject()).then(function () { window.nikpakToast && nikpakToast(cfg.copied); })
      .catch(function () { var r = document.createRange(); r.selectNodeContents($('c-address')); var s = getSelection(); s.removeAllRanges(); s.addRange(r); });
  });

  update();
  selectMethod();
  updateCrypto();
})();
