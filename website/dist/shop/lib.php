<?php
/**
 * NikPak shop - shared code for checkout.php, gpay.php and the payment webhooks.
 * PHP 7.2+ (also PHP 8.x), extensions: curl, openssl, sodium, pdo_mysql, mbstring.
 *
 * Secrets live in shop-config.php OUTSIDE the web root (see shop-config.sample.php).
 * Licences are created with the existing kit in server/ (license_store.php: nikpak_issue).
 */
require_once dirname(__DIR__) . '/inc/common.php';

/** Shop settings (secret). */
function shop_config(): array
{
    $cfg = nikpak_private_config();   // inc/common.php: NIKPAK_SHOP_CONFIG, ..\nikpak-server\, ..\
    if (!$cfg) throw new RuntimeException('shop-config.php not found (copy shop/shop-config.sample.php outside the web root)');
    return $cfg;
}

function shop_url(string $path): string
{
    return rtrim((string) (shop_config()['site_url'] ?? nikpak_site()['domain']), '/') . '/' . ltrim($path, '/');
}

function shop_thanks_url(string $lang, string $status, string $provider = ''): string
{
    return shop_url($lang . '/thanks.html?status=' . $status . ($provider !== '' ? '&provider=' . $provider : ''));
}

/** Never throws: falls back to the PHP error log when the data folder is not configured. */
function shop_log(string $msg): void
{
    $line = gmdate('Y-m-d H:i:s') . ' ' . $msg;
    try {
        if (@file_put_contents(shop_data_dir() . '/shop.log', $line . "\n", FILE_APPEND | LOCK_EX) !== false) return;
    } catch (Throwable $e) {
    }
    error_log('[nikpak shop] ' . $line);
}

function shop_data_dir(): string
{
    $dir = (string) (shop_config()['data_dir'] ?? (dirname(__DIR__, 2) . '/nikpak-data'));
    if (!is_dir($dir . '/orders')) @mkdir($dir . '/orders', 0700, true);
    return $dir;
}

/* ------------------------------------------------------------------ prices */

/** Same tiers as the licence (LICENSE.txt section 3) and nikpak_price() in server/nikpak_license.php. */
function shop_price(string $type, int $seats): array
{
    if ($type !== 'company') return ['per_seat' => 19, 'seats' => 1, 'total' => 19];
    $seats = max(1, min(100000, $seats));
    foreach ([500 => 5, 200 => 6, 100 => 7, 50 => 9, 25 => 12, 10 => 15, 1 => 19] as $min => $usd) {
        if ($seats >= $min) return ['per_seat' => $usd, 'seats' => $seats, 'total' => $usd * $seats];
    }
    return ['per_seat' => 19, 'seats' => $seats, 'total' => 19 * $seats];
}

/* ------------------------------------------------------------------ orders */

/** Validates what the browser sent and computes the price on the server. */
function shop_order_from_input(array $in): array
{
    $type = ($in['type'] ?? '') === 'company' ? 'company' : 'personal';
    $p = shop_price($type, (int) ($in['seats'] ?? 1));
    $name = nikpak_clean_line($in['name'] ?? '', 120);
    $email = strtolower(nikpak_clean_line($in['email'] ?? '', 190));
    if (mb_strlen($name) < 2 || !nikpak_valid_email($email)) throw new InvalidArgumentException('invalid');
    return [
        'id'       => 'NP' . gmdate('ymd') . strtoupper(bin2hex(random_bytes(8))),   // 24 chars, letters and digits
        'type'     => $type,
        'seats'    => $p['seats'],
        'per_seat' => $p['per_seat'],
        'total'    => $p['total'],
        'currency' => 'USD',
        'name'     => $name,
        'email'    => $email,
        'lang'     => nikpak_lang_or_null($in['lang'] ?? null) ?? 'en',
        'created'  => time(),
    ];
}

function shop_item_name(array $o): string
{
    return $o['type'] === 'company' ? 'NikPak company licence (' . $o['seats'] . ' seat' . ($o['seats'] > 1 ? 's' : '') . ')' : 'NikPak personal licence';
}

function shop_save_order(array $o): void
{
    file_put_contents(shop_data_dir() . '/orders/' . $o['id'] . '.json', json_encode($o, JSON_UNESCAPED_UNICODE | JSON_PRETTY_PRINT), LOCK_EX);
}

function shop_load_order(string $id): ?array
{
    if (!preg_match('/^NP[0-9A-F]{6,40}$/i', $id)) return null;
    $f = shop_data_dir() . '/orders/' . $id . '.json';
    return is_file($f) ? json_decode((string) file_get_contents($f), true) : null;
}

/**
 * The payment is confirmed: create the licence (once per payment reference) and e-mail it.
 * Safe to call several times for the same payment (webhooks are retried).
 */
function shop_fulfil(array $o, string $provider, string $ref, float $paid, string $currency): array
{
    $c = shop_config();
    require_once rtrim((string) $c['server_dir'], '/\\') . '/license_store.php';
    $pdo = nikpak_db();
    $lic = nikpak_issue($pdo, $o['name'], $o['email'], $o['type'], (int) $o['seats'], [
        'payment_ref' => $provider . ':' . $ref,
        'provider'    => $provider,
        'amount'      => $paid,
        'currency'    => $currency,
        'note'        => 'order ' . $o['id'],
    ]);
    $marker = shop_data_dir() . '/orders/' . $o['id'] . '.mailed';
    if (!is_file($marker)) {
        $att = [];
        if ($lic['type'] === 'company' && function_exists('nikpak_license_file')) {
            $att[] = ['name' => 'nikpak.key', 'type' => 'text/plain', 'data' => nikpak_license_file($lic['name'], $lic['email'], $lic['license_key'])];
        }
        $sent = nikpak_mail($lic['email'], 'NikPak - your licence key', nikpak_license_email($lic), (string) nikpak_site()['contact_email'], $att);
        $notify = (string) ($c['notify_email'] ?? nikpak_site()['contact_email']);
        nikpak_mail($notify, 'NikPak - sale ' . $provider . ' ' . $o['total'] . ' USD',
            "New sale\n\nProvider: $provider\nReference: $ref\nPaid: $paid $currency\nOrder: {$o['id']}\n"
            . "Licence: {$lic['type']} x{$lic['seats']}\nHolder: {$lic['name']}\nE-mail: {$lic['email']}\nKey e-mail sent: " . ($sent ? 'yes' : 'NO - send it by hand') . "\n");
        if ($sent) @file_put_contents($marker, (string) time());
        shop_log("fulfilled {$o['id']} $provider $ref $paid $currency -> licence #{$lic['id']}" . ($sent ? '' : ' (MAIL FAILED)'));
    }
    return $lic;
}

/* ------------------------------------------------------------------ HTTP */

/** Raw HTTP call to a payment API; returns [status, decoded body]. */
function shop_http(string $method, string $url, array $headers = [], ?string $body = null, ?string $userpwd = null): array
{
    $ch = curl_init($url);
    curl_setopt_array($ch, [
        CURLOPT_CUSTOMREQUEST  => $method,
        CURLOPT_RETURNTRANSFER => true,
        CURLOPT_HTTPHEADER     => $headers,
        CURLOPT_TIMEOUT        => 30,
        CURLOPT_CONNECTTIMEOUT => 10,
    ]);
    if ($body !== null) curl_setopt($ch, CURLOPT_POSTFIELDS, $body);
    if ($userpwd !== null) curl_setopt($ch, CURLOPT_USERPWD, $userpwd);
    $raw = curl_exec($ch);
    $status = (int) curl_getinfo($ch, CURLINFO_RESPONSE_CODE);
    if ($raw === false) {
        shop_log("HTTP error $url: " . curl_error($ch));
        return [0, []];
    }
    $json = json_decode((string) $raw, true);
    return [$status, is_array($json) ? $json : ['_raw' => (string) $raw]];
}

function shop_read_json_body(): array
{
    $raw = (string) file_get_contents('php://input');
    $j = json_decode($raw, true);
    return is_array($j) ? $j : $_POST;
}

/* ------------------------------------------------------------------ Stripe */

function stripe_call(string $method, string $path, array $params = []): array
{
    $key = (string) shop_config()['stripe']['secret_key'];
    return shop_http($method, 'https://api.stripe.com/v1/' . $path,
        ['Authorization: Bearer ' . $key, 'Content-Type: application/x-www-form-urlencoded', 'Stripe-Version: 2024-06-20'],
        $params ? http_build_query($params) : null);
}

function stripe_metadata(array $o): array
{
    return ['nikpak_order' => $o['id'], 'type' => $o['type'], 'seats' => (string) $o['seats'], 'name' => $o['name'], 'email' => $o['email'], 'lang' => $o['lang']];
}

/** Stripe-Signature header check (t=...,v1=...), 5 minutes tolerance. */
function stripe_verify(string $payload, string $header, string $secret): bool
{
    $t = null;
    $sigs = [];
    foreach (explode(',', $header) as $part) {
        [$k, $v] = array_pad(explode('=', trim($part), 2), 2, '');
        if ($k === 't') $t = (int) $v;
        if ($k === 'v1') $sigs[] = $v;
    }
    if ($t === null || abs(time() - $t) > 300) return false;
    $expected = hash_hmac('sha256', $t . '.' . $payload, $secret);
    foreach ($sigs as $s) if (hash_equals($expected, $s)) return true;
    return false;
}

/* ------------------------------------------------------------------ PayPal */

function paypal_base(): string
{
    return !empty(shop_config()['paypal']['sandbox']) ? 'https://api-m.sandbox.paypal.com' : 'https://api-m.paypal.com';
}

function paypal_token(): string
{
    $p = shop_config()['paypal'];
    [$st, $j] = shop_http('POST', paypal_base() . '/v1/oauth2/token', ['Content-Type: application/x-www-form-urlencoded'],
        'grant_type=client_credentials', $p['client_id'] . ':' . $p['secret']);
    if ($st !== 200 || empty($j['access_token'])) throw new RuntimeException('PayPal authentication failed');
    return (string) $j['access_token'];
}

function paypal_call(string $method, string $path, ?array $body = null, string $token = ''): array
{
    $token = $token !== '' ? $token : paypal_token();
    return shop_http($method, paypal_base() . $path,
        ['Authorization: Bearer ' . $token, 'Content-Type: application/json', 'PayPal-Request-Id: ' . bin2hex(random_bytes(12))],
        $body === null ? null : json_encode($body, JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES));
}

/* ------------------------------------------------------------------ Paddle Billing */

function paddle_call(string $method, string $path, ?array $body = null): array
{
    $p = shop_config()['paddle'];
    $base = !empty($p['sandbox']) ? 'https://sandbox-api.paddle.com' : 'https://api.paddle.com';
    return shop_http($method, $base . $path, ['Authorization: Bearer ' . $p['api_key'], 'Content-Type: application/json'],
        $body === null ? null : json_encode($body, JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES));
}

/** Paddle-Signature: ts=...;h1=... over "ts:body". */
function paddle_verify(string $payload, string $header, string $secret): bool
{
    $ts = null;
    $h1 = [];
    foreach (explode(';', $header) as $part) {
        [$k, $v] = array_pad(explode('=', trim($part), 2), 2, '');
        if ($k === 'ts') $ts = $v;
        if ($k === 'h1') $h1[] = $v;
    }
    if ($ts === null || abs(time() - (int) $ts) > 300) return false;
    $expected = hash_hmac('sha256', $ts . ':' . $payload, $secret);
    foreach ($h1 as $s) if (hash_equals($expected, $s)) return true;
    return false;
}

/* ------------------------------------------------------------------ Binance Pay */

const BINANCEPAY_BASE = 'https://bpay.binanceapi.com';

/** Signed request to the Binance Pay merchant API (HMAC-SHA512 of timestamp, nonce and body). */
function binancepay_call(string $path, $body): array
{
    $b = shop_config()['binancepay'];
    $json = json_encode($body, JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);
    $ts = (string) (int) round(microtime(true) * 1000);
    $chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';
    $nonce = '';
    for ($i = 0; $i < 32; $i++) $nonce .= $chars[random_int(0, 51)];
    $sig = strtoupper(hash_hmac('sha512', $ts . "\n" . $nonce . "\n" . $json . "\n", (string) $b['secret_key']));
    return shop_http('POST', BINANCEPAY_BASE . $path, [
        'Content-Type: application/json',
        'BinancePay-Timestamp: ' . $ts,
        'BinancePay-Nonce: ' . $nonce,
        'BinancePay-Certificate-SN: ' . $b['api_key'],
        'BinancePay-Signature: ' . $sig,
    ], $json);
}

/** Binance's public key for webhook signatures (cached for a day). */
function binancepay_public_key(bool $refresh = false): ?string
{
    $cache = shop_data_dir() . '/binancepay-cert.json';
    if (!$refresh && is_file($cache) && filemtime($cache) > time() - 86400) {
        $c = json_decode((string) file_get_contents($cache), true);
        if (!empty($c['certPublic'])) return (string) $c['certPublic'];
    }
    [$st, $j] = binancepay_call('/binancepay/openapi/certificates', new stdClass());   // body must be "{}"
    $cert = $j['data'][0] ?? null;
    if ($st !== 200 || !is_array($cert) || empty($cert['certPublic'])) return null;
    @file_put_contents($cache, json_encode($cert), LOCK_EX);
    return (string) $cert['certPublic'];
}

/** Webhook signature: base64 RSA-SHA256 over "timestamp\nnonce\nbody\n". */
function binancepay_verify(string $payload, string $ts, string $nonce, string $signature): bool
{
    if ($ts === '' || $nonce === '' || $signature === '') return false;
    if (abs(time() * 1000 - (int) $ts) > 10 * 60 * 1000) return false;
    $data = $ts . "\n" . $nonce . "\n" . $payload . "\n";
    foreach ([false, true] as $refresh) {
        $pem = binancepay_public_key($refresh);
        if ($pem === null) return false;
        $key = openssl_pkey_get_public($pem);
        if ($key !== false && openssl_verify($data, (string) base64_decode($signature, true), $key, OPENSSL_ALGO_SHA256) === 1) return true;
    }
    return false;
}
