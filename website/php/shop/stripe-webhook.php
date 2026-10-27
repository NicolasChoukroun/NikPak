<?php
/**
 * Stripe webhook. In the Stripe dashboard add the endpoint https://nikpak.nikkoid.com/shop/stripe-webhook.php
 * with the events checkout.session.completed, checkout.session.async_payment_succeeded and payment_intent.succeeded,
 * and copy its signing secret (whsec_...) into shop-config.php.
 */
require __DIR__ . '/lib.php';

$payload = (string) file_get_contents('php://input');
try {
    $secret = (string) shop_config()['stripe']['webhook_secret'];
} catch (Throwable $e) {
    http_response_code(500);
    exit;
}
if (!stripe_verify($payload, (string) ($_SERVER['HTTP_STRIPE_SIGNATURE'] ?? ''), $secret)) {
    http_response_code(400);
    exit('bad signature');
}
$event = json_decode($payload, true) ?: [];
$type = (string) ($event['type'] ?? '');
$obj = $event['data']['object'] ?? [];

try {
    $paidSession = in_array($type, ['checkout.session.completed', 'checkout.session.async_payment_succeeded'], true)
        && ($obj['payment_status'] ?? '') === 'paid';
    // Google Pay payments (gpay.php) that were still processing
    $paidIntent = $type === 'payment_intent.succeeded' && ($obj['metadata']['source'] ?? '') === 'googlepay';

    if ($paidSession || $paidIntent) {
        $orderId = (string) ($obj['metadata']['nikpak_order'] ?? '');
        $o = shop_load_order($orderId);
        if ($o === null) {
            shop_log("stripe: unknown order $orderId");
            http_response_code(200);
            exit('unknown order');
        }
        $paid = ((int) ($paidSession ? $obj['amount_total'] : $obj['amount_received'])) / 100;
        if ($paid + 0.001 < $o['total']) {
            shop_log("stripe: amount $paid lower than order {$o['total']} ($orderId)");
            http_response_code(200);
            exit('amount');
        }
        $ref = $paidSession ? (string) ($obj['payment_intent'] ?? $obj['id']) : (string) $obj['id'];
        shop_fulfil($o, $paidIntent ? 'googlepay' : 'stripe', $ref, $paid, strtoupper((string) ($obj['currency'] ?? 'usd')));
    }
    http_response_code(200);
    echo 'ok';
} catch (Throwable $e) {
    shop_log('stripe webhook exception: ' . $e->getMessage());
    http_response_code(500);   // Stripe retries later
}
