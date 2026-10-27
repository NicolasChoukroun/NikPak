# NikPak web site

Multi-language web site for **https://nikpak.nikkoid.com** in the look of the NikPak / unNik windows
(navy gradient, blue accent, progress ring). Ten languages, the same as the program:
English, 中文, हिन्दी, Español, العربية (right to left), Français, বাংলা, Português, Русский, Bahasa Indonesia.

Every link points to a page file (`en/index.html`, `fr/buy.html` …), never to a bare folder, so the site
works on servers where `index.html` is not a default document.

## Pages (in every language: `xx/<page>.html`)

| Page | Content |
|---|---|
| `index.html` | Home: animated NikPak window, key figures, features, chart, unNik, command line, prices |
| `features.html` | Media codecs, Office/PDF recompression, levels, encryption, quantum resistance, safety, limits, roadmap |
| `benchmarks.html` | The section 11.7 benchmarks of the technical guide: charts and tables vs RAR 7, 7-Zip, ZIP |
| `guide.html` | User guide: install, NikPak window, unNik, registration, command line, passwords, exit codes, Total Commander |
| `buy.html` | Shop: personal $19 / company per seat with the volume tiers, seat calculator, Google Pay, card (Stripe), PayPal, Paddle, Binance Pay, crypto transfer (BTC, ETH, BNB, USDT/USDC on TRON, Ethereum and BNB Smart Chain BEP-20) |
| `contact.html` | Contact form (PHP); topic "Crypto payment" with coin, transaction ID, licence and seats |
| `thanks.html` | Result page after a payment (`?status=ok|pending|cancel|error`) |

Root files: `index.html` + `index.php` (send visitors to their language), `buy.php` (the address opened by
NikPak's **Buy a licence** button, `src/license.rs`), `contact.php`, `pay.html` (Paddle checkout page),
`sitemap.xml`, `robots.txt`, `web.config` (IIS).

## Hosting: IIS with PHP 7.2 or newer

The PHP code runs on PHP 7.2 to 8.x (checked with PHPCompatibility). `dist\web.config` replaces Apache's
`.htaccess` and only uses built-in IIS features (no URL Rewrite module):
- default document `index.php` then `index.html` (only for the bare address; all links name a page file);
- MIME type for `.webp` images (older IIS returns 404.3 without it);
- blocks `inc\`, `shop\lib.php`, any `shop-config` file, `site-public.json` and `*.log`;
- one-week browser cache for `assets\`.

In `php.ini` enable: `extension=curl`, `openssl`, `mbstring`, `sodium` (licence keys), `pdo_mysql` (licence database).

### Folder layout on the server (example)

```
D:\Websites\nikpak\
  www\                     <- IIS web root: the CONTENT of dist\
  nikpak-server\           <- the licence kit (server\ of the project) + shop-config.php
  keys\nikpak-private.key  <- licence private key (the kit looks in ..\keys\)
  nikpak-data\             <- orders and shop.log (created automatically)
```

`shop-config.php` is found automatically, in this order:
1. the path in the environment variable `NIKPAK_SHOP_CONFIG` (IIS Manager > FastCGI Settings > Environment Variables);
2. `<parent of web root>\nikpak-server\shop-config.php`;
3. `<parent of web root>\shop-config.php`;
4. last resort: `<web root>\shop\shop-config.php` (blocked from the web by web.config).

Permissions (administrator command prompt):
```
icacls "D:\Websites\nikpak\nikpak-server" /grant "IIS_IUSRS:(OI)(CI)R"
icacls "D:\Websites\nikpak\keys" /grant "IIS_IUSRS:(OI)(CI)R"
mkdir "D:\Websites\nikpak\nikpak-data"
icacls "D:\Websites\nikpak\nikpak-data" /grant "IIS_IUSRS:(OI)(CI)M"
```
In `shop-config.php` write Windows paths with forward slashes: `'D:/Websites/nikpak/nikpak-server'`.

## Build

```
python build.py            # writes dist/  (upload the CONTENT of dist/ to the web root)
python check_i18n.py       # checks that every language has the same texts as English
```

Texts: `i18n/<lang>.json` (English is the reference). Public settings: `site.json`.
Styles and scripts: `static/`. PHP: `php/`. Python 3.8+, no packages needed.

## Pictures

The screenshots in `static/img/` must come from the current build, because the
window changes (buttons, the skins box beside it) between versions:

```
.\tools\site-shots.ps1 -NoBuild      # captures every state into website\shots-raw\
python tools\site-shots-to-webp.py     # turns them into static/img/*.webp
python website/build.py                # rebuild dist
```

`site-shots.ps1` starts the program with the test hooks (`NIKPAK_GUI_SKIN`,
`NIKPAK_GUI_SKINMENU`, `NIKPAK_GUI_MODAL`, …), so no clicking is needed. The
skin tiles themselves are made by `website/tools/gen_skin_tiles.py`, which
needs the backgrounds dumped by `tools/build-shots.ps1`.

## Settings to fill in before going live

### `site.json` (public, then rebuild)
- `contact_email`: the address that receives the contact form and crypto payment e-mails (now `contact@nikkoid.com`).
- `mail_from`: sender of the e-mails the site sends (an address of your domain, so mail is not rejected).
- `downloads`: the three downloads (Windows, macOS, Linux/Unix), `{v}` = `version`. Put the files in
  `dist/downloads/`: `nikpak-<v>-setup.exe` (made by `buildall.ps1`), `nikpak-<v>-macos.zip` and
  `nikpak-<v>-linux-x86_64.tar.gz` (made by `./package-unix.sh` on a Mac and on Linux).
  The site highlights the visitor's system; Windows is the default.
- `crypto`: your **wallet addresses** for each coin and network. The same 0x… address can receive ETH/USDC on
  Ethereum and BNB/USDT/USDC on BNB Smart Chain. Remove an entry to hide that coin.
- `payments.googlepay`: `environment` (`TEST` → `PRODUCTION`), your Google Pay `merchant_id`, Stripe publishable key.
- `payments.paddle.client_token` and `environment` (`sandbox` → `production`).
- `payments.<method>.enabled`: hide a payment method you do not use.

### `shop-config.php` (SECRET, outside the web root)
Copy `dist\shop\shop-config.sample.php` to `nikpak-server\shop-config.php` (see the folder layout above).
It holds the SMTP login, the Stripe, PayPal, Paddle and Binance Pay API keys, the path of the licence kit
(`server_dir`) and a private `data_dir` for orders and the shop log.

## How a sale works

1. `buy.html` computes the price for display; `shop/checkout.php` (or `shop/gpay.php`) **computes it again on the
   server** with the same tiers as LICENSE.txt (1–9 $19, 10–24 $15, 25–49 $12, 50–99 $9, 100–199 $7, 200–499 $6, 500+ $5).
2. The buyer pays on the provider's page.
3. The provider confirms the payment:
   - Stripe and Google Pay → `shop/stripe-webhook.php` (events `checkout.session.completed`,
     `checkout.session.async_payment_succeeded`, `payment_intent.succeeded`)
   - PayPal → `shop/paypal-return.php` (captures the payment)
   - Paddle → `shop/paddle-webhook.php` (`transaction.completed`; set the default payment link to `/pay.html`)
   - Binance Pay → `shop/binancepay-webhook.php` (signature checked with Binance's public key)
4. `shop_fulfil()` issues the licence with the existing kit (`nikpak_issue`, one licence per payment, safe on
   retries), e-mails the key to the buyer (companies also get `nikpak.key`) and a copy of the sale to you.

**Crypto transfers** are manual: the buyer e-mails the transaction (subject must contain **NikPak**) or uses the
contact form topic *Crypto payment*. The e-mail you receive contains the exact `php tools/licenses.php issue …`
command to create the key once you have checked the transaction.

## E-mail

`contact.php` and the shop send e-mail through **SMTP with login** when `smtp` is filled in `shop-config.php`
(host, port 587 + `tls` or 465 + `ssl`, user, password): use this on IIS, because PHP's `mail()` on Windows
cannot log in to a mail server. With `smtp.host` empty, `mail()` is used with the `SMTP` / `smtp_port`
settings of php.ini. Every e-mail subject starts with **NikPak** so it can be filtered; the contact form
shows the subject before sending.

## Not tested here
The payment providers' live APIs (Stripe, PayPal, Paddle, Binance Pay, Google Pay) could not be reached from
the build machine. Test each one in sandbox / test mode before switching to live keys.
