#!/usr/bin/env bash
# Stops the local devnet and wipes its data.
set -euo pipefail
WORK=${QDAY_LOCAL_DEVNET:-$HOME/.qday-local-devnet}
docker rm -f qday-local-gateway >/dev/null 2>&1 || true
if [ -d "$WORK" ]; then
  (cd "$WORK" && LIGHTHOUSE_BEACON_1_IP=x LIGHTHOUSE_BEACON_1_ENR=x docker compose down -v --remove-orphans)
  rm -rf "$WORK"
fi
echo "local devnet stopped"
