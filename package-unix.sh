#!/usr/bin/env sh
# package-unix.sh - build NikPak + unNik and make the macOS / Linux download of the web site.
#
#   ./package-unix.sh            on a Mac   -> binaries/nikpak-<version>-macos.zip
#                                on Linux   -> binaries/nikpak-<version>-linux-<arch>.tar.gz
#
# On a Mac both Intel and Apple-silicon programs are built and joined (universal binaries)
# when the two Rust targets are installed (the script installs them with rustup).
# On Linux, set ZIG=1 to build with cargo-zigbuild against glibc 2.28, so the programs
# also run on older distributions (pip install ziglang; cargo install cargo-zigbuild).
# The Windows downloads are made by buildall.ps1.
set -e
T="${CARGO_TARGET_DIR:-target}"
cd "$(dirname "$0")"
V=$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -1)
OS=$(uname -s)
ARCH=$(uname -m)
[ "$ARCH" = "amd64" ] && ARCH=x86_64
[ "$ARCH" = "arm64" ] && [ "$OS" = "Linux" ] && ARCH=aarch64
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

if [ "$OS" = "Darwin" ]; then
  NAME="nikpak-$V-macos"
  rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null 2>&1 || true
  if cargo build --release --target aarch64-apple-darwin --bin nikpak --bin unNik &&
     cargo build --release --target x86_64-apple-darwin --bin nikpak --bin unNik; then
    mkdir -p "$STAGE/$NAME"
    for p in nikpak unNik; do
      lipo -create -output "$STAGE/$NAME/$p" "$T/aarch64-apple-darwin/release/$p" "$T/x86_64-apple-darwin/release/$p"
    done
  else
    echo "Universal build failed: building for this Mac only ($ARCH)."
    cargo build --release --bin nikpak --bin unNik
    mkdir -p "$STAGE/$NAME"
    cp "$T/release/nikpak" "$T/release/unNik" "$STAGE/$NAME/"
  fi
else
  NAME="nikpak-$V-linux-$ARCH"
  if [ "$ZIG" = "1" ]; then
    cargo zigbuild --release --target "$ARCH-unknown-linux-gnu.2.28" --bin nikpak --bin unNik
    BIN="$T/$ARCH-unknown-linux-gnu/release"
  else
    cargo build --release --bin nikpak --bin unNik
    BIN="$T/release"
  fi
  mkdir -p "$STAGE/$NAME"
  cp "$BIN/nikpak" "$BIN/unNik" "$STAGE/$NAME/"
fi

D="$STAGE/$NAME"
chmod +x "$D/nikpak" "$D/unNik"
[ -f LICENSE.txt ] && cp LICENSE.txt "$D/"
[ -f THIRD-PARTY-NOTICES.txt ] && cp THIRD-PARTY-NOTICES.txt "$D/"
chmod 644 "$D"/*.txt 2>/dev/null || true

cat > "$D/install.sh" <<'EOS'
#!/bin/sh
# install.sh - install NikPak and unNik for this user, with the right-click menu.
#   ./install.sh               install (Linux: ~/.local/bin, macOS: /usr/local/bin)
#   PREFIX=/opt/bin ./install.sh   install somewhere else
#   ./install.sh --uninstall   remove the programs and the menu
set -e
cd "$(dirname "$0")"
if [ "$(uname -s)" = "Darwin" ]; then BIN="${PREFIX:-/usr/local/bin}"; else BIN="${PREFIX:-$HOME/.local/bin}"; fi
SUDO=""
mkdir -p "$BIN" 2>/dev/null || SUDO=sudo
[ -w "$BIN" ] || SUDO=sudo
if [ "$1" = "--uninstall" ]; then
  "$BIN/nikpak" --uninstall-menu 2>/dev/null || true
  $SUDO rm -f "$BIN/nikpak" "$BIN/unNik"
  echo "NikPak was removed from $BIN."
  exit 0
fi
$SUDO mkdir -p "$BIN"
$SUDO cp nikpak unNik "$BIN/"
$SUDO chmod 755 "$BIN/nikpak" "$BIN/unNik"
# downloaded programs are quarantined by macOS: allow these two
[ "$(uname -s)" = "Darwin" ] && $SUDO xattr -d com.apple.quarantine "$BIN/nikpak" "$BIN/unNik" 2>/dev/null || true
"$BIN/nikpak" --install-menu || echo "(The right-click menu could not be added; NikPak works without it.)"

# Put the folder on the PATH, so `nikpak` and `unNik` open from any shell.
# /usr/local/bin (macOS) is already in the default PATH, so nothing is needed.
case ":$PATH:" in
  *":$BIN:"*) ;;
  *)
    LINE="export PATH=\"$BIN:\$PATH\""
    case "${SHELL##*/}" in
      zsh)  RC="$HOME/.zshrc" ;;
      bash) RC="$HOME/.bashrc" ;;
      *)    RC="$HOME/.profile" ;;
    esac
    [ -f "$RC" ] || RC="$HOME/.profile"
    if grep -Fqx "$LINE" "$RC" 2>/dev/null; then
      :
    else
      printf '\n# added by NikPak so that `nikpak` opens in any shell\n%s\n' "$LINE" >> "$RC"
      echo "Added $BIN to the PATH in $RC (open a new terminal to use it)."
    fi
    PATH="$BIN:$PATH"
    ;;
esac

echo
VER=$("$BIN/nikpak" -V 2>/dev/null | head -1 | cut -d' ' -f2)
echo "NikPak $VER is installed in $BIN."
echo "  nikpak          opens the NikPak window (drop files to compress)"
echo "  unNik           opens the archive browser"
echo "  nikpak -help    command line"
echo "  nikpak -V       version"
case ":$PATH:" in
  *":$BIN:"*) ;;
  *) echo; echo "Note: start a new terminal, or run:  $LINE" ;;
esac
EOS
chmod +x "$D/install.sh"

cat > "$D/README.txt" <<EOS
NikPak $V for $( [ "$OS" = "Darwin" ] && echo macOS || echo Linux ) - NikPak (archiver) and unNik (free archive browser)

Install:    ./install.sh
Uninstall:  ./install.sh --uninstall

Or run the programs from this folder: ./nikpak (window) - ./nikpak -help (command line) - ./unNik
User guide: https://nikpak.nikkoid.com
NikPak is shareware: free for personal use, companies need one licence per seat.
(c) 2026 Nicolas Choukroun, distributed by Nikkoid LLC.
EOS
chmod 644 "$D/README.txt"

mkdir -p binaries
if [ "$OS" = "Darwin" ]; then
  (cd "$STAGE" && rm -f "$OLDPWD/binaries/$NAME.zip" && zip -qry "$OLDPWD/binaries/$NAME.zip" "$NAME")
  OUT="binaries/$NAME.zip"
else
  tar -C "$STAGE" -czf "binaries/$NAME.tar.gz" "$NAME"
  OUT="binaries/$NAME.tar.gz"
fi
echo "OK -> $(pwd)/$OUT"
echo "Upload it to the downloads/ folder of the web site (name set in website/site.json)."
