//! Writes the real signatures the Solidity tests check. Run from the repository root:
//! `cargo run -p qday-xmss --example contract_fixtures > contracts/test/fixtures/signatures.json`

use std::fmt::Write;

use qday_xmss::{Hash, KeySet, PublicKey, SIG_BYTES, Tree, keccak256};

/// The transfers and rotations these fixtures sign, as stand-ins for real transaction hashes.
const HASH_A: Hash = [0xAA; 32];
const HASH_B: Hash = [0xBB; 32];
/// Signatures used to measure average verifier gas.
const GAS_SAMPLES: u32 = 32;

struct Signer {
    keys: KeySet,
    tree: Tree,
}

impl Signer {
    fn new(seed: &Hash, number: u32) -> Self {
        let keys = KeySet::derive(seed, number);
        let tree = Tree::build(&keys, |_| {});
        Self { keys, tree }
    }

    fn public_key(&self) -> PublicKey {
        PublicKey {
            root: self.tree.root(),
            pub_seed: *self.keys.pub_seed(),
        }
    }

    fn sign(&self, index: u32, message: &Hash) -> [u8; SIG_BYTES] {
        self.keys
            .sign(&self.tree.root(), index, message, &self.tree.auth_path(index))
            .expect("every fixture index is in range")
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(2 + bytes.len() * 2);
    out.push_str("0x");
    for byte in bytes {
        write!(out, "{byte:02x}").expect("writing to a String cannot fail");
    }
    out
}

fn rotation_message(sig_hash: &Hash, next: &PublicKey) -> Hash {
    let mut tag = [0; 32];
    tag[..14].copy_from_slice(b"QDAY-ROTATE-V1");
    keccak256(&[&tag, sig_hash, &next.root, &next.pub_seed])
}

fn main() {
    let seed: Hash = core::array::from_fn(|i| if i % 2 == 0 { 0x5e } else { 0xed });
    let old = Signer::new(&seed, 0);
    let new = Signer::new(&seed, 1);
    let (old_key, new_key) = (old.public_key(), new.public_key());
    let rotation = rotation_message(&HASH_A, &new_key);

    let gas_samples: Vec<String> = (0..GAS_SAMPLES)
        .map(|index| {
            let message = keccak256(&[&index.to_be_bytes()]);
            format!("\"{}\"", hex(&old.sign(index, &message)))
        })
        .collect();

    let fields = [
        ("root0", hex(&old_key.root)),
        ("pubSeed0", hex(&old_key.pub_seed)),
        ("root1", hex(&new_key.root)),
        ("pubSeed1", hex(&new_key.pub_seed)),
        ("hashA", hex(&HASH_A)),
        ("hashB", hex(&HASH_B)),
        ("vector", hex(&old.sign(5, &[0x11; 32]))),
        ("sig5", hex(&old.sign(5, &HASH_A))),
        ("sig6HashB", hex(&old.sign(6, &HASH_B))),
        ("sig1007", hex(&old.sign(1007, &HASH_A))),
        ("sig1008Plain", hex(&old.sign(1008, &HASH_A))),
        ("sig1008Rotate", hex(&old.sign(1008, &rotation))),
        ("sig1023Rotate", hex(&old.sign(1023, &rotation))),
        ("sig5Rotate", hex(&old.sign(5, &rotation))),
        ("newSig0HashB", hex(&new.sign(0, &HASH_B))),
    ];

    println!("{{");
    for (name, value) in fields {
        println!("  \"{name}\": \"{value}\",");
    }
    println!("  \"gasSamples\": [{}]", gas_samples.join(", "));
    println!("}}");
}
