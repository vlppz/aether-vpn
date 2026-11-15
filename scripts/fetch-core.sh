#!/usr/bin/env bash
# Downloads the pinned Xray-core release for a Rust target triple, verifies its
# SHA-256 digest and installs it as a Tauri sidecar plus geo resources.
#
#   scripts/fetch-core.sh                 # host target
#   scripts/fetch-core.sh x86_64-pc-windows-msvc
set -euo pipefail

XRAY_VERSION="${XRAY_VERSION:-v26.11.9}"
WINTUN_VERSION="0.14.1"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="$ROOT/src-tauri/binaries"
RES_DIR="$ROOT/src-tauri/resources"
TARGET="${1:-$(rustc -vV | sed -n 's/^host: //p')}"

case "$TARGET" in
  aarch64-apple-darwin)       ASSET="Xray-macos-arm64-v8a" ;;
  x86_64-apple-darwin)        ASSET="Xray-macos-64" ;;
  x86_64-unknown-linux-gnu)   ASSET="Xray-linux-64" ;;
  aarch64-unknown-linux-gnu)  ASSET="Xray-linux-arm64-v8a" ;;
  x86_64-pc-windows-msvc)     ASSET="Xray-windows-64";       WINTUN_ARCH="amd64" ;;
  aarch64-pc-windows-msvc)    ASSET="Xray-windows-arm64-v8a"; WINTUN_ARCH="arm64" ;;
  *) echo "unsupported target: $TARGET" >&2; exit 1 ;;
esac

EXT=""
[[ "$TARGET" == *windows* ]] && EXT=".exe"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

BASE="https://github.com/XTLS/Xray-core/releases/download/$XRAY_VERSION"
echo "→ downloading $ASSET.zip ($XRAY_VERSION)"
curl -fsSL -o "$WORK/xray.zip" "$BASE/$ASSET.zip"
curl -fsSL -o "$WORK/xray.zip.dgst" "$BASE/$ASSET.zip.dgst"

expected="$(sed -n 's/^SHA2-256= *//p' "$WORK/xray.zip.dgst" | tr -d '[:space:]')"
if command -v sha256sum >/dev/null; then
  actual="$(sha256sum "$WORK/xray.zip" | cut -d' ' -f1)"
else
  actual="$(shasum -a 256 "$WORK/xray.zip" | cut -d' ' -f1)"
fi
if [[ -z "$expected" || "$expected" != "$actual" ]]; then
  echo "checksum mismatch: expected '$expected' got '$actual'" >&2
  exit 1
fi
echo "✓ sha256 $actual"

mkdir -p "$WORK/x" "$BIN_DIR" "$RES_DIR"
unzip -q -o "$WORK/xray.zip" -d "$WORK/x"
install -m 0755 "$WORK/x/xray$EXT" "$BIN_DIR/xray-$TARGET$EXT"
install -m 0644 "$WORK/x/geoip.dat" "$RES_DIR/geoip.dat"
install -m 0644 "$WORK/x/geosite.dat" "$RES_DIR/geosite.dat"

if [[ -n "${WINTUN_ARCH:-}" ]]; then
  if [[ -f "$WORK/x/wintun.dll" ]]; then
    install -m 0644 "$WORK/x/wintun.dll" "$RES_DIR/wintun.dll"
  else
    echo "→ downloading wintun $WINTUN_VERSION"
    curl -fsSL -o "$WORK/wintun.zip" "https://www.wintun.net/builds/wintun-$WINTUN_VERSION.zip"
    unzip -q -o "$WORK/wintun.zip" -d "$WORK/w"
    install -m 0644 "$WORK/w/wintun/bin/$WINTUN_ARCH/wintun.dll" "$RES_DIR/wintun.dll"
  fi
fi

echo "✓ installed $BIN_DIR/xray-$TARGET$EXT"
"$BIN_DIR/xray-$TARGET$EXT" version 2>/dev/null | head -1 || true
