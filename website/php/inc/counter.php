<?php
/**
 * Nikkoid web site - download counter. Shared by every product's site.
 *
 * **One JSON file, no database.** A counter is a number that goes up, and a file holds
 * it perfectly well; it also keeps counting if MySQL is down or not configured.
 *
 * The file lives in the private `data_dir` of the server's shop-config.php, OUTSIDE the
 * web root, beside the shop's own data:
 *
 *     'data_dir' => 'C:/inetpub/<product>/<product>-data',
 *
 * Without that setting it falls back to the system temp folder, which counts but does
 * not survive a server cleanup - so set data_dir.
 *
 * **Self-contained on purpose.** It does not include the site's own helper file: the
 * five sites each have a different one, and a counter that only works on the site it
 * was written for is a counter that gets copied wrong.
 */

/** Private settings, searched the way the shops already do. Cached. */
function nk_counter_config(): array
{
    static $cfg = null;
    if ($cfg !== null) {
        return $cfg;
    }
    $cfg = [];
    $root = dirname(__DIR__);   // the web root
    $candidates = array_filter([
        getenv('NIKKOID_SHOP_CONFIG') ?: null,
        dirname($root) . '/nikko-server/shop-config.php',
        dirname($root) . '/nikverb-server/shop-config.php',
        dirname($root) . '/shop-config.php',
        $root . '/shop/shop-config.php',
    ]);
    foreach ($candidates as $f) {
        if (is_file($f)) {
            $c = require $f;
            $cfg = is_array($c) ? $c : [];
            break;
        }
    }
    return $cfg;
}

/** Where the counts are kept. Created on first use. */
function nikverb_counter_file(): string
{
    static $path = null;
    if ($path !== null) {
        return $path;
    }
    $cfg = nk_counter_config();
    $dir = (isset($cfg['data_dir']) && $cfg['data_dir']) ? (string) $cfg['data_dir'] : sys_get_temp_dir();
    if (!is_dir($dir)) {
        @mkdir($dir, 0775, true);
    }
    $path = rtrim($dir, '/\\') . DIRECTORY_SEPARATOR . 'download-counts.json';
    return $path;
}

/**
 * The counts: ['total' => int, 'files' => ['name' => int], 'first' => 'Y-m-d'].
 * A missing or damaged file reads as zero - never as an error.
 */
function nikverb_counter_read(): array
{
    $empty = ['total' => 0, 'files' => [], 'first' => null];
    $raw = @file_get_contents(nikverb_counter_file());
    if ($raw === false || $raw === '') {
        return $empty;
    }
    $data = json_decode($raw, true);
    if (!is_array($data)) {
        return $empty;
    }
    return [
        'total' => (int) (isset($data['total']) ? $data['total'] : 0),
        'files' => (isset($data['files']) && is_array($data['files'])) ? $data['files'] : [],
        'first' => isset($data['first']) ? (string) $data['first'] : null,
    ];
}

/**
 * Count one download of $file. Never throws; a read-only folder means the counter
 * stands still, not that the visitor gets an error page.
 */
function nikverb_counter_record(string $file): void
{
    $file = substr(trim($file), 0, 160);
    if ($file === '') {
        return;
    }

    $fh = @fopen(nikverb_counter_file(), 'c+');
    if (!$fh) {
        return;
    }
    if (!flock($fh, LOCK_EX)) {
        fclose($fh);
        return;
    }

    // Re-read *inside* the lock: two downloads arriving together would otherwise both
    // read the old total and one of them would be lost.
    $raw = stream_get_contents($fh);
    $data = json_decode((string) $raw, true);
    if (!is_array($data)) {
        $data = [];
    }
    if (!isset($data['files']) || !is_array($data['files'])) {
        $data['files'] = [];
    }
    $data['files'][$file] = (int) (isset($data['files'][$file]) ? $data['files'][$file] : 0) + 1;
    $data['total'] = (int) (isset($data['total']) ? $data['total'] : 0) + 1;
    if (empty($data['first'])) {
        $data['first'] = gmdate('Y-m-d');
    }
    $data['last'] = gmdate('Y-m-d');

    ftruncate($fh, 0);
    rewind($fh);
    fwrite($fh, json_encode($data, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES));
    fflush($fh);
    flock($fh, LOCK_UN);
    fclose($fh);
}

/** ['total' => int, 'files' => [...]] for stats.php. Never throws. */
function nikverb_counter_totals(): array
{
    $d = nikverb_counter_read();
    return ['total' => $d['total'], 'files' => $d['files']];
}
