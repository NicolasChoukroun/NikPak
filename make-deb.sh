#!/usr/bin/env sh
# make-deb.sh - build the two Debian packages (nikpak, unnik) into binaries/.
# Called by package-unix.sh on Linux (the GitHub Actions "linux" leg) and usable
# by hand. Needs dpkg-deb (any Debian/Ubuntu system).
#
#   ./make-deb.sh                  # binaries from target/release
#   ./make-deb.sh /path/to/bin     # binaries (nikpak, unNik) from a folder
#
# The `nikpak` package installs /usr/bin/nikpak and /usr/bin/unNik; the `unnik`
# package installs /usr/bin/unNik alone - mirroring the two Windows setups.
# Both install the system-wide .nik MIME type and an unNik desktop entry, so a
# .nik file opens in unNik. The per-user right-click submenu (Dolphin / Nemo /
# Nautilus) is added by each user with `nikpak --install-menu`; it is per-user
# by design (like the tar.gz install.sh), so a system package cannot add it for
# everyone from its postinst, which runs as root.
set -e
cd "$(dirname "$0")"
T="${CARGO_TARGET_DIR:-target}"
V=$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -1)
ARCH=amd64
BIN="${1:-$T/release}"
DEPENDS="${NIKPAK_DEB_DEPENDS:-libc6}"

if ! command -v dpkg-deb >/dev/null 2>&1; then
  echo "make-deb.sh: dpkg-deb not found (not Debian/Ubuntu); skipping the .deb packages." >&2
  exit 0
fi
[ -x "$BIN/nikpak" ] || { echo "make-deb.sh: $BIN/nikpak missing - build it first." >&2; exit 1; }
[ -x "$BIN/unNik" ] || { echo "make-deb.sh: $BIN/unNik missing - build it first." >&2; exit 1; }

STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT
mkdir -p binaries

MAINT="Nikkoid LLC <contact@nikkoid.com>"
URL="https://nikpak.nikkoid.com"

# common files under a package root: $1 = root, $2 = pkg, $3 = both|unnik
write_common() {
  root=$1 pkg=$2 mode=$3
  mkdir -p "$root/DEBIAN" "$root/usr/bin" "$root/usr/share/mime/packages" \
           "$root/usr/share/applications" "$root/usr/share/doc/$pkg"

  # the same MIME XML that `nikpak --install-menu` writes per-user, but system-wide
  cat > "$root/usr/share/mime/packages/nikpak.xml" <<'XML'
<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="application/x-nikpak">
    <comment>NikPak archive</comment>
    <sub-class-of type="application/octet-stream"/>
    <magic priority="60"><match type="string" value="\x89NIK\r\n\x1a\n" offset="0"/></magic>
    <glob pattern="*.nik"/>
  </mime-type>
</mime-info>
XML

  cat > "$root/usr/share/applications/unnik.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=unNik
Comment=Browse and extract .nik archives
Exec=/usr/bin/unNik %f
MimeType=application/x-nikpak;
Categories=Utility;Archiving;
Terminal=false
DESKTOP

  if [ "$mode" = "both" ]; then
    cat > "$root/usr/share/applications/nikpak.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=NikPak
Comment=Compress files and folders into .nik archives
Exec=/usr/bin/nikpak
Categories=Utility;Archiving;
Terminal=false
DESKTOP
    cp "$BIN/nikpak" "$root/usr/bin/"
  fi
  cp "$BIN/unNik" "$root/usr/bin/"
  chmod 755 "$root/usr/bin/"* 2>/dev/null || true

  [ -f LICENSE.txt ] && cp LICENSE.txt "$root/usr/share/doc/$pkg/"
  [ -f THIRD-PARTY-NOTICES.txt ] && cp THIRD-PARTY-NOTICES.txt "$root/usr/share/doc/$pkg/"
  [ -f CHANGELOG.md ] && cp CHANGELOG.md "$root/usr/share/doc/$pkg/"
}

write_maintainer() { # $1 = root
  root=$1
  cat > "$root/DEBIAN/postinst" <<'SH'
#!/bin/sh
set -e
if command -v update-mime-database >/dev/null 2>&1; then
  update-mime-database /usr/share/mime >/dev/null 2>&1 || true
fi
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
fi
exit 0
SH
  cat > "$root/DEBIAN/postrm" <<'SH'
#!/bin/sh
set -e
if [ "$1" = "remove" ] || [ "$1" = "purge" ]; then
  command -v update-mime-database >/dev/null 2>&1 && update-mime-database /usr/share/mime >/dev/null 2>&1 || true
  command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
fi
exit 0
SH
  chmod 755 "$root/DEBIAN/postinst" "$root/DEBIAN/postrm"
}

build_deb() { # $1 pkg, $2 mode, $3 short desc, $4 long desc
  pkg=$1 mode=$2
  root="$STAGE/$pkg"
  write_common "$root" "$pkg" "$mode"
  write_maintainer "$root"
  cat > "$root/DEBIAN/control" <<EOF
Package: $pkg
Version: $V
Section: utils
Priority: optional
Architecture: $ARCH
Maintainer: $MAINT
Homepage: $URL
Depends: $DEPENDS
Description: $3
 $4
EOF
  dpkg-deb --build --root-owner-group "$root" "binaries/${pkg}_${V}_${ARCH}.deb" >/dev/null
  echo "  binaries/${pkg}_${V}_${ARCH}.deb"
}

build_deb nikpak both \
  "NikPak and unNik - fast, encrypted archiver for the .nik format" \
  "NikPak makes smaller archives than 7-Zip, RAR and ZIP, protected by quantum-resistant encryption (XChaCha20-Poly1305 + Argon2id). unNik is the free archive browser. Shareware: free for personal use, companies need one licence per seat."

build_deb unnik unnik \
  "unNik - the free .nik archive browser" \
  "unNik browses, extracts, views, deletes and tests .nik archives. Free for everyone."
