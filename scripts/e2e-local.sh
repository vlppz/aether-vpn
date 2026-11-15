#!/usr/bin/env bash
# End-to-end test of the real app against local Xray servers — no VPN
# subscription needed. For each protocol it:
#   1. starts the debug Aether binary with an isolated data dir,
#   2. lets it import a share link and connect (AETHER_E2E_LINK hook),
#   3. sends HTTPS traffic through the SOCKS and HTTP inbounds,
#   4. checks traffic stats, the system proxy (first case only, macOS),
#   5. sends SIGTERM and verifies Xray stopped and the proxy was restored.
#
# Requires internet access (REALITY borrows www.cloudflare.com's handshake and
# the test fetches https://www.gstatic.com/generate_204).
#
#   scripts/e2e-local.sh            # build + run
#   SKIP_SYSPROXY=1 scripts/e2e-local.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
XRAY="$ROOT/src-tauri/binaries/xray-$TRIPLE"
APP="$ROOT/target/debug/aether"
WORK="$(mktemp -d)"
SOCKS=21808 HTTP=21809 METRICS=21813
PASS=0 FAIL=0
SERVER_PID="" APP_PID=""

cleanup() {
  [[ -n "$APP_PID" ]] && kill "$APP_PID" 2>/dev/null || true
  [[ -n "$SERVER_PID" ]] && kill "$SERVER_PID" 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT

ok()   { echo "  ✓ $*"; PASS=$((PASS + 1)); }
bad()  { echo "  ✗ $*"; FAIL=$((FAIL + 1)); }

[[ -x "$XRAY" ]] || { echo "missing $XRAY — run scripts/fetch-core.sh"; exit 1; }
echo "→ building app"
node "$ROOT/scripts/prepare-sidecars.mjs" >/dev/null
cargo build -q -p aether --manifest-path "$ROOT/Cargo.toml"

# ---------------------------------------------------------------- server
UUID="$("$XRAY" uuid)"
KEYS="$("$XRAY" x25519)"
PRIV="$(sed -n 's/^PrivateKey: //p' <<<"$KEYS")"
PUB="$(sed -n 's/^Password (PublicKey): //p' <<<"$KEYS")"
[[ -z "$PUB" ]] && PUB="$(sed -n 's/^Public key: //p' <<<"$KEYS")"
SS_KEY="$(head -c 16 /dev/urandom | base64)"
SID="6ba85179e30d4fc2"

cat >"$WORK/server.json" <<EOF
{
  "log": { "loglevel": "warning" },
  "inbounds": [
    { "listen": "127.0.0.1", "port": 24443, "protocol": "vless",
      "settings": { "clients": [{ "id": "$UUID", "flow": "xtls-rprx-vision" }], "decryption": "none" },
      "streamSettings": { "network": "raw", "security": "reality",
        "realitySettings": { "target": "www.cloudflare.com:443", "serverNames": ["www.cloudflare.com"],
          "privateKey": "$PRIV", "shortIds": ["$SID"] } } },
    { "listen": "127.0.0.1", "port": 24444, "protocol": "shadowsocks",
      "settings": { "method": "2022-blake3-aes-128-gcm", "password": "$SS_KEY", "network": "tcp,udp" } },
    { "listen": "127.0.0.1", "port": 24445, "protocol": "vless",
      "settings": { "clients": [{ "id": "$UUID" }], "decryption": "none" },
      "streamSettings": { "network": "xhttp", "xhttpSettings": { "path": "/x" } } },
    { "listen": "127.0.0.1", "port": 24446, "protocol": "trojan",
      "settings": { "clients": [{ "password": "e2e-secret" }] },
      "streamSettings": { "network": "ws", "wsSettings": { "path": "/ws" } } }
  ],
  "outbounds": [{ "protocol": "freedom" }]
}
EOF
"$XRAY" run -c "$WORK/server.json" >"$WORK/server.log" 2>&1 &
SERVER_PID=$!
sleep 1
kill -0 "$SERVER_PID" 2>/dev/null || { cat "$WORK/server.log"; echo "server failed"; exit 1; }

urlenc() { python3 -c 'import sys,urllib.parse;print(urllib.parse.quote(sys.argv[1],safe=""))' "$1"; }
LINKS=(
  "vless://$UUID@127.0.0.1:24443?encryption=none&flow=xtls-rprx-vision&security=reality&sni=www.cloudflare.com&fp=chrome&pbk=$PUB&sid=$SID&type=tcp#E2E%20Reality"
  "ss://2022-blake3-aes-128-gcm:$(urlenc "$SS_KEY")@127.0.0.1:24444#E2E%20SS2022"
  "vless://$UUID@127.0.0.1:24445?encryption=none&security=none&type=xhttp&path=%2Fx#E2E%20XHTTP"
  "trojan://e2e-secret@127.0.0.1:24446?security=none&type=ws&path=%2Fws#E2E%20Trojan%20WS"
)

proxy_state() { networksetup -getwebproxy "Wi-Fi" 2>/dev/null | tr '\n' ' '; }
# macOS keeps stale server/port fields once a proxy is off, so compare the switch.
proxy_enabled() { networksetup -getwebproxy "Wi-Fi" 2>/dev/null | sed -n 's/^Enabled: //p'; }

run_case() {
  local link="$1" sysproxy="$2"
  local data="$WORK/data-$RANDOM"
  mkdir -p "$data"
  cat >"$data/settings.json" <<EOF
{ "core": { "socksPort": $SOCKS, "httpPort": $HTTP, "metricsPort": $METRICS, "logLevel": "warning" },
  "setSystemProxy": $sysproxy, "startMinimized": true, "closeToTray": true }
EOF
  local before
  before="$(proxy_enabled)"
  AETHER_DATA_DIR="$data" AETHER_E2E_LINK="$link" "$APP" >"$data/app.log" 2>&1 &
  APP_PID=$!

  local waited=0
  until grep -q "AETHER_E2E:" "$data/app.log" 2>/dev/null || ((waited >= 40)); do
    sleep 0.5; waited=$((waited + 1))
  done
  if ! grep -q "AETHER_E2E: connected" "$data/app.log"; then
    bad "connect: $(grep -m1 "AETHER_E2E" "$data/app.log" || echo timeout)"
    kill "$APP_PID" 2>/dev/null; APP_PID=""
    return
  fi
  ok "connected"

  code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 15 -x "socks5h://127.0.0.1:$SOCKS" https://www.gstatic.com/generate_204 || true)"
  [[ "$code" == 204 ]] && ok "HTTPS via SOCKS inbound → $code" || bad "HTTPS via SOCKS inbound → $code"
  code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 15 -x "http://127.0.0.1:$HTTP" https://www.gstatic.com/generate_204 || true)"
  [[ "$code" == 204 ]] && ok "HTTPS via HTTP inbound → $code" || bad "HTTPS via HTTP inbound → $code"

  up="$(curl -s "http://127.0.0.1:$METRICS/debug/vars" | python3 -c 'import sys,json;print(json.load(sys.stdin)["stats"]["outbound"]["proxy"]["downlink"])' 2>/dev/null || echo 0)"
  ((up > 0)) && ok "traffic counted through proxy outbound ($up bytes down)" || bad "no traffic stats"

  waited=0
  until grep -q "AETHER_E2E: latency" "$data/app.log" 2>/dev/null || ((waited >= 30)); do
    sleep 0.5; waited=$((waited + 1))
  done
  ms="$(sed -n 's/^AETHER_E2E: latency [^ ]* //p' "$data/app.log" | head -1)"
  [[ -n "$ms" && "$ms" -gt 0 ]] && ok "real-delay test through throw-away xray: ${ms} ms" || bad "latency test: ${ms:-none}"

  if [[ "$sysproxy" == true ]]; then
    proxy_state | grep -q "Enabled: Yes Server: 127.0.0.1 Port: $SOCKS" \
      && ok "system proxy points at 127.0.0.1:$SOCKS" || bad "system proxy not set: $(proxy_state)"
  fi

  kill -TERM "$APP_PID"
  for _ in $(seq 1 30); do kill -0 "$APP_PID" 2>/dev/null || break; sleep 0.3; done
  if kill -0 "$APP_PID" 2>/dev/null; then bad "app did not exit on SIGTERM"; kill -9 "$APP_PID"; else ok "app exited on SIGTERM"; fi
  APP_PID=""
  if nc -z 127.0.0.1 "$SOCKS" 2>/dev/null; then bad "xray still listening after exit"; else ok "xray stopped"; fi
  if [[ "$sysproxy" == true ]]; then
    [[ "$(proxy_enabled)" == "$before" ]] && ok "system proxy restored (Enabled: $before)" || bad "proxy not restored: $(proxy_state)"
  fi
}

first=true
for link in "${LINKS[@]}"; do
  name="$(python3 -c 'import sys,urllib.parse;print(urllib.parse.unquote(sys.argv[1].split("#")[-1]))' "$link")"
  echo "→ $name"
  sp=false
  if $first && [[ "$(uname)" == Darwin && -z "${SKIP_SYSPROXY:-}" ]]; then sp=true; fi
  first=false
  run_case "$link" "$sp"
done

echo
echo "passed: $PASS  failed: $FAIL"
((FAIL == 0))
