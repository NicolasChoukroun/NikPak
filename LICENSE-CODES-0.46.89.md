# HexaWar — giving license codes to friends or buyers (0.46.89)

## 1. One-time server setup
1. Upload hexawar-api-0.46.89.zip over the existing API (keep your config.php).
2. Run the 0.46.89 part at the end of `upgrade.sql` (tables `hw_licenses`, `hw_lic_dev`, view `v_licenses`) in phpMyAdmin / MySQL.
3. Put the `tools` folder next to `api` (outside the web root if possible). It only runs from a command line.

## 2. Create a code to e-mail to a friend
    php tools/licenses.php new "Bob (friend)"          # 1 code, 3 devices
    php tools/licenses.php new "Giveaway" 2 50         # 50 codes, 2 devices each
Prints e.g. `K7M2-9QXD-4TRA-WB3N`. Paste it in the e-mail with the app download.

No command line? Use SQL (code = 16 letters/digits, no dashes):
    INSERT INTO hw_licenses(code,note,max_dev,created) VALUES('K7M29QXD4TRAWB3N','Bob',3,UNIX_TIMESTAMP());

## 3. Manage
    php tools/licenses.php list                 # codes, devices used, last seen
    php tools/licenses.php revoke  CODE         # stops working at next check
    php tools/licenses.php unrevoke CODE
    php tools/licenses.php reset   CODE         # frees its devices (friend changed computer)

## 4. Build the app that asks for a code
Add `HexaWarLic` after the token in the Pake user agent:

    pake https://hexawar.nikkoid.com --name HexaWar --user-agent "HexaWarApp/9a3faae8d49d210a2763ae7716c64b9c HexaWarLic"

Builds WITHOUT `HexaWarLic` (Steam / itch.io) never ask for a code — the store is the license.

## 5. Behaviour
- First launch: the player pastes the code once; it is remembered on that device.
- Each launch re-checks online; if the server is unreachable the game still works for 30 days.
- A revoked code is refused at the next online check.
- Default limit: 3 devices per code (set per code).
