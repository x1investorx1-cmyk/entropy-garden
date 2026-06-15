#!/usr/bin/env bash
# Warns (and logs) when a crank wallet's XNT balance drops below threshold.
# Run via systemd timer every few minutes.
THRESHOLD_LAMPORTS=1000000000   # 1 XNT
LOG=/home/entropy-garden/crank-balance.log

check() {
  local name="$1" wallet="$2" rpc="$3"
  local bal
  bal=$(/root/.local/share/solana/install/active_release/bin/solana balance "$wallet" --url "$rpc" 2>/dev/null | awk '{print $1}')
  if [ -z "$bal" ]; then
    echo "$(date -u +%FT%TZ) [$name] WARN: could not read balance (RPC issue?)" >> "$LOG"
    return
  fi
  # compare as float
  local low
  low=$(awk -v b="$bal" 'BEGIN{print (b < 1.0) ? 1 : 0}')
  if [ "$low" = "1" ]; then
    echo "$(date -u +%FT%TZ) [$name] LOW BALANCE: ${bal} XNT — TOP UP $wallet" >> "$LOG"
  else
    echo "$(date -u +%FT%TZ) [$name] ok: ${bal} XNT" >> "$LOG"
  fi
}

check "mainnet" "FL7joRzoH4y1NnXtD8U99kNX8AkfJPTWeFASFXqwVDoh" "https://rpc.mainnet.x1.xyz"
check "testnet" "64EiYTzXGmKvQxicp6pMkdsjphg7XXyYED7TfgWhbGKd" "https://rpc.testnet.x1.xyz"
