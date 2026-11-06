# Deploys QDay's contracts to the devnet with cast, from pre-built init code (see deploy.sh).
FROM ghcr.io/foundry-rs/foundry:v1.8.6
COPY initcode /initcode
COPY deploy.sh /deploy.sh
ENTRYPOINT ["/bin/sh", "/deploy.sh"]
