<?php
/**
 * NikPak web site secrets. COPY THIS FILE OUTSIDE THE WEB ROOT as shop-config.php.
 * It is found automatically in the folder that CONTAINS the web root, for example on IIS:
 *   web root   C:\inetpub\nikpak\www\            (content of dist\)
 *   this file  C:\inetpub\nikpak\nikpak-server\shop-config.php   (next to license_store.php)
 *          or  C:\inetpub\nikpak\shop-config.php
 * or set the environment variable NIKPAK_SHOP_CONFIG (IIS Manager > FastCGI Settings > Environment Variables).
 * The IIS application pool user (IIS_IUSRS) needs read access to it, and write access to data_dir.
 * Never commit the filled-in file. Use test/sandbox keys first, then switch to live keys.
 * Windows paths may use forward slashes: 'C:/inetpub/nikpak/nikpak-server'.
 */
return [
    // Folder of the licence kit (server/ of the NikPak project: license_store.php, config.php, ../keys/)
    'server_dir' => 'C:/inetpub/nikpak/nikpak-server',

    // Private folder for orders and the shop log (created automatically, must NOT be in the web root)
    'data_dir'   => 'C:/inetpub/nikpak/nikpak-data',

    'site_url'     => 'https://nikpak.nikkoid.com',
    'notify_email' => 'contact@nikkoid.com',     // receives a copy of every sale

    // Outgoing e-mail (contact form, licence keys). Recommended on IIS: PHP's mail() on Windows
    // cannot log in to an SMTP server. Leave 'host' empty to use mail() and the SMTP settings of php.ini.
    // 'secure': 'tls' = STARTTLS on port 587, 'ssl' = port 465, '' = no encryption (port 25).
    'smtp' => [
        'host'   => 'nikkoid-com-smtp.dynu.com',
        'port'   => 587,
        'secure' => 'tls',
        'user'   => 'nikko@nikkoid.com',
        'pass'   => 'REPLACE_ME', // the Nikkoid mailbox password (never commit the filled-in file)
    ],

    // Stripe: cards, Apple Pay and the Google Pay button (gateway "stripe")
    // dashboard.stripe.com > Developers > API keys / Webhooks
    'stripe' => [
        'secret_key'     => 'sk_test_REPLACE_ME',
        'webhook_secret' => 'whsec_REPLACE_ME',
    ],

    // PayPal REST app: developer.paypal.com > Apps & Credentials
    'paypal' => [
        'client_id' => 'REPLACE_ME',
        'secret'    => 'REPLACE_ME',
        'sandbox'   => true,
    ],

    // Paddle Billing: vendors.paddle.com > Developer tools > Authentication / Notifications
    // Set the "default payment link" to https://nikpak.nikkoid.com/pay.html (Checkout settings).
    'paddle' => [
        'api_key'        => 'REPLACE_ME',
        'webhook_secret' => 'pdl_ntfset_REPLACE_ME',
        'sandbox'        => true,
    ],

    // Binance Pay merchant: merchant.binance.com > Developers > API keys
    'binancepay' => [
        'api_key'    => 'REPLACE_ME',          // "Certificate SN" in the API headers
        'secret_key' => 'REPLACE_ME',
        'currency'   => 'USDT',                // order currency; 1 USDT is charged per USD of the price
    ],
];
