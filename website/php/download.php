<?php
/**
 * Nikkoid web site - counted download link.
 *
 * Every download points here:  download.php?f=<file name>
 * The request is counted, then answered with a 302 to the real file, so the browser
 * (and any download manager) gets an ordinary URL and the counter cannot slow it down.
 *
 * Only files that really exist in downloads/ or documents/ are served: the name is
 * reduced to its basename first, so ?f=../../shop-config.php cannot escape.
 */

require __DIR__ . '/inc/counter.php';

$name = isset($_GET['f']) ? (string) $_GET['f'] : '';
$name = basename(str_replace('\\', '/', $name));

$target = null;
if ($name !== '' && $name !== '.' && $name !== '..') {
    foreach (['downloads', 'documents'] as $dir) {
        if (is_file(__DIR__ . '/' . $dir . '/' . $name)) {
            $target = $dir . '/' . $name;
            break;
        }
    }
}

if ($target === null) {
    http_response_code(404);
    header('Content-Type: text/html; charset=utf-8');
    echo '<!doctype html><meta charset="utf-8"><title>Not found</title>' .
         '<p>That download does not exist. <a href="downloads.html">Every file is listed here</a>.</p>';
    exit;
}

// Count real downloads only. A HEAD (a link checker, a download manager asking for the
// size first) is answered but not counted, or every file would count twice.
$method = isset($_SERVER['REQUEST_METHOD']) ? $_SERVER['REQUEST_METHOD'] : 'GET';
$prefetch = !empty($_SERVER['HTTP_X_MOZ']) && strtolower((string) $_SERVER['HTTP_X_MOZ']) === 'prefetch';
if ($method === 'GET' && !$prefetch) {
    if (function_exists('ignore_user_abort')) {
        ignore_user_abort(true);   // finish counting even if the browser disconnects
    }
    nikverb_counter_record($name);
}

header('Cache-Control: no-store, no-cache, must-revalidate');
header('Referrer-Policy: strict-origin-when-cross-origin');
header('Location: ' . $target, true, 302);
