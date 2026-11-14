<?php
// https://nikpak.nikkoid.com/ -> the home page. It is one page for every language now:
// the visitor's language is chosen in the browser (assets/js/site.js), so there is no
// en/index.html to redirect to.
header('Content-Type: text/html; charset=utf-8');
readfile(__DIR__ . '/index.html');
