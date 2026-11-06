# The devnet's genesis: the upstream generator with QDay's values (chain 1337, the throwaway premine
# key, 6-second slots), written once and kept across restarts.
FROM ethpandaops/ethereum-genesis-generator@sha256:51decce4284d8498dabfa38ae13ddce84cbf8c7be4f33a32b798ce10c9868e77
COPY values.env /config/values.env
COPY run.sh /qday-genesis.sh
RUN chmod +x /qday-genesis.sh
ENTRYPOINT ["/qday-genesis.sh"]
