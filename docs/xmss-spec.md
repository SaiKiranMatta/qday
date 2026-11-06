# QDay signature scheme

The byte-exact definition of the XMSS-style scheme in `crates/qday-xmss`. The Solidity verifier and
the device must both match it.

This is **not** RFC 8391. It has the same shape and the same 2,500-byte signature, but it hashes with
keccak256 and uses its own addresses and domain separation, so RFC 8391 test vectors and tools don't
apply.

## Parameters

| Name | Value |
|---|---|
| H | keccak256 |
| n | 32 bytes |
| w | 16 |
| Chains | 67: 64 message digits and 3 checksum digits |
| Tree height | 10 |
| One-time keys per tree | 1,024 |
| Tree nodes | 2,047, about 64 KB |
| Signature | 2,500 bytes |

Integers are big-endian. `u32(x)` is 4 bytes and `u256(x)` is 32 bytes. `||` joins byte strings.

## Addresses

An ADRS is 32 bytes, all zero except for byte 0 (the type) and these three big-endian `u32` fields:

| Bytes | Field |
|---|---|
| 20-23 | a |
| 24-27 | b |
| 28-31 | c |

| Type | Byte 0 | a | b | c |
|---|---|---|---|---|
| CHAIN | 0 | leaf | chain | step |
| LEAF | 1 | leaf | 0 | 0 |
| NODE | 2 | height | node index | 0 |
| SECRET | 3 | leaf | chain | 0 |

## Keys

From a 32-byte seed and a `u32` tree number:

```text
sk_seed  = H(0x00 || seed || u32(tree))
pub_seed = H(0x01 || seed || u32(tree))
prf_key  = H(0x02 || seed || u32(tree))
```

The public key is `root || pub_seed`.

For one-time key `j` and chain `i`:

```text
x_0     = H(sk_seed || ADRS(SECRET, j, i, 0))
x_(k+1) = H(pub_seed || ADRS(CHAIN, j, i, k) || x_k)        for k = 0..14
leaf_j  = H(pub_seed || ADRS(LEAF, j, 0, 0) || x_15 of chains 0..66)
```

Leaves are height 0, and the root is `node(10, 0)`:

```text
node(z, q) = H(pub_seed || ADRS(NODE, z, q, 0) || node(z-1, 2q) || node(z-1, 2q+1))
```

## Signing

To sign a 32-byte message `M` with key `idx`:

```text
r      = H(prf_key || u256(idx))
digest = H(r || root || u256(idx) || M)
```

Each digest byte gives two digits, high nibble first, as `d_0..d_63`. The checksum is:

```text
C    = sum of (15 - d_i) for i in 0..63
d_64 = (C >> 8) & 15
d_65 = (C >> 4) & 15
d_66 = C & 15
```

Then:

- `sig_i` is `x_0` of chain `i`, hashed `d_i` times starting at step 0.
- `auth_z` is the node at height `z`, index `(idx >> z) XOR 1`, for `z = 0..9`.

The encoding is:

| Offset | Bytes | Field |
|---|---|---|
| 0 | 4 | `u32(idx)` |
| 4 | 32 | `r` |
| 36 | 2,144 | `sig_0..sig_66` |
| 2,180 | 320 | `auth_0..auth_9` |

## Verifying

1. Reject anything that isn't exactly 2,500 bytes, and reject `idx >= 1024`.
2. Recompute the digest and the 67 digits.
3. Finish each chain from step `d_i` for `15 - d_i` steps.
4. Hash the 67 chain ends into a leaf.
5. For `z = 0..9`, with `q = idx >> (z+1)`:
   - If bit `z` of `idx` is 0: `node = H(pub_seed || ADRS(NODE, z+1, q, 0) || node || auth_z)`.
   - Otherwise, swap `node` and `auth_z` in that hash.
6. Accept if `node == root`.

## Index safety

Each one-time key may sign only one message. Signing two different messages with one key lets an
attacker forge. Signing is deterministic, so signing the same message twice is harmless.

The crate never picks an index and keeps no state: callers pass the index in. Keeping indexes unique
is the job of the device counter and the account's on-chain `nextIndex`.

## Test vectors

The seed is `0x5eed` repeated 16 times.

| Value | Expected |
|---|---|
| Tree 0 root | `0xbfefd6eccd30b7f3b763b5289150e2ee0124555cd5ac7f14601005179d1273e9` |
| Tree 0 pub_seed | `0x78e135a918cf039e569ea83fdefdb29ddd1985b7b33a9867da1edd9026adceb8` |
| Tree 1 root | `0x18479eb13d693cdb0474c6de337a4544b3a93939bc20476ac23364491c04e881` |
| Tree 1 pub_seed | `0x8c587f5c22ec4115ef0688f0cb254e8fe084ab4ce56416257d8494bddd56a754` |
| keccak256 of the signature with tree 0, index 5, message `0x11` x 32 | `0x8937def506577209cdaaff87612f54af2e417d5502132267f358b5daa32ece1f` |

Run them with:

```bash
cargo test -p qday-xmss
```
