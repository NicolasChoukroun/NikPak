<?php
// NikPak licence tool - command line only (keep it outside the web root).
//   php tools/licenses.php issue "Jane Doe" jane@example.com                 personal licence
//   php tools/licenses.php issue "ACME Corp" it@acme.example company 25 "PO 1234"
//   php tools/licenses.php find jane@example.com                             resend a key
//   php tools/licenses.php list [n]                                          last n licences
//   php tools/licenses.php email ID                                          e-mail text of a licence
//   php tools/licenses.php keyfile ID > nikpak.key                           licence file for companies
if (PHP_SAPI !== 'cli') { http_response_code(404); exit; }
require_once __DIR__ . '/../license_store.php';

$cmd = isset($argv[1]) ? $argv[1] : '';
$a = array_slice($argv, 2);
try {
    $pdo = nikpak_db();
    switch ($cmd) {
        case 'issue':
            if (count($a) < 2) throw new InvalidArgumentException('issue "Name" email [personal|company] [seats] [note]');
            $lic = nikpak_issue($pdo, $a[0], $a[1], isset($a[2]) ? $a[2] : 'personal', isset($a[3]) ? (int)$a[3] : 1,
                                array('provider' => 'manual', 'note' => isset($a[4]) ? $a[4] : null));
            echo nikpak_license_email($lic);
            break;
        case 'find':
            foreach (nikpak_find_by_email($pdo, $a[0]) as $l) {
                echo "#{$l['id']}  {$l['created']}  {$l['name']}  {$l['type']} x{$l['seats']}\n  {$l['license_key']}\n";
            }
            break;
        case 'list':
            $n = isset($a[0]) ? max(1, (int)$a[0]) : 20;
            foreach ($pdo->query("SELECT id, created, name, email, type, seats, provider FROM nikpak_licenses ORDER BY id DESC LIMIT $n") as $l) {
                echo "#{$l['id']}  {$l['created']}  {$l['name']} <{$l['email']}>  {$l['type']} x{$l['seats']}  {$l['provider']}\n";
            }
            break;
        case 'email':
        case 'keyfile':
            $st = $pdo->prepare('SELECT * FROM nikpak_licenses WHERE id = ?');
            $st->execute(array((int)$a[0]));
            $l = $st->fetch();
            if (!$l) throw new RuntimeException('no such licence');
            echo $cmd === 'email' ? nikpak_license_email($l) : nikpak_license_file($l['name'], $l['email'], $l['license_key']);
            break;
        default:
            fwrite(STDERR, "usage: php tools/licenses.php issue|find|list|email|keyfile ...\n");
            exit(2);
    }
} catch (Exception $e) {
    fwrite(STDERR, 'ERROR: ' . $e->getMessage() . "\n");
    exit(1);
}
