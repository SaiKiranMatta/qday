# Roadmap

What QDay does today, what comes next, and what has to happen outside this repository before it can
hold real funds. Everything under **Built** runs now; each line points at where to check it.

## Built

### Signatures and the account

- **Hash-based signatures over keccak256**: an XMSS-style scheme with 1,024 one-time keys per tree and
  2,500-byte signatures, defined byte for byte in [xmss-spec.md](xmss-spec.md)
  (`crates/qday-xmss`).
- **On-chain verification** in Solidity with inline assembly: 68,537 to 87,297 gas per signature in
  Foundry (`contracts/src/XMSSVerifier.sol`, `contracts/test/XMSSVerifier.t.sol`).
- **An EIP-8141 account with no ECDSA key**: `validate()` checks the signature in a VERIFY frame,
  `spendIndex()` moves `nextIndex` forward, and a reused key is refused with `KeyAlreadyUsed`
  (`contracts/src/QDayFrameAccount.sol`).
- **Self-deploying accounts**: the address follows from the device's key before the account exists,
  and the account pays for its own creation in its first transaction, approved on the device
  (`contracts/src/QDayAccountFactory.sol`, EIP-1167 clones).
- **Rotation at the same address**: keys 1008 to 1023 can only switch the account to a new tree.

### A second scheme: ML-DSA-44

- **ML-DSA-44 signing on the device** (NIST FIPS 204, through the `fips204` crate), from the same seed
  as XMSS, with the public key kept in flash so showing it needs no passcode (`crates/qday-mldsa`,
  `crates/qday-wallet/src/mlkey.rs`).
- **Switching schemes on the device**, after approval: switching back finds the XMSS key and counter
  unchanged, checked on the board.
- **On-chain verification at about 1.23M gas**, with Fireblocks' verifier, patched so the mempool
  accepts it during validation (`contracts/mldsa`, [NOTICE](../contracts/mldsa/NOTICE.md)).
- **An ML-DSA account and factory** that bind the verifier and the key contract by code hash and
  stake in the mempool's registry (`contracts/src/QDayMlDsaAccount.sol`, `QDayMlDsaFactory.sol`).
- **In the web wallet**: create the ML-DSA account and send ETH from it.

### Private payments

- **A post-quantum stealth address**: X-Wing (ML-KEM-768 with X25519) gives payer and owner a shared
  secret that turns the owner's published ML-DSA key into a one-time key (`crates/qday-wallet/src/stealth.rs`).
- **A shared pool** that holds each payment under its key's hash and pays it out once, for a signature
  from the one-time key; nothing is deployed per payment (`contracts/src/QDayLatticePool.sol`).
- **Finding and spending kept apart**: the view key finds payments in the browser and can't spend;
  the claim is signed on the device.
- **In the web wallet**: share the private address, pay one from **Send**, see payments arrive, claim
  them to any address.

### The device

- **ESP32-S3 firmware** that decodes every transaction itself and shows what it will sign: ETH
  transfers, ERC-20 transfers to tokens on its built-in list (with a warning and raw units for any
  other token), account creation and rotation (`firmware/`, `crates/qday-ui`, `crates/qday-wallet`).
- **Two locks on one-time keys**: the device's counter moves before it signs, and the chain refuses an
  index below `nextIndex`.
- **Passcode, 24-word backup and restore**: a restored device reads `nextIndex` from the chain and
  skips 64 keys ahead.
- **Five themes**, rendered to PNG on the laptop by the same code the device runs
  ([screens](screens/all-themes.png)).

### The web wallet

- **Live at [qday.unitlabs.xyz](https://qday.unitlabs.xyz)**, against QDay's own hosted devnet
  (`web/`).
- **Connects over Web Serial**: no browser extension, no key in the browser.
- **Account setup in two clicks**: get test ETH, then create the account from the device.
- **Sends ETH and ERC-20 tokens**, with a review that matches the device's screen line for line, and
  live progress: signed, submitted, confirmed in block N.
- **Activity read from the chain** and a **built-in explorer** that decodes frame transactions frame by
  frame, which standard explorers can't (`web/src/TxPage.tsx`, `web/src/lib/explorer.ts`).
- **One codec for browser, laptop and device**: the web wallet builds transactions with the device's
  own Rust code compiled to WebAssembly (`crates/qday-wasm`, `packages/frame`).

### The network

- **A hosted EIP-8141 devnet** that stays up: ethrex and Lighthouse on chain 1337, the ERC-8421
  gateway, and all of QDay's contracts at fixed addresses, the ML-DSA and pool contracts included
  (`deploy/coolify`).
- **A local devnet** in one command for development (`tools/devnet/local/up.sh`).

## Next

In rough order of value for the people who would use it.

1. **Key refresh in the web wallet.** The device and the CLI already prepare a new tree and rotate to
   it (`qday device-prepare`, `qday device-rotate`); the web wallet should offer it when signatures run
   low.
2. **Recovery and clone protection, shown in the wallet.** Restore a device, watch it skip ahead, and
   see the chain refuse a reused key, from the web wallet instead of the CLI.
3. **Connect to other apps.** An EIP-1193 provider (browser extension) so any dApp can ask the QDay to
   sign, with the device decoding what it can and warning on the rest.
4. **Batch payments.** Several transfers in one frame transaction and one signature, reviewed as a list
   on the device.
5. **Hardware rollback protection.** A flash counter can be dumped and restored; an eFuse high-water
   mark or a secure element closes that ([R8](risks-and-blockers.md)).
6. **Tokens for ML-DSA accounts and private payments.** Both carry ETH only today.
7. **No dev key in the loop.** On the devnet a funded dev key deploys ML-DSA accounts and submits
   private-payment claims. Anyone may already submit a claim and is repaid from the note, so a public
   relayer can take that over; deploying accounts needs a paymaster or a cheaper account (below).
8. **Show who a private payment is for.** The device shows the amount and the pool; it should show a
   fingerprint of the private address too, so a page can't swap it unseen.
9. **A cheaper ML-DSA account.** About 37M gas to deploy, almost all of it the key, because code costs
   about 1,555 gas per byte on the devnet; a shared key contract per device, or a precompile, would cut
   it.

## Before real funds

These are outside the code, and none is done:

- **EIP-8141 on mainnet.** It is a draft, considered for the Hegotá upgrade; until then QDay runs on
  devnets only ([fact-check](fact-check.md), item 6).
- **An independent audit** of the signature scheme, both verifiers, the accounts and the pool.
- **A review of the one-time ML-DSA signer** used for private payments, which is QDay's own unreviewed
  variant, and of the privacy assumption that ML-KEM ciphertexts don't reveal their recipient.
- **A certified secure element** to meet NIST SP 800-208's requirement for stateful hash-based
  signatures ([fact-check](fact-check.md), item 15).
- **A standard key derivation**: QDay derives tree secrets from BIP39 entropy with its own keccak256
  construction, because no standard exists yet ([xmss-spec.md](xmss-spec.md#keys)).
