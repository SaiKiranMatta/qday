# Phase 0–1: Signature Core and Gas Gate — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Commit rules:** Conventional Commits; the subject says what the change adds and never mentions phases, plans or tasks. Add the Claude co-author trailer only for minor changes (tests, docs, small fixes), never for feature or core work.

**Goal:** A `no_std` Rust crate implementing RFC 8391 `XMSS-SHA2_10_256` that interoperates with the RFC
reference implementation, plus a Yul on-chain verifier checked against the same vectors with a
**measured gas number**, which drives the Fri 04:00 OD1 gate.

**Architecture:** `crates/qday-xmss` is the single source of truth for the scheme, shared later by the
host CLI and the ESP32-S3 firmware. It emits JSON test vectors into `vectors/`. `contracts/yul/` holds
a pure Yul verifier (no EIP-8141 opcodes) wrapped in a test harness object, compiled with
`solc --strict-assembly` and exercised from Foundry.

**Tech Stack:** Rust 1.80+ (`sha2` 0.10, `no_std` + `alloc`), C reference `XMSS/xmss-reference` +
OpenSSL, Foundry (forge-std), solc ≥ 0.8.30 (Yul, `prague` EVM).

**Spec:** [docs/superpowers/specs/2026-11-05-qday-design.md](../specs/2026-11-05-qday-design.md)

## Global Constraints

- Parameter set: RFC 8391 `XMSS-SHA2_10_256`, OID `0x00000001`, n = 32, w = 16, len = 67, h = 10, 1,024 pages.
- Signature bytes = idx (4, big-endian) ‖ r (32) ‖ 67 × 32 WOTS+ ‖ 10 × 32 auth = **2,500**.
- Public key bytes = OID (4) ‖ root (32) ‖ PUB_SEED (32).
- Hash domain separators (32-byte big-endian padding): F = 0, H = 1, H_msg = 2, PRF = 3, PRF_keygen = 4.
- Whenever the ADRS type changes, words 4–7 are zeroed (RFC 8391 §2.5).
- The crate API takes `idx` explicitly and is **stateless**; page bookkeeping belongs to the device
  (later plan). Every public `sign` doc comment must say so.
- Non-standard variants (w=4, target-sum) are **out of this plan**; they are only added if the OD1 gate says so.

## File Structure

```
Cargo.toml                         workspace
crates/qday-xmss/
  Cargo.toml
  src/lib.rs                       re-exports, SecretKey/PublicKey/Signature, sign/verify
  src/params.rs                    constants
  src/adrs.rs                      32-byte hash address
  src/hash.rs                      F, H, H_msg, PRF, PRF_keygen, thash_f, rand_hash
  src/wots.rs                      base_w, checksum digits, chain, WOTS+ sign/pk_gen/pk_from_sig
  src/tree.rs                      ltree, full-tree build, node offsets, auth path
  examples/gen_vectors.rs          writes vectors/qday/basic.json
  examples/sign_hex.rs             prints pk/msg/sig hex for the reference verifier
  tests/rfc_reference.rs           verifies reference-produced signatures
tools/xmss-ref/
  driver.c                         keypair+sign → JSON, or verify(pk,msg,sig) → exit code
  build.sh                         clones + builds xmss-reference with driver.c
vectors/
  rfc8391/ref_sigs.json            produced by the reference implementation
  qday/basic.json                  produced by qday-xmss
contracts/
  foundry.toml
  yul/XmssVerifierHarness.yul      Yul object: runtime = xmss verify
  scripts/build-yul.sh             solc --strict-assembly → out-yul/*.bin
  test/XmssVerifier.t.sol          vectors + tamper tests + gas logging
docs/gas.md                        measured numbers + OD1 recommendation
```

---

### Task 1: Record the devnet probe (Phase 0)

**Files:**
- Modify: `docs/devnet.md`

**Interfaces:** Produces `RPC_URL` (chain 1337) for the later account plan.

- [ ] **Step 1: Query the node**

```bash
export RPC_URL=<the alt-mempool devnet RPC the team used for the Nov 5 probe>
cast chain-id --rpc-url "$RPC_URL"
cast client --rpc-url "$RPC_URL"
cast block latest --rpc-url "$RPC_URL" --field number
```

Expected: `1337`, a client version string, and a block number that increases between runs.

- [ ] **Step 2: Record the probe transaction**

```bash
cast tx <probe_tx_hash> --rpc-url "$RPC_URL"
cast receipt <probe_tx_hash> --rpc-url "$RPC_URL"
```

Copy the chain id, RPC, client version, tx hash and per-frame gas into the table in `docs/devnet.md`.
Never paste private keys.

- [ ] **Step 3: Commit**

```bash
git add docs/devnet.md
git commit -m "docs: add devnet 1337 connection details and frame tx probe"
```

---

### Task 2: Workspace, params and ADRS

**Files:**
- Create: `Cargo.toml`, `crates/qday-xmss/Cargo.toml`, `crates/qday-xmss/src/lib.rs`, `crates/qday-xmss/src/params.rs`, `crates/qday-xmss/src/adrs.rs`

**Interfaces:**
- Produces: `params::{N, W, LEN_1, LEN_2, LEN, H, NUM_PAGES, SIG_BYTES, PK_BYTES, OID}`;
  `adrs::Adrs` with `new_type(u32)`, `set_type`, `set_ots`, `set_ltree`, `set_chain`, `set_tree_height`,
  `set_hash`, `set_tree_index`, `set_key_and_mask`, `to_bytes() -> [u8; 32]`; consts `Adrs::OTS/LTREE/HASH_TREE`.

- [ ] **Step 1: Create the workspace files**

`Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/qday-xmss"]

[profile.test]
opt-level = 3
```

`crates/qday-xmss/Cargo.toml`:
```toml
[package]
name = "qday-xmss"
version = "0.1.0"
edition = "2021"
license = "MIT"
description = "RFC 8391 XMSS-SHA2_10_256 for QDay (no_std)"

[dependencies]
sha2 = { version = "0.10", default-features = false }

[dev-dependencies]
hex = "0.4"
serde_json = "1"
```

`crates/qday-xmss/src/params.rs`:
```rust
//! RFC 8391 XMSS-SHA2_10_256 parameters.
pub const N: usize = 32;
pub const W: u8 = 16;
pub const LEN_1: usize = 64;
pub const LEN_2: usize = 3;
pub const LEN: usize = LEN_1 + LEN_2; // 67
pub const H: usize = 10;
pub const NUM_PAGES: u32 = 1 << H; // 1024
pub const SIG_BYTES: usize = 4 + N + LEN * N + H * N; // 2500
pub const PK_BYTES: usize = 4 + 2 * N; // 68
pub const OID: u32 = 0x0000_0001;
```

`crates/qday-xmss/src/lib.rs` (grows in later tasks):
```rust
#![cfg_attr(not(test), no_std)]
extern crate alloc;

pub mod adrs;
pub mod params;
```

- [ ] **Step 2: Write the failing ADRS tests** in `crates/qday-xmss/src/adrs.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_tree_layout_is_big_endian_words() {
        let mut a = Adrs::new_type(Adrs::HASH_TREE);
        a.set_tree_height(3);
        a.set_tree_index(5);
        a.set_key_and_mask(1);
        let b = a.to_bytes();
        assert_eq!(&b[12..16], &[0, 0, 0, 2]);
        assert_eq!(&b[20..24], &[0, 0, 0, 3]);
        assert_eq!(&b[24..28], &[0, 0, 0, 5]);
        assert_eq!(&b[28..32], &[0, 0, 0, 1]);
        assert_eq!(&b[0..12], &[0u8; 12]);
    }

    #[test]
    fn set_type_zeroes_following_words() {
        let mut a = Adrs::new_type(Adrs::OTS);
        a.set_ots(7);
        a.set_chain(8);
        a.set_hash(9);
        a.set_key_and_mask(1);
        a.set_type(Adrs::LTREE);
        assert_eq!(a.to_bytes()[16..32], [0u8; 16]);
    }
}
```

- [ ] **Step 3: Run to see it fail**

Run: `cargo test -p qday-xmss adrs`
Expected: compile error, `cannot find type Adrs`.

- [ ] **Step 4: Implement** (top of `adrs.rs`)

```rust
//! 32-byte XMSS hash address (RFC 8391 §2.5). Words: layer, tree(2), type, w4..w7.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Adrs([u32; 8]);

impl Adrs {
    pub const OTS: u32 = 0;
    pub const LTREE: u32 = 1;
    pub const HASH_TREE: u32 = 2;

    pub fn new_type(t: u32) -> Self {
        let mut a = Self::default();
        a.set_type(t);
        a
    }
    /// Sets the type and zeroes words 4..=7, as RFC 8391 requires.
    pub fn set_type(&mut self, t: u32) {
        self.0[3] = t;
        self.0[4..8].fill(0);
    }
    pub fn set_ots(&mut self, v: u32) { self.0[4] = v }
    pub fn set_ltree(&mut self, v: u32) { self.0[4] = v }
    pub fn set_chain(&mut self, v: u32) { self.0[5] = v }
    pub fn set_tree_height(&mut self, v: u32) { self.0[5] = v }
    pub fn set_hash(&mut self, v: u32) { self.0[6] = v }
    pub fn set_tree_index(&mut self, v: u32) { self.0[6] = v }
    pub fn set_key_and_mask(&mut self, v: u32) { self.0[7] = v }

    pub fn to_bytes(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        for (i, w) in self.0.iter().enumerate() {
            out[4 * i..4 * i + 4].copy_from_slice(&w.to_be_bytes());
        }
        out
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p qday-xmss adrs`
Expected: 2 passed.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/qday-xmss
git commit -m "feat(xmss): add RFC 8391 parameters and hash address type"
```

---

### Task 3: Hash primitives

**Files:**
- Create: `crates/qday-xmss/src/hash.rs`
- Modify: `crates/qday-xmss/src/lib.rs` (add `pub mod hash;`)

**Interfaces:**
- Consumes: `Adrs`.
- Produces: `type Node = [u8; 32]`; `f(&Node,&Node)->Node`, `h(&Node,&[u8;64])->Node`,
  `h_msg(r:&Node, root:&Node, idx:u32, m:&[u8])->Node`, `prf(&Node,&[u8;32])->Node`,
  `prf_keygen(sk_seed:&Node, pub_seed:&Node, adrs:&Adrs)->Node`,
  `thash_f(pub_seed:&Node, adrs:&mut Adrs, x:&Node)->Node`,
  `rand_hash(pub_seed:&Node, adrs:&mut Adrs, l:&Node, r:&Node)->Node`.

- [ ] **Step 1: Failing tests** (bottom of `hash.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn f_is_sha256_of_pad0_key_msg() {
        let key = [7u8; 32];
        let m = [9u8; 32];
        let mut buf = [0u8; 96];
        buf[64..].copy_from_slice(&m);
        buf[32..64].copy_from_slice(&key);
        let want: [u8; 32] = Sha256::digest(buf).into();
        assert_eq!(f(&key, &m), want);
    }

    #[test]
    fn h_msg_key_is_r_root_idx32() {
        let (r, root) = ([1u8; 32], [2u8; 32]);
        let mut buf = [0u8; 32 + 96 + 3];
        buf[31] = 2;
        buf[32..64].copy_from_slice(&r);
        buf[64..96].copy_from_slice(&root);
        buf[124..128].copy_from_slice(&5u32.to_be_bytes());
        buf[128..].copy_from_slice(b"abc");
        let want: [u8; 32] = Sha256::digest(buf).into();
        assert_eq!(h_msg(&r, &root, 5, b"abc"), want);
    }

    #[test]
    fn thash_f_uses_km0_key_and_km1_mask() {
        let seed = [3u8; 32];
        let x = [4u8; 32];
        let mut a = Adrs::new_type(Adrs::OTS);
        let mut a0 = a; a0.set_key_and_mask(0);
        let mut a1 = a; a1.set_key_and_mask(1);
        let key = prf(&seed, &a0.to_bytes());
        let bm = prf(&seed, &a1.to_bytes());
        let mut xm = x;
        for i in 0..32 { xm[i] ^= bm[i]; }
        assert_eq!(thash_f(&seed, &mut a, &x), f(&key, &xm));
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p qday-xmss hash`
Expected: compile errors (functions missing).

- [ ] **Step 3: Implement**

```rust
//! RFC 8391 §5.1 hash functions for SHA2 with n = 32.
use crate::adrs::Adrs;
use sha2::{Digest, Sha256};

pub type Node = [u8; 32];

fn core(pad: u8, key: &[u8], m: &[u8]) -> Node {
    let mut p = [0u8; 32];
    p[31] = pad;
    let mut s = Sha256::new();
    s.update(p);
    s.update(key);
    s.update(m);
    s.finalize().into()
}

fn xor(a: &Node, b: &Node) -> Node {
    let mut o = *a;
    for i in 0..32 { o[i] ^= b[i]; }
    o
}

pub fn f(key: &Node, m: &Node) -> Node { core(0, key, m) }
pub fn h(key: &Node, m: &[u8; 64]) -> Node { core(1, key, m) }
pub fn prf(key: &Node, m: &[u8; 32]) -> Node { core(3, key, m) }

/// H_msg(r || root || toByte(idx, 32), M).
pub fn h_msg(r: &Node, root: &Node, idx: u32, m: &[u8]) -> Node {
    let mut key = [0u8; 96];
    key[..32].copy_from_slice(r);
    key[32..64].copy_from_slice(root);
    key[92..96].copy_from_slice(&idx.to_be_bytes());
    core(2, &key, m)
}

/// WOTS+ secret element derivation. Implementation-defined in RFC 8391; mirrors the reference
/// implementation's PRF_keygen(SK_SEED, PUB_SEED || ADRS). Signature *format* is unaffected.
pub fn prf_keygen(sk_seed: &Node, pub_seed: &Node, adrs: &Adrs) -> Node {
    let mut m = [0u8; 64];
    m[..32].copy_from_slice(pub_seed);
    m[32..].copy_from_slice(&adrs.to_bytes());
    core(4, sk_seed, &m)
}

/// One WOTS+ chain step: F(KEY, X xor BM).
pub fn thash_f(pub_seed: &Node, adrs: &mut Adrs, x: &Node) -> Node {
    adrs.set_key_and_mask(0);
    let key = prf(pub_seed, &adrs.to_bytes());
    adrs.set_key_and_mask(1);
    let bm = prf(pub_seed, &adrs.to_bytes());
    f(&key, &xor(x, &bm))
}

/// RAND_HASH (RFC 8391 §4.1.4).
pub fn rand_hash(pub_seed: &Node, adrs: &mut Adrs, l: &Node, r: &Node) -> Node {
    adrs.set_key_and_mask(0);
    let key = prf(pub_seed, &adrs.to_bytes());
    adrs.set_key_and_mask(1);
    let bm0 = prf(pub_seed, &adrs.to_bytes());
    adrs.set_key_and_mask(2);
    let bm1 = prf(pub_seed, &adrs.to_bytes());
    let mut m = [0u8; 64];
    m[..32].copy_from_slice(&xor(l, &bm0));
    m[32..].copy_from_slice(&xor(r, &bm1));
    h(&key, &m)
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p qday-xmss hash`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/qday-xmss
git commit -m "feat(xmss): add SHA2 hash primitives"
```

---

### Task 4: WOTS+

**Files:**
- Create: `crates/qday-xmss/src/wots.rs`
- Modify: `crates/qday-xmss/src/lib.rs` (add `pub mod wots;`)

**Interfaces:**
- Consumes: `hash::{Node, thash_f, prf_keygen}`, `Adrs`, `params::{LEN, LEN_1}`.
- Produces: `msg_digits(&Node) -> [u8; LEN]`;
  `pk_gen(sk_seed, pub_seed, ots_idx:u32) -> [Node; LEN]`;
  `sign(m:&Node, sk_seed, pub_seed, ots_idx) -> [Node; LEN]`;
  `pk_from_sig(sig:&[Node; LEN], m:&Node, pub_seed, ots_idx) -> [Node; LEN]`.

- [ ] **Step 1: Failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_all_zero_message_has_max_checksum() {
        let d = msg_digits(&[0u8; 32]);
        assert!(d[..64].iter().all(|&x| x == 0));
        // csum = 64*15 = 960; 960 << 4 = 15360 = 0x3C00 → digits 3, C, 0
        assert_eq!(&d[64..], &[0x3, 0xC, 0x0]);
    }

    #[test]
    fn digits_split_high_nibble_first() {
        let mut m = [0u8; 32];
        m[0] = 0xAB;
        let d = msg_digits(&m);
        assert_eq!((d[0], d[1]), (0xA, 0xB));
    }

    #[test]
    fn sign_then_pk_from_sig_roundtrips() {
        let (sk, ps) = ([1u8; 32], [2u8; 32]);
        let m = [0x5Au8; 32];
        let sig = sign(&m, &sk, &ps, 3);
        assert_eq!(pk_from_sig(&sig, &m, &ps, 3), pk_gen(&sk, &ps, 3));
        let mut m2 = m; m2[0] ^= 1;
        assert_ne!(pk_from_sig(&sig, &m2, &ps, 3), pk_gen(&sk, &ps, 3));
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p qday-xmss wots`
Expected: compile errors.

- [ ] **Step 3: Implement**

```rust
//! WOTS+ for w = 16 (RFC 8391 §3).
use crate::adrs::Adrs;
use crate::hash::{prf_keygen, thash_f, Node};
use crate::params::{LEN, LEN_1};

fn base_w16(x: &[u8], out: &mut [u8]) {
    for (i, o) in out.iter_mut().enumerate() {
        let b = x[i / 2];
        *o = if i % 2 == 0 { b >> 4 } else { b & 0x0f };
    }
}

/// Message digits plus checksum digits (len_1 = 64, len_2 = 3).
pub fn msg_digits(m: &Node) -> [u8; LEN] {
    let mut d = [0u8; LEN];
    base_w16(m, &mut d[..LEN_1]);
    let csum: u32 = d[..LEN_1].iter().map(|&x| 15 - x as u32).sum();
    let cb = ((csum << 4) as u16).to_be_bytes();
    base_w16(&cb, &mut d[LEN_1..]);
    d
}

fn ots_adrs(ots_idx: u32, chain: u32) -> Adrs {
    let mut a = Adrs::new_type(Adrs::OTS);
    a.set_ots(ots_idx);
    a.set_chain(chain);
    a
}

fn chain(x: Node, start: u8, steps: u8, pub_seed: &Node, adrs: &mut Adrs) -> Node {
    let mut t = x;
    for j in start..start + steps {
        adrs.set_hash(j as u32);
        t = thash_f(pub_seed, adrs, &t);
    }
    t
}

fn sk_element(sk_seed: &Node, pub_seed: &Node, ots_idx: u32, i: u32) -> Node {
    prf_keygen(sk_seed, pub_seed, &ots_adrs(ots_idx, i))
}

pub fn pk_gen(sk_seed: &Node, pub_seed: &Node, ots_idx: u32) -> [Node; LEN] {
    core::array::from_fn(|i| {
        let mut a = ots_adrs(ots_idx, i as u32);
        chain(sk_element(sk_seed, pub_seed, ots_idx, i as u32), 0, 15, pub_seed, &mut a)
    })
}

pub fn sign(m: &Node, sk_seed: &Node, pub_seed: &Node, ots_idx: u32) -> [Node; LEN] {
    let d = msg_digits(m);
    core::array::from_fn(|i| {
        let mut a = ots_adrs(ots_idx, i as u32);
        chain(sk_element(sk_seed, pub_seed, ots_idx, i as u32), 0, d[i], pub_seed, &mut a)
    })
}

pub fn pk_from_sig(sig: &[Node; LEN], m: &Node, pub_seed: &Node, ots_idx: u32) -> [Node; LEN] {
    let d = msg_digits(m);
    core::array::from_fn(|i| {
        let mut a = ots_adrs(ots_idx, i as u32);
        chain(sig[i], d[i], 15 - d[i], pub_seed, &mut a)
    })
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p qday-xmss wots`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/qday-xmss
git commit -m "feat(xmss): add WOTS+ signing and public key recovery"
```

---

### Task 5: L-tree, key tree, XMSS sign and verify

**Files:**
- Create: `crates/qday-xmss/src/tree.rs`
- Modify: `crates/qday-xmss/src/lib.rs`

**Interfaces:**
- Consumes: Tasks 2–4.
- Produces (in `lib.rs`): `SecretKey { sk_seed, sk_prf, pub_seed: Node }`,
  `PublicKey { root, pub_seed: Node }` with `to_bytes() -> [u8; PK_BYTES]`,
  `Signature { idx: u32, r: Node, ots: [Node; LEN], auth: [Node; H] }` with
  `to_bytes() -> Vec<u8>` (2,500 B) and `from_bytes(&[u8]) -> Option<Signature>`,
  `KeyTree(Vec<Node>)` (2,047 nodes) with `build(&SecretKey) -> KeyTree`, `root() -> Node`,
  `public_key(&self, &SecretKey) -> PublicKey`,
  `sign(sk:&SecretKey, tree:&KeyTree, idx:u32, msg:&[u8]) -> Signature`,
  `verify(pk:&PublicKey, msg:&[u8], sig:&Signature) -> bool`.

- [ ] **Step 1: Failing tests** (`crates/qday-xmss/tests/roundtrip.rs`)

```rust
use qday_xmss::*;

fn key() -> (SecretKey, KeyTree) {
    let sk = SecretKey { sk_seed: [1; 32], sk_prf: [2; 32], pub_seed: [3; 32] };
    let tree = KeyTree::build(&sk);
    (sk, tree)
}

#[test]
fn sign_verify_first_middle_last_pages() {
    let (sk, tree) = key();
    let pk = tree.public_key(&sk);
    for idx in [0u32, 1, 513, 1023] {
        let sig = sign(&sk, &tree, idx, b"send 0.5 ETH to alice");
        assert_eq!(sig.idx, idx);
        assert!(verify(&pk, b"send 0.5 ETH to alice", &sig), "idx {idx}");
    }
}

#[test]
fn rejects_tampering() {
    let (sk, tree) = key();
    let pk = tree.public_key(&sk);
    let sig = sign(&sk, &tree, 7, b"m");
    assert!(!verify(&pk, b"n", &sig));
    let mut bytes = sig.to_bytes();
    for pos in [3usize, 4, 36, 36 + 67 * 32, 2499] {
        let mut b = bytes.clone();
        b[pos] ^= 1;
        let s = Signature::from_bytes(&b).unwrap();
        assert!(!verify(&pk, b"m", &s), "flip at {pos}");
    }
    bytes[0] = 0x04; // idx >= 1024
    assert!(Signature::from_bytes(&bytes).is_none());
}

#[test]
fn encoding_sizes() {
    let (sk, tree) = key();
    assert_eq!(sign(&sk, &tree, 0, b"x").to_bytes().len(), params::SIG_BYTES);
    assert_eq!(tree.public_key(&sk).to_bytes()[..4], [0, 0, 0, 1]);
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p qday-xmss --test roundtrip`
Expected: compile errors.

- [ ] **Step 3: Implement `tree.rs`**

```rust
//! L-tree and the full h = 10 hash tree, stored level by level (2,047 nodes).
use crate::adrs::Adrs;
use crate::hash::{rand_hash, Node};
use crate::params::{H, LEN, NUM_PAGES};
use crate::wots;
use alloc::vec;
use alloc::vec::Vec;

pub const TREE_NODES: usize = (1 << (H + 1)) - 1;

/// Index of the first node at height `k` (leaves are height 0).
pub fn level_offset(k: usize) -> usize {
    (1 << (H + 1)) - (1 << (H + 1 - k))
}

pub fn ltree(pk: &mut [Node; LEN], pub_seed: &Node, ltree_idx: u32) -> Node {
    let mut a = Adrs::new_type(Adrs::LTREE);
    a.set_ltree(ltree_idx);
    let (mut len, mut height) = (LEN, 0u32);
    while len > 1 {
        a.set_tree_height(height);
        for i in 0..len / 2 {
            a.set_tree_index(i as u32);
            pk[i] = rand_hash(pub_seed, &mut a, &pk[2 * i], &pk[2 * i + 1]);
        }
        if len % 2 == 1 {
            pk[len / 2] = pk[len - 1];
        }
        len = (len + 1) / 2;
        height += 1;
    }
    pk[0]
}

pub fn build_nodes(sk_seed: &Node, pub_seed: &Node) -> Vec<Node> {
    let mut nodes = vec![[0u8; 32]; TREE_NODES];
    for i in 0..NUM_PAGES {
        let mut pk = wots::pk_gen(sk_seed, pub_seed, i);
        nodes[i as usize] = ltree(&mut pk, pub_seed, i);
    }
    for k in 0..H {
        for p in 0..(1usize << (H - k - 1)) {
            let mut a = Adrs::new_type(Adrs::HASH_TREE);
            a.set_tree_height(k as u32);
            a.set_tree_index(p as u32);
            let (l, r) = (nodes[level_offset(k) + 2 * p], nodes[level_offset(k) + 2 * p + 1]);
            nodes[level_offset(k + 1) + p] = rand_hash(pub_seed, &mut a, &l, &r);
        }
    }
    nodes
}

/// Recompute the root from a leaf and its authentication path (RFC 8391 Alg. 13, tree part).
pub fn root_from_leaf(leaf: Node, idx: u32, auth: &[Node; H], pub_seed: &Node) -> Node {
    let mut a = Adrs::new_type(Adrs::HASH_TREE);
    let mut node = leaf;
    for k in 0..H {
        a.set_tree_height(k as u32);
        a.set_tree_index(idx >> (k + 1));
        node = if (idx >> k) & 1 == 0 {
            rand_hash(pub_seed, &mut a, &node, &auth[k])
        } else {
            rand_hash(pub_seed, &mut a, &auth[k], &node)
        };
    }
    node
}
```

- [ ] **Step 4: Implement the API in `lib.rs`**

```rust
#![cfg_attr(not(test), no_std)]
extern crate alloc;

pub mod adrs;
pub mod hash;
pub mod params;
pub mod tree;
pub mod wots;

use alloc::vec::Vec;
use hash::{h_msg, prf, Node};
use params::*;

pub struct SecretKey { pub sk_seed: Node, pub sk_prf: Node, pub pub_seed: Node }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicKey { pub root: Node, pub pub_seed: Node }

impl PublicKey {
    pub fn to_bytes(&self) -> [u8; PK_BYTES] {
        let mut o = [0u8; PK_BYTES];
        o[..4].copy_from_slice(&OID.to_be_bytes());
        o[4..36].copy_from_slice(&self.root);
        o[36..].copy_from_slice(&self.pub_seed);
        o
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signature { pub idx: u32, pub r: Node, pub ots: [Node; LEN], pub auth: [Node; H] }

impl Signature {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut o = Vec::with_capacity(SIG_BYTES);
        o.extend_from_slice(&self.idx.to_be_bytes());
        o.extend_from_slice(&self.r);
        self.ots.iter().for_each(|n| o.extend_from_slice(n));
        self.auth.iter().for_each(|n| o.extend_from_slice(n));
        o
    }
    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.len() != SIG_BYTES { return None; }
        let idx = u32::from_be_bytes(b[..4].try_into().ok()?);
        if idx >= NUM_PAGES { return None; }
        let node = |off: usize| -> Node { b[off..off + N].try_into().unwrap() };
        Some(Self {
            idx,
            r: node(4),
            ots: core::array::from_fn(|i| node(36 + i * N)),
            auth: core::array::from_fn(|k| node(36 + LEN * N + k * N)),
        })
    }
}

pub struct KeyTree(pub Vec<Node>);

impl KeyTree {
    pub fn build(sk: &SecretKey) -> Self { Self(tree::build_nodes(&sk.sk_seed, &sk.pub_seed)) }
    pub fn root(&self) -> Node { self.0[tree::level_offset(H)] }
    pub fn public_key(&self, sk: &SecretKey) -> PublicKey {
        PublicKey { root: self.root(), pub_seed: sk.pub_seed }
    }
}

fn idx32(idx: u32) -> [u8; 32] {
    let mut o = [0u8; 32];
    o[28..].copy_from_slice(&idx.to_be_bytes());
    o
}

/// Sign `msg` with page `idx`. **Stateless:** the caller MUST guarantee that `idx` was never used
/// before for a different message. Reusing a page lets an attacker forge. QDay enforces this with
/// the device counter and the on-chain `nextIndex`.
pub fn sign(sk: &SecretKey, tree: &KeyTree, idx: u32, msg: &[u8]) -> Signature {
    assert!(idx < NUM_PAGES);
    let r = prf(&sk.sk_prf, &idx32(idx));
    let mp = h_msg(&r, &tree.root(), idx, msg);
    let ots = wots::sign(&mp, &sk.sk_seed, &sk.pub_seed, idx);
    let auth = core::array::from_fn(|k| tree.0[tree::level_offset(k) + (((idx >> k) ^ 1) as usize)]);
    Signature { idx, r, ots, auth }
}

pub fn verify(pk: &PublicKey, msg: &[u8], sig: &Signature) -> bool {
    if sig.idx >= NUM_PAGES { return false; }
    let mp = h_msg(&sig.r, &pk.root, sig.idx, msg);
    let mut wpk = wots::pk_from_sig(&sig.ots, &mp, &pk.pub_seed, sig.idx);
    let leaf = tree::ltree(&mut wpk, &pk.pub_seed, sig.idx);
    tree::root_from_leaf(leaf, sig.idx, &sig.auth, &pk.pub_seed) == pk.root
}
```

- [ ] **Step 5: Run all crate tests**

Run: `cargo test -p qday-xmss`
Expected: all pass (tree build takes a few seconds at opt-level 3).

- [ ] **Step 6: Commit**

```bash
git add crates/qday-xmss
git commit -m "feat(xmss): add key tree, signing and verification"
```

---

### Task 6: Conformance with the RFC 8391 reference implementation

This is the task that proves the scheme is the standard one.

**Files:**
- Create: `tools/xmss-ref/driver.c`, `tools/xmss-ref/build.sh`, `vectors/rfc8391/ref_sigs.json`,
  `crates/qday-xmss/tests/rfc_reference.rs`, `crates/qday-xmss/examples/sign_hex.rs`
- Modify: `.gitignore` (add `tools/xmss-ref/xmss-reference/`, `tools/xmss-ref/xmss-ref`)

**Interfaces:**
- Consumes: `verify`, `Signature::from_bytes`, `sign`, `KeyTree`.
- Produces: `vectors/rfc8391/ref_sigs.json` = `{"oid":1,"pk":"0x…68B","cases":[{"msg":"0x…","sig":"0x…2500B"}]}`;
  `tools/xmss-ref/xmss-ref gen` and `tools/xmss-ref/xmss-ref verify <pk> <msg> <sig>` (exit 0 = valid).

- [ ] **Step 1: Write the driver** `tools/xmss-ref/driver.c`

```c
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "xmss.h"
#include "params.h"

static void hex(const unsigned char *b, size_t n) { printf("0x"); for (size_t i = 0; i < n; i++) printf("%02x", b[i]); }
static size_t unhex(const char *s, unsigned char *out) {
    if (s[0] == '0' && s[1] == 'x') s += 2;
    size_t n = strlen(s) / 2;
    for (size_t i = 0; i < n; i++) sscanf(s + 2 * i, "%2hhx", &out[i]);
    return n;
}

int main(int argc, char **argv) {
    uint32_t oid;
    xmss_params p; xmss_str_to_oid(&oid, "XMSS-SHA2_10_256"); xmss_parse_oid(&p, oid);
    if (argc >= 2 && strcmp(argv[1], "gen") == 0) {
        unsigned char pk[XMSS_OID_LEN + p.pk_bytes], sk[XMSS_OID_LEN + p.sk_bytes];
        xmss_keypair(pk, sk, oid);
        printf("{\"oid\":%u,\"pk\":\"", oid); hex(pk, sizeof pk); printf("\",\"cases\":[");
        for (int c = 0; c < 4; c++) {
            unsigned char m[32]; memset(m, 0x10 + c, 32);
            unsigned char sm[p.sig_bytes + 32]; unsigned long long smlen;
            xmss_sign(sk, sm, &smlen, m, 32);
            printf("%s{\"msg\":\"", c ? "," : ""); hex(m, 32);
            printf("\",\"sig\":\""); hex(sm, p.sig_bytes); printf("\"}");
        }
        printf("]}\n");
        return 0;
    }
    if (argc == 5 && strcmp(argv[1], "verify") == 0) {
        unsigned char pk[4096], m[4096], sm[8192], mout[8192];
        unhex(argv[2], pk);
        size_t ml = unhex(argv[3], m);
        size_t sl = unhex(argv[4], sm);
        memcpy(sm + sl, m, ml);
        unsigned long long mlen;
        return xmss_sign_open(mout, &mlen, sm, sl + ml, pk) == 0 ? 0 : 1;
    }
    fprintf(stderr, "usage: xmss-ref gen | verify <pk> <msg> <sig>\n");
    return 2;
}
```

- [ ] **Step 2: Write the build script** `tools/xmss-ref/build.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
[ -d xmss-reference ] || git clone --depth 1 https://github.com/XMSS/xmss-reference.git
SSL=$(brew --prefix openssl 2>/dev/null || echo /usr)
cd xmss-reference
cc -O2 -I. -I"$SSL/include" -L"$SSL/lib" \
  params.c hash.c fips202.c hash_address.c randombytes.c wots.c xmss.c xmss_core.c xmss_commons.c utils.c \
  ../driver.c -lcrypto -o ../xmss-ref
echo "built tools/xmss-ref/xmss-ref"
```

If linking fails, compare the source list against `SOURCES` in the reference `Makefile` and use theirs.

- [ ] **Step 3: Build and generate the reference vectors**

```bash
chmod +x tools/xmss-ref/build.sh && tools/xmss-ref/build.sh
mkdir -p vectors/rfc8391
tools/xmss-ref/xmss-ref gen > vectors/rfc8391/ref_sigs.json
```

Expected: JSON with a 68-byte `pk` and 4 cases, each with a 2,500-byte `sig`.

- [ ] **Step 4: Write the test that reference signatures verify in Rust** (`tests/rfc_reference.rs`)

```rust
use qday_xmss::*;

fn unhex(s: &str) -> Vec<u8> { hex::decode(s.trim_start_matches("0x")).unwrap() }

#[test]
fn reference_signatures_verify() {
    let j: serde_json::Value =
        serde_json::from_str(include_str!("../../../vectors/rfc8391/ref_sigs.json")).unwrap();
    let pkb = unhex(j["pk"].as_str().unwrap());
    assert_eq!(&pkb[..4], &[0, 0, 0, 1]);
    let pk = PublicKey { root: pkb[4..36].try_into().unwrap(), pub_seed: pkb[36..68].try_into().unwrap() };
    for (i, c) in j["cases"].as_array().unwrap().iter().enumerate() {
        let msg = unhex(c["msg"].as_str().unwrap());
        let sig = Signature::from_bytes(&unhex(c["sig"].as_str().unwrap())).unwrap();
        assert_eq!(sig.idx, i as u32);
        assert!(verify(&pk, &msg, &sig), "case {i}");
        assert!(!verify(&pk, b"other", &sig));
    }
}
```

Run: `cargo test -p qday-xmss --test rfc_reference`
Expected: PASS. If it fails, the bug is in our verify path (hash padding, ADRS layout, checksum shift),
not in key generation. Debug with superpowers:systematic-debugging.

- [ ] **Step 5: The other direction: our signatures verify in the reference**

`crates/qday-xmss/examples/sign_hex.rs`:
```rust
use qday_xmss::*;
fn main() {
    let sk = SecretKey { sk_seed: [1; 32], sk_prf: [2; 32], pub_seed: [3; 32] };
    let tree = KeyTree::build(&sk);
    let msg = [0x42u8; 32];
    let sig = sign(&sk, &tree, 513, &msg);
    println!("0x{} 0x{} 0x{}", hex::encode(tree.public_key(&sk).to_bytes()), hex::encode(msg), hex::encode(sig.to_bytes()));
}
```

Run:
```bash
tools/xmss-ref/xmss-ref verify $(cargo run -q --release -p qday-xmss --example sign_hex); echo "exit=$?"
```
Expected: `exit=0`.

- [ ] **Step 6: Commit**

```bash
git add tools/xmss-ref/driver.c tools/xmss-ref/build.sh vectors/rfc8391 crates/qday-xmss .gitignore
git commit -m "test(xmss): verify interop with the RFC 8391 reference implementation"
```

---

### Task 7: Shared QDay vectors

**Files:**
- Create: `crates/qday-xmss/examples/gen_vectors.rs`, `vectors/qday/basic.json`

**Interfaces:**
- Produces: `vectors/qday/basic.json` =
  `{"cases":[{"idx":u32,"root":"0x…32","pub_seed":"0x…32","msg":"0x…32","sig":"0x…2500"}]}` for
  idx 0, 1, 513, 1023 with `msg = sha256("qday-vector-<idx>")`. Consumed by the Foundry tests and,
  later, the firmware tests.

- [ ] **Step 1: Write the generator**

```rust
use qday_xmss::*;
use sha2::{Digest, Sha256};

fn main() {
    let sk = SecretKey { sk_seed: [1; 32], sk_prf: [2; 32], pub_seed: [3; 32] };
    let tree = KeyTree::build(&sk);
    let pk = tree.public_key(&sk);
    let cases: Vec<_> = [0u32, 1, 513, 1023].iter().map(|&idx| {
        let msg: [u8; 32] = Sha256::digest(format!("qday-vector-{idx}")).into();
        let sig = sign(&sk, &tree, idx, &msg);
        assert!(verify(&pk, &msg, &sig));
        serde_json::json!({
            "idx": idx,
            "root": format!("0x{}", hex::encode(pk.root)),
            "pub_seed": format!("0x{}", hex::encode(pk.pub_seed)),
            "msg": format!("0x{}", hex::encode(msg)),
            "sig": format!("0x{}", hex::encode(sig.to_bytes())),
        })
    }).collect();
    println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "cases": cases })).unwrap());
}
```

- [ ] **Step 2: Generate the vectors and check them**

```bash
mkdir -p vectors/qday
cargo run -q --release -p qday-xmss --example gen_vectors > vectors/qday/basic.json
python3 -c "import json;d=json.load(open('vectors/qday/basic.json'));print([ (c['idx'],len(c['sig'])) for c in d['cases']])"
```
Expected: `[(0, 5002), (1, 5002), (513, 5002), (1023, 5002)]` (2 + 2×2500 hex chars).

- [ ] **Step 3: Commit**

```bash
git add crates/qday-xmss vectors/qday
git commit -m "test(xmss): add shared signature test vectors"
```

---

### Task 8: Yul XMSS verifier and gas measurement (feeds the OD1 gate)

**Files:**
- Create: `contracts/foundry.toml`, `contracts/yul/XmssVerifierHarness.yul`,
  `contracts/scripts/build-yul.sh`, `contracts/test/XmssVerifier.t.sol`
- Modify: `.gitignore` (add `contracts/out/`, `contracts/cache/`, `contracts/out-yul/`, `contracts/lib/`)

**Interfaces:**
- Consumes: `vectors/qday/basic.json`.
- Produces: harness calldata ABI = `root(32) ‖ pub_seed(32) ‖ msg(32) ‖ sig(2500)` → returns a 32-byte
  word `1` (valid) or `0`. The Yul functions `xmss_verify(root, seed, msg, sigOff) -> ok` and helpers
  get copied unchanged into `QDayAccount` by the next plan.

- [ ] **Step 1: Set up Foundry**

```bash
forge init contracts --no-git
rm -f contracts/src/Counter.sol contracts/test/Counter.t.sol contracts/script/Counter.s.sol
```

`contracts/foundry.toml`:
```toml
[profile.default]
src = "src"
test = "test"
out = "out"
libs = ["lib"]
evm_version = "prague"
fs_permissions = [{ access = "read", path = "../vectors" }, { access = "read", path = "./out-yul" }]
```

- [ ] **Step 2: Write the failing Foundry test** `contracts/test/XmssVerifier.t.sol`

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console} from "forge-std/Test.sol";

contract XmssVerifierTest is Test {
    address verifier;
    string json;

    function setUp() public {
        bytes memory code = vm.parseBytes(vm.readFile("out-yul/XmssVerifierHarness.bin"));
        address a;
        assembly { a := create(0, add(code, 0x20), mload(code)) }
        require(a != address(0), "deploy failed");
        verifier = a;
        json = vm.readFile("../vectors/qday/basic.json");
    }

    function _case(uint256 i) internal view returns (bytes32 root, bytes32 seed, bytes32 m, bytes memory sig) {
        string memory p = string.concat(".cases[", vm.toString(i), "]");
        root = vm.parseJsonBytes32(json, string.concat(p, ".root"));
        seed = vm.parseJsonBytes32(json, string.concat(p, ".pub_seed"));
        m = vm.parseJsonBytes32(json, string.concat(p, ".msg"));
        sig = vm.parseJsonBytes(json, string.concat(p, ".sig"));
    }

    function _verify(bytes32 root, bytes32 seed, bytes32 m, bytes memory sig) internal view returns (bool ok, uint256 used) {
        bytes memory input = abi.encodePacked(root, seed, m, sig);
        uint256 g = gasleft();
        (bool s, bytes memory out) = verifier.staticcall(input);
        used = g - gasleft();
        ok = s && out.length == 32 && abi.decode(out, (uint256)) == 1;
    }

    function test_validVectors() public view {
        for (uint256 i = 0; i < 4; i++) {
            (bytes32 root, bytes32 seed, bytes32 m, bytes memory sig) = _case(i);
            (bool ok, uint256 used) = _verify(root, seed, m, sig);
            assertTrue(ok, "valid vector rejected");
            console.log("case", i, "verify gas", used);
        }
    }

    function test_rejectsWrongMessage() public view {
        (bytes32 root, bytes32 seed, bytes32 m, bytes memory sig) = _case(1);
        (bool ok,) = _verify(root, seed, m ^ bytes32(uint256(1)), sig);
        assertFalse(ok);
    }

    function test_rejectsFlippedBytes() public view {
        (bytes32 root, bytes32 seed, bytes32 m, bytes memory sig) = _case(2);
        uint256[5] memory pos = [uint256(3), 4, 36, 36 + 67 * 32, 2499];
        for (uint256 k = 0; k < pos.length; k++) {
            bytes memory s = bytes.concat(sig);
            s[pos[k]] = bytes1(uint8(s[pos[k]]) ^ 1);
            (bool ok,) = _verify(root, seed, m, s);
            assertFalse(ok);
        }
    }

    function test_rejectsIdxOutOfRange() public view {
        (bytes32 root, bytes32 seed, bytes32 m, bytes memory sig) = _case(0);
        sig[2] = 0x04; // idx = 1024
        (bool ok,) = _verify(root, seed, m, sig);
        assertFalse(ok);
    }
}
```

Run: `cd contracts && forge test` → Expected: FAIL in `setUp` (missing `out-yul/XmssVerifierHarness.bin`).

- [ ] **Step 3: Write the Yul harness** `contracts/yul/XmssVerifierHarness.yul`

```yul
// RFC 8391 XMSS-SHA2_10_256 verifier. No EIP-8141 opcodes: copy the functions unchanged into QDayAccount.
// Memory: 0x00..0xa0 hash input buffer, 0xc0 hash output, 0x100.. WOTS pk / L-tree (67 words).
object "XmssVerifierHarness" {
    code {
        datacopy(0, dataoffset("runtime"), datasize("runtime"))
        return(0, datasize("runtime"))
    }
    object "runtime" {
        code {
            if iszero(eq(calldatasize(), 2596)) { revert(0, 0) }
            let ok := xmss_verify(calldataload(0), calldataload(32), calldataload(64), 96)
            mstore(0, ok)
            return(0, 32)

            function sha(len) -> out {
                if iszero(staticcall(gas(), 0x02, 0x00, len, 0xc0, 0x20)) { revert(0, 0) }
                out := mload(0xc0)
            }
            // ADRS word as uint256: layer=0, tree=0, type<<128, w4<<96, w5<<64, w6<<32, km
            function adrs(t, w4, w5, w6, km) -> a {
                a := or(shl(128, t), or(shl(96, w4), or(shl(64, w5), or(shl(32, w6), km))))
            }
            function prf(seed, a) -> out {
                mstore(0x00, 3) mstore(0x20, seed) mstore(0x40, a)
                out := sha(0x60)
            }
            // F step: a has km = 0
            function thash_f(seed, a, x) -> out {
                let key := prf(seed, a)
                let bm := prf(seed, or(a, 1))
                mstore(0x00, 0) mstore(0x20, key) mstore(0x40, xor(x, bm))
                out := sha(0x60)
            }
            // RAND_HASH: a has km = 0
            function rand_hash(seed, a, l, r) -> out {
                let key := prf(seed, a)
                let bm0 := prf(seed, or(a, 1))
                let bm1 := prf(seed, or(a, 2))
                mstore(0x00, 1) mstore(0x20, key) mstore(0x40, xor(l, bm0)) mstore(0x60, xor(r, bm1))
                out := sha(0x80)
            }
            function h_msg(r, root, idx, m) -> out {
                mstore(0x00, 2) mstore(0x20, r) mstore(0x40, root) mstore(0x60, idx) mstore(0x80, m)
                out := sha(0xa0)
            }
            // digit i of the 67 base-16 digits (64 message nibbles + 3 checksum nibbles)
            function digit(mp, csum, i) -> d {
                switch lt(i, 64)
                case 1 {
                    let b := byte(shr(1, i), mp)
                    switch and(i, 1) case 0 { d := shr(4, b) } default { d := and(b, 0x0f) }
                }
                default { d := and(shr(sub(12, mul(4, sub(i, 64))), shl(4, csum)), 0x0f) }
            }
            function xmss_verify(root, seed, m, sigOff) -> ok {
                let idx := shr(224, calldataload(sigOff))
                if iszero(lt(idx, 1024)) { leave }
                let mp := h_msg(calldataload(add(sigOff, 4)), root, idx, m)

                let csum := 0
                for { let i := 0 } lt(i, 64) { i := add(i, 1) } { csum := add(csum, sub(15, digit(mp, 0, i))) }

                // WOTS+ public key → memory 0x100 + 32*i
                for { let i := 0 } lt(i, 67) { i := add(i, 1) } {
                    let x := calldataload(add(sigOff, add(36, mul(32, i))))
                    for { let j := digit(mp, csum, i) } lt(j, 15) { j := add(j, 1) } {
                        x := thash_f(seed, adrs(0, idx, i, j, 0), x)
                    }
                    mstore(add(0x100, mul(32, i)), x)
                }

                // L-tree
                let len := 67
                for { let h := 0 } gt(len, 1) { h := add(h, 1) } {
                    let half := shr(1, len)
                    for { let i := 0 } lt(i, half) { i := add(i, 1) } {
                        let l := mload(add(0x100, mul(64, i)))
                        let r := mload(add(0x120, mul(64, i)))
                        mstore(add(0x100, mul(32, i)), rand_hash(seed, adrs(1, idx, h, i, 0), l, r))
                    }
                    if and(len, 1) { mstore(add(0x100, mul(32, half)), mload(add(0x100, mul(32, sub(len, 1))))) }
                    len := shr(1, add(len, 1))
                }
                let node := mload(0x100)

                // Auth path
                let authOff := add(sigOff, 2180) // 4 + 32 + 67*32
                for { let k := 0 } lt(k, 10) { k := add(k, 1) } {
                    let auth := calldataload(add(authOff, mul(32, k)))
                    let a := adrs(2, 0, k, shr(add(k, 1), idx), 0)
                    switch and(shr(k, idx), 1)
                    case 0 { node := rand_hash(seed, a, node, auth) }
                    default { node := rand_hash(seed, a, auth, node) }
                }
                ok := eq(node, root)
            }
        }
    }
}
```

- [ ] **Step 4: Write the build script** `contracts/scripts/build-yul.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p out-yul
for f in yul/*.yul; do
  name=$(basename "$f" .yul)
  solc --strict-assembly --optimize --evm-version prague --bin "$f" \
    | awk '/Binary representation:/{getline; printf "0x%s", $0}' > "out-yul/$name.bin"
  echo "out-yul/$name.bin: $(($(wc -c < "out-yul/$name.bin") / 2 - 1)) bytes"
done
```

- [ ] **Step 5: Build and run the tests**

```bash
chmod +x contracts/scripts/build-yul.sh && contracts/scripts/build-yul.sh
cd contracts && forge test -vv
```
Expected: 4 tests pass, with `verify gas` logged for each case. If valid vectors are rejected, check
in this order: digit/checksum (`digit()` for i ≥ 64), the ADRS word positions, the L-tree odd-length
carry, the auth tree index `idx >> (k+1)`.

- [ ] **Step 6: Commit**

```bash
git add contracts .gitignore
git commit -m "feat(contracts): add Yul XMSS signature verifier"
```

---

### Task 9: Gas report and OD1 recommendation

**Files:**
- Create: `docs/gas.md`

- [ ] **Step 1: Collect the numbers**

```bash
cd contracts && forge test -vv --match-test test_validVectors | grep "verify gas"
```

Calldata cost of the signature: 2,500 B × 16 = 40,000 gas (EIP-8141 empty-`msg` pricing).

- [ ] **Step 2: Write `docs/gas.md`** with this table filled in from the actual output:

```markdown
| Item | Gas | Source |
|------|-----|--------|
| XMSS-SHA2_10_256 verify (Yul), avg of 4 vectors | <measured> | forge test, <date>, solc <version> |
| Signature calldata (2,500 B) | 40,000 | EIP-8141 pricing |
| commit(idx) frame | <from devnet probe> | docs/devnet.md |
| Prefix budget (MAX_VERIFY_GAS) | 100,000 | EIP-8141 |
| **Fits native VERIFY?** | yes / no | |
| Falcon-512 Solidity (reference) | ~1.8M | ETHFALCON |
```

Then add a recommendation paragraph that picks option (a) or (b) from spec OD1, giving the
measured number as the reason.

- [ ] **Step 3: Commit, then bring the decision to the team at the Fri 04:00 gate**

```bash
git add docs/gas.md
git commit -m "docs: add XMSS verification gas measurements"
```
