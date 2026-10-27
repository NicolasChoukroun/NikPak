<?php
/**
 * Binance Pay webhook (the order's webhookUrl, also settable in the Binance Merchant Admin Portal).
 * Binance signs the body with its RSA key; the public key is fetched from the certificates API and cached.
 * Binance expects {"returnCode":"SUCCESS","returnMessage":null}, otherwise it retries.
 */
require __DIR__ . '/lib.php';

function bp_answer(bool $ok) {
    header('Content-Type: application/json');
    http_response_code($ok ? 200 : 400);
    echo json_encode(['returnCode' => $ok ? 'SUCCESS' : 'FAIL', 'returnMessage' => null]);
    exit;
}

$payload = (string) file_get_contents('php://input');
$verified = false;
try {
    $verified = binancepay_verify(
        $payload,
        (string) ($_SERVER['HTTP_BINANCEPAY_TIMESTAMP'] ?? ''),
        (string) ($_SERVER['HTTP_BINANCEPAY_NONCE'] ?? ''),
        (string) ($_SERVER['HTTP_BINANCEPAY_SIGNATURE'] ?? '')
    );
} catch (Throwable $e) {
    shop_log('binancepay verify exception: ' . $e->getMessage());
}
if (!$verified) {
    shop_log('binancepay: bad signature');
    bp_answer(false);
}

$event = json_decode($payload, true) ?: [];
try {
    if (($event['bizType'] ?? '') === 'PAY' && ($event['bizStatus'] ?? '') === 'PAY_SUCCESS') {
        $d = is_string($event['data'] ?? null) ? (json_decode($event['data'], true) ?: []) : ($event['data'] ?? []);
        $o = shop_load_order((string) ($d['merchantTradeNo'] ?? ''));
        if ($o === null) {
            shop_log('binancepay: unknown order ' . ($d['merchantTradeNo'] ?? '?'));
            bp_answer(true);   // nothing to retry
        }
        $paid = (float) ($d['totalFee'] ?? $d['orderAmount'] ?? 0);
        $cur = (string) ($d['currency'] ?? 'USDT');
        if ($paid + 0.001 < $o['total']) {
            shop_log("binancepay: amount $paid $cur lower than order {$o['total']} ({$o['id']})");
            bp_answer(true);
        }
        shop_fulfil($o, 'binancepay', (string) ($event['bizIdStr'] ?? $event['bizId'] ?? $o['id']), $paid, $cur);
    } elseif (($event['bizStatus'] ?? '') !== '') {
        shop_log('binancepay: ' . $event['bizStatus'] . ' ' . ($event['bizIdStr'] ?? ''));
    }
    bp_answer(true);
} catch (Throwable $e) {
    shop_log('binancepay webhook exception: ' . $e->getMessage());
    http_response_code(500);
    echo json_encode(['returnCode' => 'FAIL', 'returnMessage' => 'retry']);
}
