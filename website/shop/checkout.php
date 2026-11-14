<?php
/**
 * Starts a payment: POST JSON {method, type, seats, name, email, lang} from buy.html.
 * The price is computed HERE (never trusted from the browser). Answers {ok, url}: the provider's payment page.
 *   stripe     -> Stripe Checkout (cards, Apple Pay, Google Pay, Link)   -> stripe-webhook.php
 *   paypal     -> PayPal order                                          -> paypal-return.php
 *   paddle     -> Paddle Billing transaction (merchant of record, VAT)  -> paddle-webhook.php
 *   binancepay -> Binance Pay order (paid in crypto from a Binance account) -> binancepay-webhook.php
 */
require __DIR__ . '/lib.php';

if (($_SERVER['REQUEST_METHOD'] ?? '') !== 'POST') nikpak_json(['ok' => false, 'error' => 'method'], 405);
if (nikpak_rate_limited('checkout', 20, 600)) nikpak_json(['ok' => false, 'error' => 'rate'], 429);

$in = shop_read_json_body();
$method = (string) ($in['method'] ?? '');
try {
    $o = shop_order_from_input($in);
} catch (InvalidArgumentException $e) {
    nikpak_json(['ok' => false, 'error' => 'invalid'], 422);
}
if (empty(nikpak_site()['payments'][$method])) nikpak_json(['ok' => false, 'error' => 'method'], 422);

try {
    $cfg = shop_config();
    $o['method'] = $method;
    shop_save_order($o);
    $item = shop_item_name($o);
    $url = null;

    switch ($method) {
        case 'stripe':
            $locales = ['en' => 'en', 'zh' => 'zh', 'es' => 'es', 'fr' => 'fr', 'pt' => 'pt-BR', 'ru' => 'ru', 'id' => 'id'];
            [$st, $j] = stripe_call('POST', 'checkout/sessions', [
                'mode' => 'payment',
                'customer_email' => $o['email'],
                'client_reference_id' => $o['id'],
                'locale' => $locales[$o['lang']] ?? 'auto',
                'line_items' => [[
                    'quantity' => $o['seats'],
                    'price_data' => [
                        'currency' => 'usd',
                        'unit_amount' => $o['per_seat'] * 100,
                        'product_data' => ['name' => $o['type'] === 'company' ? 'NikPak company licence (per seat)' : 'NikPak personal licence',
                                           'description' => 'Licence holder: ' . $o['name']],
                    ],
                ]],
                'metadata' => stripe_metadata($o),
                'payment_intent_data' => ['metadata' => stripe_metadata($o), 'description' => $item],
                'success_url' => shop_thanks_url($o['lang'], 'ok', 'stripe'),
                'cancel_url' => shop_thanks_url($o['lang'], 'cancel', 'stripe'),
            ]);
            if ($st === 200) $url = $j['url'] ?? null;
            else shop_log("stripe checkout error $st " . json_encode($j['error'] ?? $j));
            break;

        case 'paypal':
            $total = number_format($o['total'], 2, '.', '');
            [$st, $j] = paypal_call('POST', '/v2/checkout/orders', [
                'intent' => 'CAPTURE',
                'purchase_units' => [[
                    'reference_id' => $o['id'],
                    'custom_id' => $o['id'],
                    'invoice_id' => $o['id'],
                    'description' => $item,
                    'amount' => ['currency_code' => 'USD', 'value' => $total,
                                 'breakdown' => ['item_total' => ['currency_code' => 'USD', 'value' => $total]]],
                    'items' => [[
                        'name' => $o['type'] === 'company' ? 'NikPak company licence (seat)' : 'NikPak personal licence',
                        'quantity' => (string) $o['seats'],
                        'unit_amount' => ['currency_code' => 'USD', 'value' => number_format($o['per_seat'], 2, '.', '')],
                        'category' => 'DIGITAL_GOODS',
                    ]],
                ]],
                'payment_source' => ['paypal' => ['experience_context' => [
                    'brand_name' => 'Nikkoid LLC - NikPak',
                    'shipping_preference' => 'NO_SHIPPING',
                    'user_action' => 'PAY_NOW',
                    'return_url' => shop_url('shop/paypal-return.php?order=' . $o['id']),
                    'cancel_url' => shop_thanks_url($o['lang'], 'cancel', 'paypal'),
                ]]],
            ]);
            if ($st === 200 || $st === 201) {
                foreach ($j['links'] ?? [] as $l) {
                    if (in_array($l['rel'] ?? '', ['payer-action', 'approve'], true)) $url = $l['href'];
                }
            } else shop_log("paypal order error $st " . json_encode($j));
            break;

        case 'paddle':
            [$st, $j] = paddle_call('POST', '/transactions', [
                'items' => [[
                    'quantity' => $o['seats'],
                    'price' => [
                        'description' => $o['type'] === 'company' ? 'NikPak company licence, per seat' : 'NikPak personal licence',
                        'name' => $o['type'] === 'company' ? 'Company licence (seat)' : 'Personal licence',
                        'unit_price' => ['amount' => (string) ($o['per_seat'] * 100), 'currency_code' => 'USD'],
                        'quantity' => ['minimum' => 1, 'maximum' => 100000],
                        'product' => ['name' => 'NikPak', 'tax_category' => 'standard'],
                    ],
                ]],
                'currency_code' => 'USD',
                'collection_mode' => 'automatic',
                'custom_data' => ['nikpak_order' => $o['id']],
            ]);
            if ($st === 200 || $st === 201) {
                $url = $j['data']['checkout']['url'] ?? null;    // your default payment link (pay.html) + ?_ptxn=...
                if ($url) $url .= (str_contains($url, '?') ? '&' : '?') . 'lang=' . $o['lang'] . '&email=' . rawurlencode($o['email']);
            } else shop_log("paddle transaction error $st " . json_encode($j));
            break;

        case 'binancepay':
            $b = $cfg['binancepay'];
            [$st, $j] = binancepay_call('/binancepay/openapi/v3/order', [
                'env' => ['terminalType' => 'WEB'],
                'merchantTradeNo' => $o['id'],
                'orderAmount' => (float) number_format($o['total'], 2, '.', ''),
                'currency' => (string) ($b['currency'] ?? 'USDT'),   // USD-pegged: 1 USDT per USD
                'description' => $item,
                'goodsDetails' => [[
                    'goodsType' => '02',            // virtual goods
                    'goodsCategory' => 'Z000',      // others
                    'referenceGoodsId' => 'nikpak-' . $o['type'],
                    'goodsName' => $item,
                    'goodsDetail' => 'Licence holder: ' . $o['name'],
                ]],
                'buyer' => ['buyerEmail' => $o['email']],
                'returnUrl' => shop_thanks_url($o['lang'], 'pending', 'binancepay'),
                'cancelUrl' => shop_thanks_url($o['lang'], 'cancel', 'binancepay'),
                'orderExpireTime' => (time() + 3600) * 1000,
                'webhookUrl' => shop_url('shop/binancepay-webhook.php'),
            ]);
            if ($st === 200 && ($j['status'] ?? '') === 'SUCCESS') $url = $j['data']['checkoutUrl'] ?? ($j['data']['universalUrl'] ?? null);
            else shop_log("binancepay order error $st " . json_encode($j));
            break;

        default:
            nikpak_json(['ok' => false, 'error' => 'method'], 422);
    }
} catch (Throwable $e) {
    shop_log("checkout $method exception: " . $e->getMessage());
    nikpak_json(['ok' => false, 'error' => 'server'], 500);
}

if (!$url) nikpak_json(['ok' => false, 'error' => 'provider'], 502);
nikpak_json(['ok' => true, 'url' => $url, 'order' => $o['id']]);
