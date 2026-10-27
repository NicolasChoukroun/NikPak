<?php
/**
 * PayPal sends the buyer back here after approval (?order=NP...&token=<PayPal order id>).
 * The payment is captured, checked against the order, then the licence is issued and e-mailed.
 */
require __DIR__ . '/lib.php';

$o = shop_load_order((string) ($_GET['order'] ?? ''));
$lang = $o['lang'] ?? nikpak_pick_lang();
$ppId = preg_replace('/[^A-Z0-9]/', '', strtoupper((string) ($_GET['token'] ?? '')));
$go = function (string $status) use ($lang) {
    header('Location: ' . shop_thanks_url($lang, $status, 'paypal'), true, 303);
    exit;
};
if ($o === null || $ppId === '') $go('error');

try {
    $token = paypal_token();
    [$st, $j] = paypal_call('POST', '/v2/checkout/orders/' . $ppId . '/capture', [], $token);
    if ($st === 422) {   // already captured (page reloaded): read the order
        [$st, $j] = paypal_call('GET', '/v2/checkout/orders/' . $ppId, null, $token);
    }
    $unit = $j['purchase_units'][0] ?? [];
    $cap = $unit['payments']['captures'][0] ?? null;
    $custom = (string) ($cap['custom_id'] ?? $unit['custom_id'] ?? $unit['reference_id'] ?? '');
    if (!is_array($cap) || $custom !== $o['id']) {
        shop_log("paypal: capture failed for {$o['id']} ($ppId) status $st " . json_encode($j));
        $go('error');
    }
    $capStatus = (string) ($cap['status'] ?? '');
    $paid = (float) ($cap['amount']['value'] ?? 0);
    if ($capStatus === 'COMPLETED' && $paid + 0.001 >= $o['total'] && ($cap['amount']['currency_code'] ?? '') === 'USD') {
        shop_fulfil($o, 'paypal', (string) $cap['id'], $paid, 'USD');
        $go('ok');
    }
    shop_log("paypal: {$o['id']} capture status $capStatus amount $paid");
    $go($capStatus === 'PENDING' ? 'pending' : 'error');
} catch (Throwable $e) {
    shop_log('paypal exception: ' . $e->getMessage());
    $go('error');
}
