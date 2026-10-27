<?php
// https://nikpak.nikkoid.com/buy.php is the address opened by the "Buy a licence" button of NikPak
// (src/license.rs, BUY_URL). It sends the visitor to the shop page in their language.
// buy.php?seats=25 and buy.php?type=company are passed on to the page.
require __DIR__ . '/inc/common.php';
nikpak_redirect_page('buy');
