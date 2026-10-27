-- NikPak licences - MySQL 5.7 (also MySQL 8 / MariaDB). Run once in phpMyAdmin.
CREATE TABLE IF NOT EXISTS nikpak_licenses (
  id            INT UNSIGNED      NOT NULL AUTO_INCREMENT,
  license_key   VARCHAR(140)      NOT NULL,
  name          VARCHAR(190)      NOT NULL,
  email         VARCHAR(190)      NOT NULL,
  type          ENUM('personal','company') NOT NULL DEFAULT 'personal',
  seats         SMALLINT UNSIGNED NOT NULL DEFAULT 1,
  amount        DECIMAL(10,2)     NULL,
  currency      CHAR(3)           NULL,
  provider      VARCHAR(40)       NULL,      -- e.g. paypal, stripe, paddle, manual
  payment_ref   VARCHAR(190)      NULL,      -- the payment provider's transaction id
  note          VARCHAR(255)      NULL,
  created       DATETIME          NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uk_payment_ref (payment_ref),   -- a payment can never create two licences
  KEY k_email (email)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
