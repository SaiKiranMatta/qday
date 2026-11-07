# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Primary: ETHGlobal judges and reviewers (human, and AI agents reading the repo) evaluating QDay in a few
minutes. They need to understand the idea, believe it is real, and try it without owning the hardware.

Also served by the live wallet: anyone with a QDay board on a desktop browser that has Web Serial
(Chrome, Edge, Brave).

## Product Purpose

QDay is a quantum-safe hardware wallet for Ethereum. An ESP32-S3 device signs with post-quantum
signatures (hash-based XMSS over keccak256, or ML-DSA-44 from NIST FIPS 204), and an EIP-8141 smart
account per scheme verifies them on-chain, so no ECDSA key holds authority over funds. It also takes
private payments whose finding (X-Wing: ML-KEM-768 with X25519) and spending (one-time ML-DSA keys) are
both post-quantum.

Success for the landing page: a reviewer understands what QDay is and why it matters, sees proof that it
runs, and opens the wallet to try it.

## Positioning

As far as the team can find, the first hardware wallet that signs for Ethereum accounts with no ECDSA key
at all (prior art in `docs/fact-check.md`, items 22 to 24). Built natively on EIP-8141 frame transactions,
which core developers scheduled for the Hegotá upgrade. Two post-quantum schemes on one device from one
seed, and private payments that stay post-quantum end to end, where post-quantum stealth-address proposals
so far protect only the finding.

## Operating Context

- The wallet (React web app) talks to the device over USB with Web Serial; every transaction is decoded
  and approved on the device with a passcode.
- It runs on QDay's own hosted EIP-8141 devnet (chain 1337): RPC `rpc-qday.unitlabs.xyz`, gateway
  `gateway-qday.unitlabs.xyz`, live wallet at `qday.unitlabs.xyz`.
- **Try without a device** opens a demo wallet whose keys live in the browser, on the same chain.
- The device: YD-ESP32-S3 N16R8, a 2.8" ILI9341 touch display, four push keys, in a 3D-printed case
  modelled in OpenSCAD.
- Placement (confirmed): the landing page lives at `/`, the wallet moves to `/app`, and explorer links stay
  at `/tx/<hash>`.

## Capabilities and Constraints

- A prototype: not audited, devnet only. EIP-8141 is scheduled for Hegotá (expected 2027) but is a draft
  and not on mainnet or Sepolia.
- XMSS: 2,500-byte signatures, 1,024 one-time keys per tree, two locks against reuse (the device counter
  moves before signing; the account refuses an index below `nextIndex`). ML-DSA-44: 2,420-byte signatures,
  stateless.
- Private payments carry ETH only; ML-DSA accounts send ETH only; the devnet's dev key deploys ML-DSA
  accounts and relays claims. The demo wallet signs XMSS only, for test ETH, with unprotected keys.
- The ESP32-S3 is not a secure element. The one-time stealth signer is an unreviewed variant.

## Brand Commitments

- Name: QDay. Wordmark: lowercase italic "qday", heavy oblique Inter, as in the wallet.
- The landing page stays within the wallet's existing theme (confirmed by the user).
- Voice: plain and precise. Claims are scoped and checkable: "as far as we can find", "quantum-safe by
  construction", never "proven quantum-secure" (`docs/fact-check.md`, item 24).

## Evidence on Hand

- Live wallet and demo: `qday.unitlabs.xyz`. Repository: `github.com/SaiKiranMatta/qday`.
- Measured gas and runs signed on the real board: `docs/devnet.md` (XMSS transfer about 192k to 197k gas,
  ML-DSA transfer 1.31M, private payment 360,325, claim about 1.38M to 1.57M).
- Hardware: `hardware/case/` (OpenSCAD source `qday-case.scad`, `bezel.stl`, `tray.stl`, `shim.stl`,
  `coupon.stl`, preview renders).
- Device screens rendered by the device's own code: `docs/screens/` (five themes), composites in
  `docs/images/`. Web screenshots: `docs/images/`, `ethglobal-assets/`.
- Test counts: 654 Rust, 124 contract, 67 web tests.
- Absent, never to be fabricated: photos of the physical device (placeholders until the team uploads them),
  users, customers, testimonials, an audit, mainnet deployment, pricing.

## Product Principles

1. Prove, don't claim: every statement points at a run, a number, a file or the live demo.
2. Honest limits are part of the pitch: prototype, devnet, unaudited, said plainly.
3. Show the hardware: the device and its signing ritual are the product, not an afterthought.
4. One click to try: the demo wallet is always one action away.
