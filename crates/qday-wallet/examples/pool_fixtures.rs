//! Writes what the pool's Solidity tests need, from the same code the device signs with. From the
//! repository root:
//!
//!     cargo run -p qday-wallet --example pool_fixtures > contracts/test/fixtures/hashpool.json

use std::fmt::Write as _;

use qday_wallet::invoice::{keys_for, spend_hash};
use qday_xmss::Hash;

const SEED: Hash = [0x5E; 32];
/// Where the tests put the pool, so these signatures are for it.
const POOL: [u8; 20] = [0x90; 20];
const TO: [u8; 20] = [0xBE; 20];
const FEE: u128 = 10_000_000_000_000_000;
const SAMPLES: u32 = 4;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::from("0x"), |mut out, byte| {
        write!(out, "{byte:02x}").expect("writing to a String cannot fail");
        out
    })
}

fn main() {
    let mut fields = vec![];
    for number in 0..SAMPLES {
        let keys = keys_for(&SEED, 0, number);
        let leaf = keys.leaf(0);
        let signature = keys.sign_one_time(&spend_hash(&POOL, &leaf, &TO, FEE));
        fields.push(format!(r#""leaf{number}": "{}""#, hex(&leaf)));
        fields.push(format!(r#""seed{number}": "{}""#, hex(keys.pub_seed())));
        fields.push(format!(r#""sig{number}": "{}""#, hex(&signature)));
    }
    println!("{{\n  {}\n}}", fields.join(",\n  "));
}
