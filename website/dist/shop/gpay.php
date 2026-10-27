<?php
/**
 * Google Pay: buy.html sends the Google Pay token (tokenised for Stripe) with the order.
 * The amount is computed here, the card is charged through Stripe, then the licence is issued and e-mailed.
 * POST JSON {type, seats, name, email, lang, token} -> {ok, url}
 */
require __DIR__ . '/lib.php';

if (($_SERVER['REQUEST_METHOD'] ?? '') !== 'POST') nikpak_json(['ok' => false, 'error' => 'method'], 405);
if (nikpak_rate_limited('gpay', 10, 600)) nikpak_json(['ok' => false, 'error' => 'rate'], 429);

$in = shop_read_json_body();
try {
    $o = shop_order_from_input($in);
} catch (InvalidArgumentException $e) {
    nikpak_json(['ok' => false, 'error' => 'invalid'], 422);
}
// With the "stripe" gateway, Google Pay returns a Stripe token as a JSON string: {"id":"tok_...", ...}
$tok = json_decode((string) ($in['token'] ?? ''), true);
$tokenId = is_array($tok) ? (string) ($tok['id'] ?? '') : '';
if (!preg_match('/^tok_[A-Za-z0-9_]+$/', $tokenId)) nikpak_json(['ok' => false, 'error' => 'token'], 422);

try {
    $o['method'] = 'googlepay';
    shop_save_order($o);
    [$st, $pi] = stripe_call('POST', 'payment_intents', [
        'amount' => $o['total'] * 100,
        'currency' => 'usd',
        'payment_method_types' => ['card'],
        'payment_method_data' => ['type' => 'card', 'card' => ['token' => $tokenId]],
        'confirm' => 'true',
        'description' => shop_item_name($o),
        'receipt_email' => $o['email'],
        'metadata' => stripe_metadata($o) + ['source' => 'googlepay'],
    ]);
    $status = (string) ($pi['status'] ?? '');
    if ($st === 200 && $status === 'succeeded') {
        shop_fulfil($o, 'googlepay', (string) $pi['id'], $o['total'], 'USD');
        nikpak_json(['ok' => true, 'url' => shop_thanks_url($o['lang'], 'ok', 'googlepay')]);
    }
    if ($st === 200 && $status === 'processing') {
        // stripe-webhook.php issues the licence when payment_intent.succeeded arrives
        nikpak_json(['ok' => true, 'url' => shop_thanks_url($o['lang'], 'pending', 'googlepay')]);
    }
    shop_log("gpay {$o['id']} failed: $st $status " . json_encode($pi['error'] ?? []));
    nikpak_json(['ok' => false, 'error' => 'declined'], 402);
} catch (Throwable $e) {
    shop_log('gpay exception: ' . $e->getMessage());
    nikpak_json(['ok' => false, 'error' => 'server'], 500);
}
