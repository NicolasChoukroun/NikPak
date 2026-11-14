<?php
/**
 * Nikkoid web site - the download total, as JSON.
 *
 * Read by assets/nk-downloads.js to fill the number in the footer of every page. Kept
 * out of the pages themselves so the HTML stays static and cacheable, and cached for a
 * minute here so a busy day does not turn page views into file reads.
 *
 *   {"total": 1234, "files": {"NikVerb-0.1.25-Linux.zip": 210, ...}}
 */

require __DIR__ . '/inc/counter.php';

header('Content-Type: application/json; charset=utf-8');
header('Cache-Control: public, max-age=60');
header('X-Content-Type-Options: nosniff');

echo json_encode(nikverb_counter_totals(), JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
