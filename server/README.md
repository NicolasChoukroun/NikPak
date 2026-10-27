# NikPak licence server kit (PHP 7.2+ / MySQL 5.7)

This folder is the base for **https://nikpak.nikkoid.com**. It generates and stores
NikPak licence keys. The shop page (buy.php) and the payment provider
will be connected later.

| File | Purpose |
|---|---|
| `nikpak_license.php` | Key generation: Ed25519 signature over name + e-mail (the same algorithm as nikpak). |
| `license_store.php`  | Database: issue a key (once per payment), find keys by e-mail, build the e-mail text. |
| `schema.sql`         | The `nikpak_licenses` table (MySQL 5.7, InnoDB, utf8mb4). |
| `config.sample.php`  | Copy to `config.php` and fill in. |
| `tools/licenses.php` | Command-line tool: `issue`, `find`, `list`, `email`, `keyfile`. |

## 1. Generate your own keys (do this once, before the first sale)

The key pair in `keys/` was created during development. Make a fresh one on
your own PC and keep it secret:

```
cargo run --release --features keygen --bin nikpak-keygen -- newkeys --force
cargo build --release
```

This writes `keys/nikpak-private.key` (the **secret**) and
`src/license_pubkey.rs` (the public key compiled into nikpak). Rebuild nikpak
afterwards. Keys made with an old private key stop working. That is harmless
until you have sold licences, but never change the key after that.

**Never** put the private key in the web root, in Git, or in a NikPak
download.

## 2. Install on the web host

1. PHP 7.2 or later has `sodium` built in. On PHP 7.0/7.1, install
   `paragonie/sodium_compat` with Composer and `require` its autoload.
2. In phpMyAdmin, create a database and import `schema.sql`.
3. Upload this folder **outside** the public `www`/`public_html` folder if
   your host allows it (for example `/home/you/nikpak-server/`). Only
   `buy.php` and the webhook script go into the web root, and they
   `require` these files.
4. Upload `keys/nikpak-private.key` next to this folder (`../keys/`), also
   outside the web root, with permissions 600.
5. Copy `config.sample.php` to `config.php`. Enter the database login, and
   in `private_key` the **path** of the key file if it is not in `../keys/`.
6. Test it from SSH: `php tools/licenses.php issue "Test" t@example.com`, then
   check the key in nikpak: `nikpak register "Test" t@example.com <key>`.
   A company licence: `php tools/licenses.php issue "ACME" it@acme.example company 25 "PO 1234"`.

## 3. Payment flow (to build with buy.php)

1. buy.php shows the two products: Personal **$19**, and Company (a seat count
   field) with volume prices per seat, like WinRAR:

   | Seats | 1–9 | 10–24 | 25–49 | 50–99 | 100–199 | 200–499 | 500+ |
   |---|---|---|---|---|---|---|---|
   | USD per seat | 19 | 15 | 12 | 9 | 7 | 6 | 5 |

   `nikpak_price('company', $seats)` returns the per-seat price and the total.
   Always compute the amount on the server; never trust a price sent by the browser.
2. The customer pays through the provider (Paddle, FastSpring, Stripe or PayPal).
3. The provider calls your **webhook** (for example `webhook.php`). It must:
   - verify the provider's signature;
   - call `nikpak_issue($pdo, $name, $email, 'personal'|'company', $seats,
     array('amount' => $amount, 'currency' => 'USD', 'provider' => 'paddle', 'payment_ref' => $payment_id))`.
     This is idempotent: the same `payment_id` always returns the same key,
     so provider retries are safe.
   - send `nikpak_license_email(...)` with `mail()` or SMTP.
4. A "lost key" page can use `nikpak_find_by_email()` and e-mail the keys to
   that address only (never display them on the page).

The customer registers in NikPak (the Register button, or
`nikpak register "Name" email key`). A company can also put a `nikpak.key`
file next to nikpak.exe on every seat: `php tools/licenses.php keyfile ID > nikpak.key`.

## 4. VAT and the legal side (France)

Selling software downloads to EU consumers means charging the **buyer's
country VAT rate** (OSS scheme). The simplest option is a **merchant of
record** such as Paddle or FastSpring. They sell in their own name, collect
and pay VAT worldwide, and send you the net amount. Stripe or PayPal alone
leave VAT declarations to you. Check with an accountant. This is
not legal or tax advice.
