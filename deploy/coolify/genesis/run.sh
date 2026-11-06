#!/bin/sh
# Writes the chain's genesis on the first start only: later starts find it and keep the chain. Then
# stays up, so Coolify sees a running service, with a health check the other services wait on.
set -e
if [ -f /data/metadata/genesis.json ]; then
  echo "genesis already written; keeping the chain"
else
  export GENESIS_TIMESTAMP=$(date +%s)
  /work/entrypoint.sh all
fi
exec sleep 2147483647
