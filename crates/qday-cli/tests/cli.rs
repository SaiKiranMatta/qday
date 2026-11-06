//! Runs the real binary, because the scripts depend on exactly what it prints.

use std::process::{Command, Output};

use qday_mldsa::blob::prepare;
use qday_mldsa::{KeySeed, Signer, StandardSigner};
use qday_wallet::hex;
use qday_xmss::keccak256;

const SEED: &str = "0x5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed";
const ROOT0: &str = "0xbfefd6eccd30b7f3b763b5289150e2ee0124555cd5ac7f14601005179d1273e9";
const PUB_SEED0: &str = "0x78e135a918cf039e569ea83fdefdb29ddd1985b7b33a9867da1edd9026adceb8";
const ROOT1: &str = "0x18479eb13d693cdb0474c6de337a4544b3a93939bc20476ac23364491c04e881";
const PUB_SEED1: &str = "0x8c587f5c22ec4115ef0688f0cb254e8fe084ab4ce56416257d8494bddd56a754";
const SENDER: &str = "0x1111111111111111111111111111111111111111";
const TO: &str = "0x2222222222222222222222222222222222222222";
const MESSAGE: &str = "0x1111111111111111111111111111111111111111111111111111111111111111";
const VALUE: &str = "1000000000000000";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_qday"))
        .args(args)
        .output()
        .expect("the binary runs")
}

fn stdout(args: &[&str]) -> String {
    let output = run(args);
    assert!(
        output.status.success(),
        "{args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("output is text")
        .trim()
        .to_string()
}

fn vector_signature() -> String {
    stdout(&["sign", SEED, "5", MESSAGE])
}

#[test]
fn pubkey_prints_root_and_pub_seed() {
    assert_eq!(stdout(&["pubkey", SEED]), format!("{ROOT0} {PUB_SEED0}"));
}

#[test]
fn pubkey_takes_a_tree_number() {
    assert_eq!(
        stdout(&["pubkey", SEED, "--tree", "1"]),
        format!("{ROOT1} {PUB_SEED1}")
    );
}

#[test]
fn sign_matches_the_signature_vector() {
    let signature = hex::decode(&vector_signature()).expect("the signature is hex");
    assert_eq!(signature.len(), 2500);
    assert_eq!(
        hex::encode(&keccak256(&[&signature])),
        "0x8937def506577209cdaaff87612f54af2e417d5502132267f358b5daa32ece1f"
    );
}

#[test]
fn words_to_seed_reads_24_separate_words() {
    let mut args = vec!["words-to-seed"];
    args.extend(["abandon"; 23]);
    args.push("art");
    assert_eq!(stdout(&args), format!("0x{}", "00".repeat(32)));
}

#[test]
fn words_to_seed_reads_one_quoted_argument() {
    let words = format!("{}art", "abandon ".repeat(23));
    assert_eq!(
        stdout(&["words-to-seed", &words]),
        format!("0x{}", "00".repeat(32))
    );
}

#[test]
fn bad_words_fail() {
    assert!(!run(&["words-to-seed", "abandon"]).status.success());
}

#[test]
fn frame_hash_matches_the_vector() {
    assert_eq!(
        stdout(&["frame-hash", SENDER, "7", "5", TO, VALUE]),
        "0x988d78a1953235fde7cc1bcfda1b7c100d57527e1bb78ce4f94983bb8fb72804"
    );
}

#[test]
fn frame_raw_matches_the_vector() {
    let raw = stdout(&[
        "frame-raw",
        SENDER,
        "7",
        "5",
        TO,
        VALUE,
        &vector_signature(),
    ]);
    let raw = hex::decode(&raw).expect("the output is hex");
    assert_eq!(raw.len(), 2666);
    assert_eq!(
        hex::encode(&keccak256(&[&raw])),
        "0xb246bd4846b25790690b364e78705b523283897f436108eb02bbe9e7d112bee2"
    );
}

#[test]
fn frame_rotate_hash_matches_the_vector() {
    assert_eq!(
        stdout(&["frame-rotate-hash", SENDER, "7", "1008", ROOT1, PUB_SEED1]),
        "0xa7c6c4859d6884586888ef4a36d41a909f04e0ecbc2a8110151bf4a64ffcae7a"
    );
}

#[test]
fn frame_rotate_message_matches_the_vector() {
    assert_eq!(
        stdout(&[
            "frame-rotate-message",
            SENDER,
            "7",
            "1008",
            ROOT1,
            PUB_SEED1
        ]),
        "0xa332fb67be17639d700170f0e46f80323df4a5f50e9f9101b43569f9aa02de2a"
    );
}

#[test]
fn frame_rotate_raw_carries_the_signature() {
    let signature = format!("0x{}", "ab".repeat(2500));
    let raw = stdout(&[
        "frame-rotate-raw",
        SENDER,
        "7",
        "1008",
        ROOT1,
        PUB_SEED1,
        &signature,
    ]);
    assert!(raw.starts_with("0x06"));
    assert!(raw.contains(&"ab".repeat(2500)));
}

#[test]
fn frame_json_has_the_submission_shape() {
    let signature = format!("0x{}", "cd".repeat(2500));
    let text = stdout(&["frame-json", SENDER, "7", "5", TO, VALUE, &signature]);
    let json: serde_json::Value = serde_json::from_str(&text).expect("the output is JSON");

    assert_eq!(json["chainId"], "0x539");
    assert_eq!(json["nonceKeys"], serde_json::json!(["0x0"]));
    assert_eq!(json["nonceSeq"], "0x7");
    assert_eq!(json["sender"], SENDER);

    let frames = json["frames"].as_array().expect("frames is a list");
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[0]["mode"], 1);
    assert_eq!(frames[0]["flags"], 3);
    assert_eq!(frames[0]["target"], serde_json::Value::Null);
    assert_eq!(frames[0]["limits"]["execution"], "0x493e0");
    assert_eq!(frames[0]["limits"]["state"], "0x0");
    assert_eq!(frames[0]["value"], "0x0");
    assert_eq!(frames[0]["data"], "0x6901f668");
    assert_eq!(frames[1]["mode"], 2);
    assert_eq!(frames[1]["flags"], 0);
    assert_eq!(frames[2]["target"], TO);
    assert_eq!(frames[2]["value"], "0x38d7ea4c68000");
    assert_eq!(frames[2]["data"], "0x");

    assert_eq!(json["signatures"][0]["scheme"], 0);
    assert_eq!(json["signatures"][0]["signer"], "0x");
    assert_eq!(json["signatures"][0]["msg"], "0x");
    assert_eq!(json["signatures"][0]["signature"], signature);

    assert_eq!(json["fees"]["maxPriorityFeePerGas"], "0x77359400");
    assert_eq!(json["fees"]["maxFeePerGas"], "0x77359400");
    assert_eq!(json["fees"]["maxFeePerBlobGas"], "0x0");
    assert_eq!(json["blobVersionedHashes"], serde_json::json!([]));
    assert_eq!(json["recentRootReferences"], serde_json::json!([]));
}

#[test]
fn frame_rotate_json_targets_the_sender_with_the_rotate_call() {
    let signature = format!("0x{}", "cd".repeat(2500));
    let text = stdout(&[
        "frame-rotate-json",
        SENDER,
        "7",
        "1008",
        ROOT1,
        PUB_SEED1,
        &signature,
    ]);
    let json: serde_json::Value = serde_json::from_str(&text).expect("the output is JSON");
    let third = &json["frames"][2];
    assert_eq!(third["target"], serde_json::Value::Null);
    assert_eq!(third["limits"]["execution"], "0x13880");
    assert!(
        third["data"]
            .as_str()
            .expect("data is text")
            .starts_with("0xed296e9c")
    );
}

fn mldsa_key() -> String {
    let seed = hex::decode_array(SEED).expect("the seed is hex");
    let signer = StandardSigner::from_key_seed(&KeySeed::derive(&seed, 0));
    hex::encode(&signer.public_key())
}

#[test]
fn mldsa_blob_prints_the_prepared_blob() {
    let blob = stdout(&["mldsa-blob", &mldsa_key()]);
    let bytes = hex::decode(&blob).expect("the output is hex");
    assert_eq!(bytes.len(), 20_545);
    assert_eq!(bytes[0], 0);
    let public: [u8; 1312] = hex::decode_array(&mldsa_key()).expect("the key is hex");
    assert_eq!(bytes, prepare(&public));
}

#[test]
fn mldsa_blob_refuses_a_key_of_the_wrong_length() {
    let longer = format!("{}00", mldsa_key());
    let output = run(&["mldsa-blob", &longer]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("1,312"));
}

#[test]
fn mldsa_blob_refuses_a_degenerate_key() {
    let degenerate = format!("0x{}", "ff".repeat(1312));
    let output = run(&["mldsa-blob", &degenerate]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("degenerate"));
}

#[test]
fn a_bad_address_fails() {
    assert!(
        !run(&["frame-hash", "0x11", "7", "5", TO, VALUE])
            .status
            .success()
    );
}

#[test]
fn an_index_past_the_tree_fails_to_sign() {
    assert!(!run(&["sign", SEED, "1024", MESSAGE]).status.success());
}
