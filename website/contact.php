<?php
/**
 * NikPak contact form (xx/contact.html posts here). PHP 7.2+ (also PHP 8.x).
 * Every e-mail sent has "NikPak" at the start of its subject, so that it can be filtered.
 * Answers JSON to fetch(), or redirects back to the page when JavaScript is off.
 */
require __DIR__ . '/inc/common.php';

const TOPICS = [
    'general' => 'General question',
    'sales'   => 'Licences and sales',
    'crypto'  => 'Crypto payment',
    'support' => 'Technical support or bug report',
    'press'   => 'Press and partnerships',
];

/** Must stay identical to subject() in assets/js/contact.js */
function nikpak_contact_subject(string $topic, string $subject, string $coin, string $txid): string
{
    $out = 'NikPak - ' . (TOPICS[$topic] ?? TOPICS['general']);
    if ($topic === 'crypto') {
        $out .= ' ' . $coin . ($txid !== '' ? ' ' . mb_substr($txid, 0, 14) . (mb_strlen($txid) > 14 ? '…' : '') : '');
    }
    if ($subject !== '') $out .= ' - ' . $subject;
    return mb_substr($out, 0, 180);
}

function done(bool $ok, string $lang, string $error = '') {
    if (nikpak_wants_json()) nikpak_json(['ok' => $ok, 'error' => $error], $ok ? 200 : ($error === 'invalid' ? 422 : 500));
    header('Location: ' . $lang . '/contact.html?sent=' . ($ok ? '1' : '0'), true, 303);
    exit;
}

if (($_SERVER['REQUEST_METHOD'] ?? '') !== 'POST') {
    nikpak_redirect_page('contact');
}

$lang = nikpak_lang_or_null($_POST['lang'] ?? null) ?? nikpak_pick_lang();

// spam traps: hidden field filled, or form sent faster than a human can type
$ts = (int) ($_POST['ts'] ?? 0);
if (trim((string) ($_POST['website'] ?? '')) !== '' || ($ts > 0 && time() - $ts < 3)) {
    done(true, $lang); // pretend success to bots
}
if (nikpak_rate_limited('contact', 6, 900)) done(false, $lang, 'rate');

$name    = nikpak_clean_line($_POST['name'] ?? '', 120);
$email   = nikpak_clean_line($_POST['email'] ?? '', 190);
$topic   = array_key_exists((string) ($_POST['topic'] ?? ''), TOPICS) ? (string) $_POST['topic'] : 'general';
$subject = nikpak_clean_line($_POST['subject'] ?? '', 150);
$message = nikpak_clean_text($_POST['message'] ?? '', 8000);

$crypto = [];
$coin = $txid = '';
if ($topic === 'crypto') {
    $coins = nikpak_site()['crypto'] ?? [];
    $coin = array_key_exists((string) ($_POST['coin'] ?? ''), $coins) ? (string) $_POST['coin'] : nikpak_clean_line($_POST['coin'] ?? '', 20);
    $txid = nikpak_clean_line($_POST['txid'] ?? '', 200);
    $licence = ($_POST['licence'] ?? '') === 'company' ? 'company' : 'personal';
    $seats = $licence === 'company' ? max(1, min(100000, (int) ($_POST['seats'] ?? 1))) : 1;
    $network = $coins[$coin]['network'] ?? '';
    $crypto = [
        'Coin'           => $coin . ($network !== '' ? ' (' . $network . ')' : ''),
        'Amount sent'    => nikpak_clean_line($_POST['amount'] ?? '', 60),
        'Transaction ID' => $txid,
        'Licence'        => $licence . ($licence === 'company' ? ", $seats seat(s)" : ''),
        'Licence holder' => nikpak_clean_line($_POST['holder'] ?? '', 120) ?: $name,
        'Key e-mail'     => $email,
    ];
    if (isset($coins[$coin]['address'])) $crypto['Paid to'] = $coins[$coin]['address'];
}

$valid = mb_strlen($name) >= 2 && nikpak_valid_email($email)
    && ($topic === 'crypto' ? mb_strlen($txid) >= 6 : (mb_strlen($message) >= 3 && $subject !== ''));
if (!$valid) done(false, $lang, 'invalid');

$fullSubject = nikpak_contact_subject($topic, $subject, $coin, $txid);

$body  = "New message from the NikPak web site\n";
$body .= str_repeat('=', 40) . "\n";
$body .= "Name:     $name\nE-mail:   $email\nTopic:    " . TOPICS[$topic] . "\nLanguage: $lang\n";
if ($crypto) {
    $body .= "\nCRYPTO PAYMENT - check the transaction before issuing the key:\n";
    foreach ($crypto as $k => $v) $body .= str_pad($k . ':', 16) . $v . "\n";
    $holderArg = str_replace('"', "'", $crypto['Licence holder']);
    $body .= "\nIssue the key with the licence manager (niklicense nikpak issue \"Name\" email type seats):\n"
        . "  niklicense nikpak issue \"$holderArg\" $email $licence $seats --ref \"crypto $coin $txid\" --provider crypto\n"
        . "Then e-mail the printed text to the customer, with NikPak in the subject.\n";
}
if ($message !== '') $body .= "\nMessage:\n" . $message . "\n";
$body .= "\n--\nIP " . ($_SERVER['REMOTE_ADDR'] ?? '?') . ' · ' . gmdate('Y-m-d H:i') . " UTC\n";

$to = (string) nikpak_site()['contact_email'];
$ok = nikpak_mail($to, $fullSubject, $body, $email);

// short acknowledgement to the sender (also with NikPak in the subject)
if ($ok) {
    $ack = "Hello $name,\n\nWe have received your message \"$fullSubject\" and will answer as soon as possible.\n";
    if ($crypto) $ack .= "Your licence key will be sent to this address once the transaction is confirmed.\n";
    $ack .= "\nWhen you write to us again, please keep NikPak in the subject.\n\nNikkoid LLC - https://nikpak.nikkoid.com\n";
    @nikpak_mail($email, 'NikPak - we received your message', $ack, $to);
}
done($ok, $lang, $ok ? '' : 'mail');
