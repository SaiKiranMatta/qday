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
  gateway, and QDay's contracts at fixed addresses (`deploy/coolify`).
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
6. **More signature schemes.** The wallet already describes schemes as data
   (`web/src/lib/schemes.ts`), so a lattice or stateless hash-based scheme can sit beside XMSS.

## Before real funds

These are outside the code, and none is done:

- **EIP-8141 on mainnet.** It is a draft, considered for the Hegotá upgrade; until then QDay runs on
  devnets only ([fact-check](fact-check.md), item 6).
- **An independent audit** of the signature scheme, the verifier and the account.
- **A certified secure element** to meet NIST SP 800-208's requirement for stateful hash-based
  signatures ([fact-check](fact-check.md), item 15).
- **A standard key derivation**: QDay derives tree secrets from BIP39 entropy with its own keccak256
  construction, because no standard exists yet ([xmss-spec.md](xmss-spec.md#keys)).
