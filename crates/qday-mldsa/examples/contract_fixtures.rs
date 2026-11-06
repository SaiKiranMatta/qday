//! Writes what the Solidity tests need, from the Rust signer and the built verifier. From the
//! repository root, after `forge build` in `contracts/mldsa`:
//!
//!     cargo run -p qday-mldsa --example contract_fixtures > contracts/test/fixtures/mldsa.json

use std::fmt::Write as _;

use qday_mldsa::blob::prepare;
use qday_mldsa::{KeySeed, Signer, StandardSigner};
use qday_xmss::{Hash, keccak256};

const VERIFIER: &str = "contracts/mldsa/out/MLDSA44Verifier.sol/MLDSA44Verifier.json";
const HELPER: &str = "contracts/mldsa/helpers/f1600_170.hex";
/// The transaction hashes these fixtures sign.
const HASH_A: Hash = [0xAA; 32];
const HASH_B: Hash = [0xBB; 32];
const RND: Hash = [0x22; 32];
/// Signatures used to measure average verifier gas.
const GAS_SAMPLES: u32 = 8;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::from("0x"), |mut out, byte| {
        write!(out, "{byte:02x}").expect("writing to a String cannot fail");
        out
    })
}

fn main() {
    let artifact = std::fs::read_to_string(VERIFIER).expect("contracts/mldsa has been built");
    let artifact: serde_json::Value =
        serde_json::from_str(&artifact).expect("the artifact is JSON");
    let verifier = artifact["bytecode"]["object"]
        .as_str()
        .expect("the artifact has bytecode");
    let helper = std::fs::read_to_string(HELPER).expect("the helper is vendored");

    let seed: Hash = core::array::from_fn(|i| if i % 2 == 0 { 0x5e } else { 0xed });
    let signer = StandardSigner::from_key_seed(&KeySeed::derive(&seed, 0));
    let other = StandardSigner::from_key_seed(&KeySeed::derive(&seed, 1));
    let sign = |message: &Hash| signer.sign(message, &RND).expect("signing works");

    let samples: Vec<String> = (0..GAS_SAMPLES)
        .map(|index| format!("\"{}\"", hex(&sign(&keccak256(&[&index.to_be_bytes()])))))
        .collect();

    let fields = [
        ("verifierInitcode", verifier.to_string()),
        ("helperRuntime", format!("0x{}", helper.trim())),
        ("blob", hex(&prepare(&signer.public_key()))),
        ("otherBlob", hex(&prepare(&other.public_key()))),
        ("hashA", hex(&HASH_A)),
        ("hashB", hex(&HASH_B)),
        ("sigA", hex(&sign(&HASH_A))),
        ("sigB", hex(&sign(&HASH_B))),
        (
            "otherSigA",
            hex(&other.sign(&HASH_A, &RND).expect("signing works")),
        ),
    ];

    println!("{{");
    for (name, value) in fields {
        println!("  \"{name}\": \"{value}\",");
    }
    println!("  \"gasSamples\": [{}]", samples.join(", "));
    println!("}}");
}
