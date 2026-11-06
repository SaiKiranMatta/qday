//! Writes what the lattice pool's Solidity tests need, from the real signer and the built verifier.
//! From the repository root, after `forge build` in `contracts/mldsa`:
//!
//!     cargo run -p qday-wallet --example lattice_fixtures > contracts/test/fixtures/lattice.json

use std::fmt::Write as _;

use qday_mldsa::Signer;
use qday_mldsa::blob::{key_part, prepare};
use qday_mldsa::stealth::Master;
use qday_wallet::invoice::spend_hash;
use qday_xmss::{Hash, keccak256};

const VERIFIER: &str =
    "contracts/mldsa/out/MLDSA44InlineVerifier.sol/MLDSA44InlineVerifier.json";
const HELPER: &str = "contracts/mldsa/helpers/f1600_170.hex";
const SEED: Hash = [0x42; 32];
/// Where the tests put the pool, so these signatures are for it.
const POOL: [u8; 20] = [0x90; 20];
const TO: [u8; 20] = [0xBE; 20];
const FEE: u128 = 10_000_000_000_000_000;
const RND: Hash = [0x22; 32];
const SAMPLES: u8 = 4;

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

    let master = Master::from_seed(&SEED);
    let stranger = Master::from_seed(&[0x43; 32]);
    let mut fields = vec![
        ("verifierInitcode".to_string(), verifier.to_string()),
        ("helperRuntime".to_string(), format!("0x{}", helper.trim())),
    ];
    for number in 0..SAMPLES {
        let signer = master.one_time(&[number; 64]);
        let public = signer.public_key();
        if number == 0 {
            let blob = prepare(&public);
            let mut matrix = vec![0];
            matrix.extend_from_slice(&blob[4161..]);
            fields.push(("matrix".to_string(), hex(&matrix)));
        }
        let key = key_part(&public);
        let message = spend_hash(&POOL, &keccak256(&[&key]), &TO, FEE);
        let signature = signer.sign(&message, &RND).expect("signing works");
        fields.push((format!("key{number}"), hex(&key)));
        fields.push((format!("sig{number}"), hex(&signature)));
        if number == 0 {
            let wrong = stranger.one_time(&[number; 64]);
            let signature = wrong.sign(&message, &RND).expect("signing works");
            fields.push(("strangerSig".to_string(), hex(&signature)));
        }
    }
    let lines: Vec<String> = fields
        .iter()
        .map(|(name, value)| format!("  \"{name}\": \"{value}\""))
        .collect();
    println!("{{\n{}\n}}", lines.join(",\n"));
}
