<?php
/**
 * NikPak licences: create + store + look up (PHP 7.0+, MySQL 5.7 via PDO).
 *
 *   $pdo = nikpak_db();
 *   $lic = nikpak_issue($pdo, 'Jane Doe', 'jane@example.com', 'personal', 1,
 *                       array('payment_ref' => $txnId, 'provider' => 'paypal', 'amount' => 10, 'currency' => 'USD'));
 *   echo $lic['license_key'];
 *
 * Calling nikpak_issue() twice with the same payment_ref returns the licence
 * created the first time (payment webhooks are often delivered more than once).
 */
require_once __DIR__ . '/nikpak_license.php';

function nikpak_config(): array {
    static $cfg = null;
    if ($cfg === null) {
        $file = getenv('NIKPAK_CONFIG') ?: __DIR__ . '/config.php';
        if (!is_file($file)) throw new RuntimeException("missing $file (copy config.sample.php)");
        $cfg = require $file;
        if (!empty($cfg['private_key']) && !getenv('NIKPAK_PRIVATE_KEY')) putenv('NIKPAK_PRIVATE_KEY=' . $cfg['private_key']);
    }
    return $cfg;
}

function nikpak_db(): PDO {
    $c = nikpak_config();
    return new PDO($c['db_dsn'], $c['db_user'], $c['db_pass'], array(
        PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION,
        PDO::ATTR_DEFAULT_FETCH_MODE => PDO::FETCH_ASSOC,
    ));
}

/** @return array the stored licence row */
function nikpak_issue(PDO $pdo, string $name, string $email, string $type = 'personal', int $seats = 1, array $extra = array()): array {
    $type = $type === 'company' ? 'company' : 'personal';
    if ($type === 'personal') $seats = 1;
    $ref = isset($extra['payment_ref']) && $extra['payment_ref'] !== '' ? (string)$extra['payment_ref'] : null;
    if ($ref !== null) {
        $st = $pdo->prepare('SELECT * FROM nikpak_licenses WHERE payment_ref = ?');
        $st->execute(array($ref));
        $row = $st->fetch();
        if ($row) return $row;   // already issued for this payment
    }
    $key = nikpak_license_key($name, $email, $type, $seats);
    $st = $pdo->prepare('INSERT INTO nikpak_licenses (license_key, name, email, type, seats, amount, currency, provider, payment_ref, note)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)');
    $st->execute(array(
        $key, trim($name), strtolower(trim($email)), $type, $seats,
        isset($extra['amount']) ? $extra['amount'] : null,
        isset($extra['currency']) ? $extra['currency'] : null,
        isset($extra['provider']) ? $extra['provider'] : null,
        $ref,
        isset($extra['note']) ? $extra['note'] : null,
    ));
    $st = $pdo->prepare('SELECT * FROM nikpak_licenses WHERE id = ?');
    $st->execute(array($pdo->lastInsertId()));
    return $st->fetch();
}

/** All licences bought with this e-mail address (for a "resend my key" page). */
function nikpak_find_by_email(PDO $pdo, string $email): array {
    $st = $pdo->prepare('SELECT * FROM nikpak_licenses WHERE email = ? ORDER BY id');
    $st->execute(array(strtolower(trim($email))));
    return $st->fetchAll();
}

/** Text of the e-mail sent to the customer. */
function nikpak_license_email(array $lic): string {
    $t  = "Thank you for buying NikPak!\n\n";
    $t .= "Name:    {$lic['name']}\nE-mail:  {$lic['email']}\nLicence: ";
    $t .= $lic['type'] === 'company' ? "company licence, {$lic['seats']} seat(s)\n" : "personal licence\n";
    $t .= "Key:     {$lic['license_key']}\n\n";
    $t .= "To register: open NikPak, click \"Register\" (or \"Evaluation copy - Register\" at the top left),\n";
    $t .= "enter exactly this name and e-mail address, and paste the key.\n";
    $t .= "Command line:  nikpak register \"{$lic['name']}\" {$lic['email']} {$lic['license_key']}\n\n";
    $t .= $lic['type'] === 'company'
        ? "Companies: you may also put the attached nikpak.key next to nikpak.exe on your computers.\n"
        : "Your personal licence is valid on all your own computers.\n";
    $t .= "\nKeep this e-mail: it is your proof of purchase.\nhttps://nikpak.nikkoid.com\n";
    return $t;
}
