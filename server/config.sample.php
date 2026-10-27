<?php
// Copy to config.php (outside the web root if possible) and fill in.
return array(
    'db_dsn'  => 'mysql:host=localhost;dbname=nikpak;charset=utf8mb4',
    'db_user' => 'nikpak',
    'db_pass' => 'change-me',
    // Licence private key (SECRET). Default: ../keys/nikpak-private.key
    'private_key' => __DIR__ . '/../keys/nikpak-private.key',
);
