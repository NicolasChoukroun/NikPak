<?php
/**
 * NikPak web site - helpers shared by index.php, buy.php, contact.php and shop/.
 * PHP 7.2 or newer (also PHP 8.x). Runs on IIS (web.config) and Apache.
 */

const NIKPAK_LANGS = ['en', 'zh', 'hi', 'es', 'ar', 'fr', 'bn', 'pt', 'ru', 'id'];

// PHP 7 polyfills (built into PHP 8)
if (!function_exists('str_starts_with')) {
    function str_starts_with($haystack, $needle) { return strncmp((string) $haystack, (string) $needle, strlen((string) $needle)) === 0; }
}
if (!function_exists('str_contains')) {
    function str_contains($haystack, $needle) { return $needle === '' || strpos((string) $haystack, (string) $needle) !== false; }
}

/**
 * Private settings (secret): shop-config.php OUTSIDE the web root. Holds the payment API keys
 * and the SMTP login. Searched in: environment variable NIKPAK_SHOP_CONFIG, then the folder
 * next to the web root (..\nikpak-server\shop-config.php, ..\shop-config.php). Returns [] if absent.
 */
function nikpak_private_config(): array
{
    static $cfg = null;
    if ($cfg !== null) return $cfg;
    $root = dirname(__DIR__);
    $candidates = array_filter([
        getenv('NIKPAK_SHOP_CONFIG') ?: null,
        dirname($root) . '/nikpak-server/shop-config.php',
        dirname($root) . '/shop-config.php',
        $root . '/shop/shop-config.php',   // last resort, blocked from the web by web.config
    ]);
    foreach ($candidates as $f) {
        if (is_file($f)) {
            $c = require $f;
            $cfg = is_array($c) ? $c : [];
            return $cfg;
        }
    }
    $cfg = [];
    return $cfg;
}

/** Public settings written by build.py (site.json). */
function nikpak_site(): array
{
    static $site = null;
    if ($site === null) {
        $file = dirname(__DIR__) . '/site-public.json';
        $site = is_file($file) ? (json_decode((string) file_get_contents($file), true) ?: []) : [];
        $site += ['langs' => NIKPAK_LANGS, 'contact_email' => 'contact@nikkoid.com', 'domain' => 'https://nikpak.nikkoid.com', 'mail_from' => 'NikPak <noreply@nikkoid.com>'];
    }
    return $site;
}

/** A known language code, or null. */
function nikpak_lang_or_null($code): ?string
{
    return is_string($code) && in_array($code, NIKPAK_LANGS, true) ? $code : null;
}

/** The visitor's language: explicit choice (cookie), then the browser's Accept-Language, else English. */
function nikpak_pick_lang(): string
{
    if ($l = nikpak_lang_or_null($_GET['lang'] ?? null)) return $l;
    if ($l = nikpak_lang_or_null($_COOKIE['nikpak_lang'] ?? null)) return $l;
    $best = 'en';
    $bestQ = 0.0;
    foreach (explode(',', (string) ($_SERVER['HTTP_ACCEPT_LANGUAGE'] ?? '')) as $part) {
        $bits = explode(';', trim($part));
        $code = strtolower(explode('-', trim($bits[0]))[0]);
        $q = 1.0;
        foreach (array_slice($bits, 1) as $p) {
            if (str_starts_with(trim($p), 'q=')) $q = (float) substr(trim($p), 2);
        }
        if (in_array($code, NIKPAK_LANGS, true) && $q > $bestQ) {
            $best = $code;
            $bestQ = $q;
        }
    }
    return $best;
}

/** Redirect to a page of the visitor's language, e.g. nikpak_redirect_page('buy') -> /fr/buy.html */
function nikpak_redirect_page(string $page) {
    $lang = nikpak_pick_lang();
    $q = $_GET;
    unset($q['lang']);
    $qs = $q ? '?' . http_build_query($q) : '';
    header('Cache-Control: no-store');
    header('Vary: Accept-Language, Cookie');
    header('Location: ' . $lang . '/' . $page . '.html' . $qs, true, 302);
    exit;
}

/** Remove line breaks and control characters (header injection) and trim. */
function nikpak_clean_line($s, int $max = 200): string
{
    $s = is_string($s) ? $s : '';
    $s = preg_replace('/[\x00-\x1F\x7F]+/u', ' ', $s) ?? '';
    $s = trim(preg_replace('/\s+/u', ' ', $s) ?? '');
    return mb_substr($s, 0, $max, 'UTF-8');
}

function nikpak_clean_text($s, int $max = 8000): string
{
    $s = is_string($s) ? $s : '';
    $s = str_replace("\r\n", "\n", $s);
    $s = preg_replace('/[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]/u', '', $s) ?? '';
    return mb_substr(trim($s), 0, $max, 'UTF-8');
}

function nikpak_valid_email(string $e): bool
{
    return strlen($e) <= 190 && filter_var($e, FILTER_VALIDATE_EMAIL) !== false;
}

/** Parses "Name <address>" into [name, address]. */
function nikpak_split_from(string $from): array
{
    if (preg_match('/^\s*(.*?)\s*<([^>]+)>\s*$/', $from, $m)) return [trim($m[1], " \"'"), trim($m[2])];
    return ['', trim($from)];
}

/**
 * Sends a UTF-8 e-mail with PHP's mail(). The subject always contains "NikPak".
 * $attachments: [['name' => 'nikpak.key', 'type' => 'text/plain', 'data' => '...']]
 */
function nikpak_mail(string $to, string $subject, string $body, ?string $replyTo = null, array $attachments = [], ?string $from = null): bool
{
    if (stripos($subject, 'nikpak') === false) $subject = 'NikPak - ' . $subject;
    $subject = nikpak_clean_line($subject, 200);
    [$fromName, $fromAddr] = nikpak_split_from($from ?? (string) nikpak_site()['mail_from']);
    $headers = [];
    $headers[] = 'From: ' . ($fromName !== '' ? mb_encode_mimeheader($fromName, 'UTF-8', 'Q') . ' ' : '') . '<' . $fromAddr . '>';
    if ($replyTo !== null && nikpak_valid_email($replyTo)) $headers[] = 'Reply-To: ' . $replyTo;
    $headers[] = 'MIME-Version: 1.0';
    $headers[] = 'X-Mailer: NikPak web site';
    $body = str_replace("\r\n", "\n", $body);
    if (!$attachments) {
        $headers[] = 'Content-Type: text/plain; charset=UTF-8';
        $headers[] = 'Content-Transfer-Encoding: 8bit';
        $payload = $body;
    } else {
        $b = 'np_' . bin2hex(random_bytes(12));
        $headers[] = 'Content-Type: multipart/mixed; boundary="' . $b . '"';
        $payload = "--$b\nContent-Type: text/plain; charset=UTF-8\nContent-Transfer-Encoding: 8bit\n\n$body\n";
        foreach ($attachments as $a) {
            $name = preg_replace('/[^A-Za-z0-9._-]/', '_', (string) $a['name']);
            $payload .= "--$b\nContent-Type: " . ($a['type'] ?? 'application/octet-stream') . "; name=\"$name\"\n"
                . "Content-Transfer-Encoding: base64\nContent-Disposition: attachment; filename=\"$name\"\n\n"
                . chunk_split(base64_encode((string) $a['data']), 76, "\n") . "\n";
        }
        $payload .= "--$b--\n";
    }
    $encSubject = mb_encode_mimeheader($subject, 'UTF-8', 'B');

    // Authenticated SMTP when configured in shop-config.php ('smtp' => [...]): the usual choice on IIS,
    // because PHP's mail() on Windows can only talk to an SMTP server without login.
    $smtp = nikpak_private_config()['smtp'] ?? null;
    if (is_array($smtp) && !empty($smtp['host'])) {
        $msg = "Date: " . date('r') . "\r\n"
            . "Message-ID: <" . bin2hex(random_bytes(12)) . "@" . (explode('@', $fromAddr)[1] ?? 'localhost') . ">\r\n"
            . "To: <$to>\r\nSubject: $encSubject\r\n" . implode("\r\n", $headers) . "\r\n\r\n"
            . str_replace("\n", "\r\n", $payload);
        return nikpak_smtp_send($smtp, $fromAddr, $to, $msg);
    }
    // mail(): sendmail on Linux; on Windows/IIS the SMTP and smtp_port settings of php.ini (no login)
    $params = (PHP_OS_FAMILY !== 'Windows' && filter_var($fromAddr, FILTER_VALIDATE_EMAIL)) ? '-f' . $fromAddr : '';
    if (PHP_OS_FAMILY === 'Windows') @ini_set('sendmail_from', $fromAddr);
    return mail($to, $encSubject, $payload, implode("\r\n", $headers), $params);
}

/**
 * Minimal SMTP client: SSL (port 465) or STARTTLS (port 587), AUTH LOGIN.
 * $smtp = ['host' => 'smtp.example.com', 'port' => 587, 'secure' => 'tls'|'ssl'|'', 'user' => '...', 'pass' => '...']
 */
function nikpak_smtp_send(array $smtp, string $from, string $to, string $message): bool
{
    $secure = strtolower((string) ($smtp['secure'] ?? 'tls'));
    $port = (int) ($smtp['port'] ?? ($secure === 'ssl' ? 465 : 587));
    $host = (string) $smtp['host'];
    $ctx = stream_context_create(['ssl' => ['verify_peer' => empty($smtp['insecure']), 'verify_peer_name' => empty($smtp['insecure']), 'SNI_enabled' => true]]);
    $errno = 0;
    $errstr = '';
    $fp = @stream_socket_client(($secure === 'ssl' ? 'ssl://' : 'tcp://') . $host . ':' . $port, $errno, $errstr, 20, STREAM_CLIENT_CONNECT, $ctx);
    if (!$fp) {
        error_log("[nikpak mail] cannot connect to $host:$port: $errstr");
        return false;
    }
    stream_set_timeout($fp, 20);
    $read = function () use ($fp) {
        $data = '';
        while (($line = fgets($fp, 1024)) !== false) {
            $data .= $line;
            if (strlen($line) < 4 || $line[3] === ' ') break;
        }
        return $data;
    };
    $cmd = function ($c, $expect) use ($fp, $read) {
        if ($c !== null) fwrite($fp, $c . "\r\n");
        $r = $read();
        if (!in_array((int) substr($r, 0, 3), (array) $expect, true)) {
            throw new RuntimeException('SMTP ' . ($c !== null ? strtok($c, ' ') : 'greeting') . ': ' . trim($r));
        }
        return $r;
    };
    try {
        $cmd(null, 220);
        $me = gethostname() ?: 'localhost';
        $cmd("EHLO $me", 250);
        if ($secure === 'tls') {
            $cmd('STARTTLS', 220);
            if (!stream_socket_enable_crypto($fp, true, STREAM_CRYPTO_METHOD_TLS_CLIENT)) throw new RuntimeException('STARTTLS failed');
            $cmd("EHLO $me", 250);
        }
        if (!empty($smtp['user'])) {
            $cmd('AUTH LOGIN', 334);
            $cmd(base64_encode((string) $smtp['user']), 334);
            $cmd(base64_encode((string) ($smtp['pass'] ?? '')), 235);
        }
        $cmd("MAIL FROM:<$from>", 250);
        $cmd("RCPT TO:<$to>", [250, 251]);
        $cmd('DATA', 354);
        // dot-stuffing: a line starting with "." gets a second "."
        $message = preg_replace('/^\./m', '..', $message);
        $cmd($message . "\r\n.", 250);
        $cmd('QUIT', [221, 250]);
        fclose($fp);
        return true;
    } catch (Throwable $e) {
        error_log('[nikpak mail] ' . $e->getMessage());
        @fclose($fp);
        return false;
    }
}

/** JSON answer for fetch() calls. */
function nikpak_json(array $data, int $status = 200) {
    http_response_code($status);
    header('Content-Type: application/json; charset=utf-8');
    header('Cache-Control: no-store');
    echo json_encode($data, JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);
    exit;
}

function nikpak_wants_json(): bool
{
    return str_contains((string) ($_SERVER['HTTP_ACCEPT'] ?? ''), 'application/json')
        || str_contains((string) ($_SERVER['CONTENT_TYPE'] ?? ''), 'application/json');
}

/** Simple per-IP rate limit stored in the system temp folder. */
function nikpak_rate_limited(string $bucket, int $max, int $seconds): bool
{
    $ip = (string) ($_SERVER['REMOTE_ADDR'] ?? '0');
    $file = sys_get_temp_dir() . '/nikpak_rl_' . $bucket . '_' . hash('sha256', $ip . __DIR__) . '.json';
    $now = time();
    $hits = is_file($file) ? (json_decode((string) @file_get_contents($file), true) ?: []) : [];
    $hits = array_values(array_filter($hits, function ($t) use ($now, $seconds) { return is_int($t) && $t > $now - $seconds; }));
    if (count($hits) >= $max) return true;
    $hits[] = $now;
    @file_put_contents($file, json_encode($hits), LOCK_EX);
    return false;
}
