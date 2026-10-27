<?php
/**
 * NikPak licence keys - server side. PHP 7.0 or newer.
 *
 * Needs the sodium functions: built into PHP 7.2+ (ext-sodium). On PHP 7.0/7.1
 * install the pure-PHP polyfill paragonie/sodium_compat and require its autoloader
 * before this file.
 *
 * Creates the licence keys that nikpak accepts. Call nikpak_license_key()
 * from the page that handles a confirmed payment (buy.php / payment webhook),
 * then send the key to the customer.
 *
 *   require 'nikpak_license.php';
 *   $key = nikpak_license_key('Jane Doe', 'jane@example.com', 'personal', 1);
 *   $key = nikpak_license_key('ACME Corp', 'it@acme.example', 'company', 25);
 *
 * The private key file must be kept SECRET and outside the web root
 * (whoever has it can create licence keys). Its path is set below or in the
 * NIKPAK_PRIVATE_KEY environment variable.
 *
 * Format (must stay identical to src/license.rs):
 *   message = "NIKPAK-LICENSE-1\n" . chr(type) . pack('n', seats)
 *             . normalize(name) . "\n" . normalize(email)
 *   key     = "NPK1-" . crockford_base32(chr(type) . pack('n', seats) . ed25519_signature)
 *             in groups of 5 characters
 */

const NIKPAK_PRIVATE_KEY_FILE = __DIR__ . '/../keys/nikpak-private.key';

function nikpak_normalize(string $s): string {
    // ASCII whitespace runs -> one space, trimmed, ASCII letters lower-cased
    $s = trim(preg_replace('/[ \t\n\r\x0B\x0C]+/', ' ', $s), " \t\n\r\x0B\x0C");
    return strtr($s, 'ABCDEFGHIJKLMNOPQRSTUVWXYZ', 'abcdefghijklmnopqrstuvwxyz');
}

function nikpak_base32(string $data): string {
    $alphabet = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
    $out = ''; $acc = 0; $bits = 0;
    for ($i = 0; $i < strlen($data); $i++) {
        $acc = (($acc << 8) | ord($data[$i])) & 0xFFFF;
        $bits += 8;
        while ($bits >= 5) {
            $bits -= 5;
            $out .= $alphabet[($acc >> $bits) & 31];
        }
    }
    if ($bits > 0) {
        $out .= $alphabet[($acc << (5 - $bits)) & 31];
    }
    return $out;
}

function nikpak_private_seed(): string {
    $file = getenv('NIKPAK_PRIVATE_KEY') ?: NIKPAK_PRIVATE_KEY_FILE;
    $hex = '';
    foreach (file($file, FILE_IGNORE_NEW_LINES) as $line) {
        if (strlen(trim($line)) && trim($line)[0] !== '#') $hex .= preg_replace('/\s+/', '', $line);
    }
    if (strlen($hex) !== 64 || !ctype_xdigit($hex)) throw new RuntimeException('invalid private key file');
    return hex2bin($hex);
}

/**
 * @param string $type  'personal' ($19, one person, any number of their computers)
 *                      or 'company' (per seat, see nikpak_price())
 * @param int    $seats number of seats (1 for personal)
 */
function nikpak_license_key(string $name, string $email, string $type = 'personal', int $seats = 1): string {
    $t = $type === 'company' ? 2 : 1;
    if ($seats < 1 || $seats > 65535) throw new InvalidArgumentException('seats must be 1..65535');
    if (nikpak_normalize($name) === '' || nikpak_normalize($email) === '') throw new InvalidArgumentException('name and email are required');
    $msg = "NIKPAK-LICENSE-1\n" . chr($t) . pack('n', $seats) . nikpak_normalize($name) . "\n" . nikpak_normalize($email);
    if (!function_exists('sodium_crypto_sign_detached')) {
        throw new RuntimeException('PHP sodium functions are missing: use PHP 7.2+ with ext-sodium, or paragonie/sodium_compat');
    }
    $kp = sodium_crypto_sign_seed_keypair(nikpak_private_seed());
    $sig = sodium_crypto_sign_detached($msg, sodium_crypto_sign_secretkey($kp));
    $b32 = nikpak_base32(chr($t) . pack('n', $seats) . $sig);
    return 'NPK1-' . implode('-', str_split($b32, 5));
}

/** Content of a nikpak.key file (optional e-mail attachment; companies can put it next to nikpak.exe). */
function nikpak_license_file(string $name, string $email, string $key): string {
    return "NikPak licence\r\nname=" . trim($name) . "\r\nemail=" . trim($email) . "\r\nkey=" . $key . "\r\n";
}

/**
 * Price in USD (excluding VAT) - volume prices modelled on WinRAR's.
 *   personal: 19
 *   company, per seat: 1-9 seats 19, 10-24 15, 25-49 12, 50-99 9,
 *                      100-199 7, 200-499 6, 500+ 5
 * @return array ['per_seat' => int, 'total' => int]
 */
function nikpak_price(string $type, int $seats = 1): array {
    if ($type !== 'company') return array('per_seat' => 19, 'total' => 19);
    if ($seats < 1) throw new InvalidArgumentException('seats must be at least 1');
    $tiers = array(500 => 5, 200 => 6, 100 => 7, 50 => 9, 25 => 12, 10 => 15, 1 => 19);
    foreach ($tiers as $min => $usd) {
        if ($seats >= $min) return array('per_seat' => $usd, 'total' => $usd * $seats);
    }
}
