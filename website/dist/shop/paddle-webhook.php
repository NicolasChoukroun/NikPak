<?php
/**
 * Paddle Billing webhook (notification destination https://nikpak.nikkoid.com/shop/paddle-webhook.php,
 * event transaction.completed). Paddle is the merchant of record: it charges the buyer's taxes and pays you the net amount.
 */
require __DIR__ . '/lib.php';

$payload = (string) file_get_contents('php://input');
try {
    $secret = (string) shop_config()['paddle']['webhook_secret'];
} catch (Throwable $e) {
    http_response_code(500);
    exit;
}
if (!paddle_verify($payload, (string) ($_SERVER['HTTP_PADDLE_SIGNATURE'] ?? ''), $secret)) {
    http_response_code(400);
    exit('bad signature');
}
$event = json_decode($payload, true) ?: [];
try {
    if (($event['event_type'] ?? '') === 'transaction.completed') {
        $d = $event['data'] ?? [];
        $o = shop_load_order((string) ($d['custom_data']['nikpak_order'] ?? ''));
        if ($o === null) {
            shop_log('paddle: unknown order ' . json_encode($d['custom_data'] ?? null));
            http_response_code(200);
            exit('unknown order');
        }
        // subtotal = price before tax, in cents
        $subtotal = ((int) ($d['details']['totals']['subtotal'] ?? 0)) / 100;
        if ($subtotal + 0.001 < $o['total']) {
            shop_log("paddle: subtotal $subtotal lower than order {$o['total']} ({$o['id']})");
            http_response_code(200);
            exit('amount');
        }
        $grand = ((int) ($d['details']['totals']['grand_total'] ?? 0)) / 100;
        shop_fulfil($o, 'paddle', (string) $d['id'], $grand, (string) ($d['currency_code'] ?? 'USD'));
    }
    http_response_code(200);
    echo 'ok';
} catch (Throwable $e) {
    shop_log('paddle webhook exception: ' . $e->getMessage());
    http_response_code(500);
}
