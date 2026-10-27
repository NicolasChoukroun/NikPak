#!/usr/bin/env sh
# build.sh - build nikpak and unNik on macOS / Linux.   Usage: ./build.sh
set -e
cd "$(dirname "$0")"
if ! command -v cargo >/dev/null 2>&1; then
  echo "Rust (cargo) not found. Install it with:"
  echo "    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
  echo "(macOS also needs the Xcode command line tools:  xcode-select --install)"
  echo "(Debian/Ubuntu also needs a C compiler:           sudo apt install build-essential)"
  exit 1
fi
# nikpak 1.2+ needs Rust 1.89 or newer (required by the preflate-rs library)
RV=$(rustc --version | sed -E 's/^rustc ([0-9]+)\.([0-9]+).*/\1 \2/')
set -- $RV
if [ "$1" -eq 1 ] && [ "$2" -lt 89 ]; then
  echo "Rust $1.$2 is too old (1.89 or newer is needed). Updating: rustup update stable"
  rustup update stable
fi
cargo build --release
mkdir -p dist
cp target/release/nikpak dist/nikpak
cp target/release/unNik dist/unNik
echo "OK -> $(pwd)/dist/nikpak"
echo "OK -> $(pwd)/dist/unNik   (the free archive browser)"
echo "Optional install:  sudo cp dist/nikpak dist/unNik /usr/local/bin/"
