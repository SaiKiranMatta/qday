# Fact-check of `QDay-Overview.pdf`

> This checks the original pitch, which assumed RFC 8391 XMSS with SHA-256. The build now uses
> keccak256 and is not RFC 8391 compatible (see [xmss-spec.md](xmss-spec.md)), so the RFC gas estimates
> in items 8 and 9 no longer describe it. Use measured gas instead.

Checked 2026-11-05. Verdicts: ✅ holds · ⚠️ partly right / needs rewording · ❌ wrong or unsupported · 🔍 not yet verified.

| # | Claim in the PDF | Verdict | What we found | Source |
|---|------------------|---------|---------------|--------|
| 1 | 2026 research cut the machine needed to break secp256k1 to ~1,200 logical qubits, minutes to days per key | ✅ | Google (Babbush, Zalcman, Gidney), March 2026: ~1,200 or ~1,450 logical qubits depending on the Toffoli budget. Later 2026 estimates: 835 logical qubits (no depth analysis); IonQ: 1,457 logical / ~19.4k physical, ~26 days per attempt | [ePrint 2026/625](https://eprint.iacr.org/2026/625), [postquantum.com (835)](https://postquantum.com/security-pqc/ecc-secp256k1-835-logical-qubits/), [IonQ coverage](https://thequantuminsider.com/2026/09/08/ionq-estimates-20000-qubit-machine-could-break-bitcoin-secp256k1-26-days/) |
| 2 | "Cut the hardware needed by orders of magnitude" | ⚠️ | A large cut, but "orders of magnitude" depends on which baseline and on physical vs logical qubits. Say "a large reduction" and cite the paper | same as 1 |
| 3 | No such machine exists yet | ✅ | Best verified logical-qubit count reported ≈ 96 (QuEra, Jan 2026) | [postquantum.com](https://postquantum.com/security-pqc/ecc-secp256k1-835-logical-qubits/) |
| 4 | EF started a dedicated post-quantum team in 2026, targets a quantum-resistant network by 2029 | ✅ | PQ team formed Jan 2026 (Thomas Coratger). ethereum.org says core PQ infrastructure ~2029, with full migration after that. These are planning milestones, not commitments | [ethereum.org roadmap](https://ethereum.org/roadmap/future-proofing/quantum-resistance/), [The Defiant](https://thedefiant.io/ethereum-foundation-launches-post-quantum-research-hub) |
| 5 | Ethereum plans the same hash-based family for validators | ✅ | Consensus roadmap replaces BLS with hash-based leanXMSS signatures | [Forklog](https://forklog.com/en/news/ethereum-foundation-to-implement-quantum-security-by-2029) |
| 6 | EIP-8141 "frame transactions" lets account code decide what a valid signature is and pay its own fees | ✅ / ⚠️ | True, but it is a **Draft** (type `0x06`), CFI for Hegotá (expected 2027). **Not on mainnet or Sepolia.** Only on devnets (ethrex hegota-testnet chain 8141, and the team's alt-mempool devnet chain 1337). "Accounts **can now** choose" overstates it; say "will be able to" | [EIP-8141](https://eips.ethereum.org/EIPS/eip-8141), [frametx-kit](https://github.com/nikhilkumar1612/frametx-kit) |
| 7 | ERC-8421 is a new alternative mempool designed to carry heavier transactions | ⚠️ | ERC-8421 (Alex Forshtat) = alternative mempool rules for EIP-8141, reputation-based, modelled on ERC-7562. It is not specifically about post-quantum, but it is the place for txs that break the public `MAX_VERIFY_GAS = 100,000` rule. Read the ERC text before quoting it on stage | [Etherspot newsletter](https://www.publish0x.com/etherspot/erc-8421-rethinks-frames-mempools-siggolf-targets-post-quant-xxrpvne) |
| 8 | XMSS is cheap to check on Ethereum / "cheapest PQ option" | ⚠️ | Depends on the variant. RFC 8391 SHA2_10_256 ≈ 1,800 SHA-256 precompile calls → our estimate **~0.4–0.8M gas** (to be measured). Hash-based verifiers *can* be cheap (SPHINCS-style "SPHINCS-minus" reports ~150k gas). A Falcon precompile (EIP-8052, draft) would cost ~2–3k gas if it ships | [poqeth (ePrint 2025/091)](https://eprint.iacr.org/2025/091), [SPHINCS-minus](https://ethresear.ch/t/sphincs-minus-efficient-stateless-post-quantum-signature-verification-on-the-evm/25165), [EIP-8052](https://eips.ethereum.org/EIPS/eip-8052) |
| 9 | Lattice schemes cost "roughly 10–15× more than XMSS" to check | ❌ | Falcon-512 in Solidity ≈ 1.8–2M gas (ETHFALCON, EVM-friendly variant) and ML-DSA ≈ 6.6M. Against RFC-XMSS at ~0.4–0.8M, Falcon is about **2–4×**, not 10–15×. Replace this with our measured numbers in the demo's "numbers" scene | [ETHFALCON](https://github.com/gmh5225/ETHFALCON), [EIP-8052 thread](https://ethereum-magicians.org/t/eip-8052-precompile-for-falcon-support/25860) |
| 10 | XMSS signature ~2.5 KB | ✅ | XMSS-SHA2_10_256: 4 + 32 + 67·32 + 10·32 = **2,500 B** | RFC 8391 |
| 11 | Lattice signatures 0.7–2.4 KB | ✅ | Falcon-512 ≈ 666 B, ML-DSA-44 = 2,420 B | EIP-8052 thread |
| 12 | SPHINCS+ signatures ~3× larger | ✅ | SLH-DSA-128s = 7,856 B ≈ 3.1× | FIPS 205 |
| 13 | SPHINCS+ "costlier to check" | ⚠️ | True for standard SLH-DSA, but EVM-tuned SPHINCS variants report ~150k gas. Soften this answer | SPHINCS-minus |
| 14 | Grover halves hash security; 256-bit → ~128-bit | ✅ | Standard result | — |
| 15 | NIST SP 800-208 says stateful hash-based signatures should only be made in dedicated hardware | ✅ | SP 800-208 requires keygen and signing inside hardware cryptographic modules (FIPS 140 Level 3+) and forbids exporting private keys. Our ESP32-S3 is **not** such a module. Say "follows the intent of SP 800-208", not "compliant" | NIST SP 800-208 |
| 16 | Reusing one page on two different messages enables forgery; same message twice is harmless | ✅ | Standard WOTS property (deterministic signing gives an identical signature for the same message) | RFC 8391 |
| 17 | "Device counter can never go back, even if memory is copied or rolled back" | ⚠️ | Not true for an ESP32-S3 flash counter: a flash image can be dumped and restored. Needs a secure element or eFuse. Reword, or add a secure element | design spec §6 |
| 18 | EIP-7702 doesn't help because the old ECDSA key stays in charge | ✅ | A delegated EOA's key can still sign | EIP-7702 |
| 19 | Signing on device takes < 0.1 s | 🔍 | Plausible with a cached tree (~3k SHA-256 calls). Measure on hardware | — |
| 20 | Restore "skip ahead 64 pages" makes reuse impossible | ⚠️ | Safe for pages below the true on-chain `nextIndex`. **Residual risk** if a lost device holds >64 signed-but-unsent txs. See [risks](risks-and-blockers.md#r7) | design spec §8 |
| 21 | "Accounts carry on forever with the same address" (book rotation) | ✅ | By design (`rotate` on a reserved page) | design spec §5 |

## Corrections to make in the pitch

1. Item 9: drop "10–15×" and use our measured gas table.
2. Item 6: "EIP-8141 (draft, targeted for Hegotá) **will let** accounts…", and run the demo on a devnet.
3. Item 15: "follows NIST SP 800-208's hardware requirement in spirit; a production device needs a
   certified secure element".
4. Item 17: say what the counter actually protects against.
5. The build is a non-RFC variant, so don't call it "standards-aligned".
