# The ERC-8421 gateway for frame transactions. The published image caps the alt mempool's verify
# budget at 100,000 gas, too little for an XMSS check, so this reads it from MAX_VERIFY_GAS instead.
FROM ghcr.io/forshtat/mempool-workshop-sidecar@sha256:c4cdc5c707d94f3968566ba34904d1a7c4ab4dee962d822e1bac1bf6714e022e
ARG PREFIX_JS=/usr/app/packages/executor/lib/services/UserOpValidation/utils/validationPrefix.js
RUN sed -i 's/^export const MAX_VERIFY_GAS = BigInt(100000);/export const MAX_VERIFY_GAS = BigInt(process.env.MAX_VERIFY_GAS ?? 100000);/' "$PREFIX_JS" \
 && grep -q 'process.env.MAX_VERIFY_GAS' "$PREFIX_JS"
COPY config.json /usr/app/config.json
CMD ["standalone", "--configFile", "/usr/app/config.json", "--dataDir", "/usr/app/data", \
     "--api.address", "0.0.0.0", "--api.port", "14337", "--api.wsPort", "14337", \
     "--executor.bundlingMode", "auto", "--redirectRpc", "--explorer.enable"]
