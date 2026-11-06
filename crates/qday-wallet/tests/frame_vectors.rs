//! The published transaction vectors, and what the strict decoder refuses.

use qday_wallet::frame::{Action, DEFAULT_FEE, DecodeError, FrameTx, rotation_message};
use qday_wallet::rlp::{put_bytes, put_list, put_uint};
use qday_xmss::{Hash, KeySet, PublicKey, Tree, keccak256, verify};

const SENDER: [u8; 20] = [0x11; 20];
const TO: [u8; 20] = [0x22; 20];
const NONCE: u64 = 7;
const VALUE: u128 = 1_000_000_000_000_000;

fn parse(text: &str) -> Hash {
    let digits = text.strip_prefix("0x").expect("vectors start with 0x");
    let mut out = [0; 32];
    for (slot, pair) in out.iter_mut().zip(digits.as_bytes().chunks(2)) {
        let pair = std::str::from_utf8(pair).expect("vectors are ASCII");
        *slot = u8::from_str_radix(pair, 16).expect("vectors are hex");
    }
    out
}

fn seed() -> Hash {
    core::array::from_fn(|i| if i % 2 == 0 { 0x5e } else { 0xed })
}

fn next_key() -> (Hash, Hash) {
    (
        parse("0x18479eb13d693cdb0474c6de337a4544b3a93939bc20476ac23364491c04e881"),
        parse("0x8c587f5c22ec4115ef0688f0cb254e8fe084ab4ce56416257d8494bddd56a754"),
    )
}

fn transfer() -> FrameTx {
    FrameTx::transfer(SENDER, NONCE, 5, TO, VALUE)
}

fn rotation() -> FrameTx {
    let (root, pub_seed) = next_key();
    FrameTx::rotation(SENDER, NONCE, 1008, root, pub_seed)
}

#[test]
fn transfer_sig_hash_matches_the_vector() {
    let want = parse("0x988d78a1953235fde7cc1bcfda1b7c100d57527e1bb78ce4f94983bb8fb72804");
    assert_eq!(transfer().sig_hash(), want);
}

/// The published raw vector is an encoding check. Its signature slot holds the published signature
/// over `0x11` x 32, so it would not verify on-chain.
#[test]
fn signed_transfer_matches_the_vector() {
    let keys = KeySet::derive(&seed(), 0);
    let tree = Tree::build(&keys, |_| {});
    let signature = keys
        .sign(&tree.root(), 5, &[0x11; 32], &tree.auth_path(5))
        .expect("the index is in range");
    let raw = transfer().encode(&signature);
    assert_eq!(raw.len(), 2666);
    let want = parse("0xb246bd4846b25790690b364e78705b523283897f436108eb02bbe9e7d112bee2");
    assert_eq!(keccak256(&[&raw]), want);
}

#[test]
fn a_signature_over_the_signing_message_verifies() {
    let keys = KeySet::derive(&seed(), 0);
    let tree = Tree::build(&keys, |_| {});
    let key = PublicKey {
        root: tree.root(),
        pub_seed: *keys.pub_seed(),
    };
    for tx in [transfer(), rotation()] {
        let message = tx.signing_message();
        let signature = keys
            .sign(&tree.root(), tx.index, &message, &tree.auth_path(tx.index))
            .expect("the index is in range");
        assert!(verify(&key, &message, &signature));
    }
}

#[test]
fn rotation_sig_hash_matches_the_vector() {
    let want = parse("0xa7c6c4859d6884586888ef4a36d41a909f04e0ecbc2a8110151bf4a64ffcae7a");
    assert_eq!(rotation().sig_hash(), want);
}

#[test]
fn rotation_message_matches_the_vector() {
    let want = parse("0xa332fb67be17639d700170f0e46f80323df4a5f50e9f9101b43569f9aa02de2a");
    assert_eq!(rotation().signing_message(), want);
    let (root, pub_seed) = next_key();
    assert_eq!(
        rotation_message(&rotation().sig_hash(), &root, &pub_seed),
        want
    );
}

#[test]
fn a_transfer_signs_its_sig_hash() {
    assert_eq!(transfer().signing_message(), transfer().sig_hash());
}

#[test]
fn wire_bytes_start_with_the_type() {
    assert_eq!(transfer().encode(&[])[0], 0x06);
}

#[test]
fn unsigned_transfer_round_trips() {
    let tx = transfer();
    assert_eq!(FrameTx::decode_unsigned(&tx.encode(&[])), Ok(tx));
}

#[test]
fn unsigned_rotation_round_trips() {
    let tx = rotation();
    assert_eq!(FrameTx::decode_unsigned(&tx.encode(&[])), Ok(tx));
}

#[test]
fn a_zero_value_transfer_round_trips() {
    let tx = FrameTx::transfer(SENDER, 0, 0, TO, 0);
    assert_eq!(FrameTx::decode_unsigned(&tx.encode(&[])), Ok(tx));
}

#[test]
fn an_unknown_call_decodes_as_other() {
    let mut tx = transfer();
    tx.action = Action::Other {
        target: Some(TO),
        value: 3,
        data: vec![0xa9, 0x05, 0x9c, 0xbb, 1, 2, 3],
        execution: 50_000,
        state: 200_000,
    };
    assert_eq!(FrameTx::decode_unsigned(&tx.encode(&[])), Ok(tx));
}

#[test]
fn a_call_back_into_the_account_decodes_as_other() {
    let mut tx = transfer();
    tx.action = Action::Other {
        target: None,
        value: 0,
        data: vec![0x12, 0x34, 0x56, 0x78],
        execution: 50_000,
        state: 200_000,
    };
    assert_eq!(FrameTx::decode_unsigned(&tx.encode(&[])), Ok(tx));
}

#[test]
fn a_transfer_with_unusual_limits_decodes_as_other() {
    let raw = Shape::transfer().third_limits(90_000, 200_000).build();
    let got = FrameTx::decode_unsigned(&raw).expect("the shape is allowed");
    assert!(matches!(
        got.action,
        Action::Other {
            execution: 90_000,
            ..
        }
    ));
}

#[test]
fn a_rotate_with_a_value_decodes_as_other() {
    let raw = Shape::rotation().third_value(1).build();
    let got = FrameTx::decode_unsigned(&raw).expect("the shape is allowed");
    assert!(matches!(got.action, Action::Other { value: 1, .. }));
}

#[test]
fn a_rotate_aimed_elsewhere_decodes_as_other() {
    let raw = Shape::rotation().third_target(&TO).build();
    let got = FrameTx::decode_unsigned(&raw).expect("the shape is allowed");
    assert!(matches!(
        got.action,
        Action::Other {
            target: Some(TO),
            ..
        }
    ));
}

#[test]
fn fees_are_read_from_the_transaction() {
    let got = FrameTx::decode_unsigned(&Shape::transfer().fees(1, 5).build())
        .expect("the shape is allowed");
    assert_eq!((got.max_priority_fee, got.max_fee), (1, 5));
}

#[test]
fn the_maximum_fee_covers_every_frame() {
    // 300,000 + 150,000 + 250,000 of gas at the default fee.
    assert_eq!(transfer().gas_limit(), 700_000);
    assert_eq!(transfer().max_fee_wei(), 700_000 * DEFAULT_FEE);
    // 300,000 + 150,000 + 280,000.
    assert_eq!(rotation().gas_limit(), 730_000);
}

// Everything below must be refused.

fn refused(raw: &[u8]) {
    assert_eq!(FrameTx::decode_unsigned(raw), Err(DecodeError));
}

#[test]
fn the_shape_builder_matches_the_encoder() {
    assert_eq!(Shape::transfer().build(), transfer().encode(&[]));
    assert_eq!(Shape::rotation().build(), rotation().encode(&[]));
}

#[test]
fn a_signed_transaction_is_refused() {
    refused(&transfer().encode(&[1; 2500]));
}

#[test]
fn a_wrong_type_byte_is_refused() {
    let mut raw = transfer().encode(&[]);
    raw[0] = 0x05;
    refused(&raw);
}

#[test]
fn trailing_bytes_are_refused() {
    let mut raw = transfer().encode(&[]);
    raw.push(0);
    refused(&raw);
}

#[test]
fn empty_input_is_refused() {
    refused(&[]);
}

#[test]
fn every_truncation_is_refused() {
    let raw = transfer().encode(&[]);
    for length in 0..raw.len() {
        refused(&raw[..length]);
    }
}

#[test]
fn another_chain_is_refused() {
    refused(&Shape::transfer().chain(1).build());
}

#[test]
fn other_nonce_keys_are_refused() {
    refused(&Shape::transfer().nonce_keys(&[1]).build());
    refused(&Shape::transfer().nonce_keys(&[0, 0]).build());
    refused(&Shape::transfer().nonce_keys(&[]).build());
}

#[test]
fn a_short_sender_is_refused() {
    refused(&Shape::transfer().sender(&[0x11; 19]).build());
}

#[test]
fn two_frames_are_refused() {
    refused(&Shape::transfer().frames(2).build());
}

#[test]
fn four_frames_are_refused() {
    refused(&Shape::transfer().frames(4).build());
}

#[test]
fn another_verify_frame_is_refused() {
    refused(&Shape::transfer().verify_flags(1).build());
    refused(
        &Shape::transfer()
            .verify_data(&[0x69, 0x01, 0xf6, 0x69])
            .build(),
    );
    refused(&Shape::transfer().verify_target(&TO).build());
    refused(&Shape::transfer().verify_limits(1_000_000, 0).build());
}

#[test]
fn another_spend_frame_is_refused() {
    refused(&Shape::transfer().spend_target(&TO).build());
    refused(&Shape::transfer().spend_limits(50_000, 1).build());
    let mut data = spend_data(5);
    data[0] ^= 1;
    refused(&Shape::transfer().spend_data(&data).build());
    data = spend_data(5);
    data.push(0);
    refused(&Shape::transfer().spend_data(&data).build());
}

#[test]
fn an_index_that_needs_more_than_32_bits_is_refused() {
    let mut data = spend_data(5);
    data[4] = 1;
    refused(&Shape::transfer().spend_data(&data).build());
}

#[test]
fn a_third_frame_that_is_not_sender_mode_is_refused() {
    refused(&Shape::transfer().third_mode(1).build());
}

#[test]
fn a_third_frame_with_flags_is_refused() {
    refused(&Shape::transfer().third_flags(1).build());
}

#[test]
fn a_nonzero_blob_fee_is_refused() {
    refused(&Shape::transfer().blob_fee(1).build());
}

#[test]
fn blob_hashes_or_root_references_are_refused() {
    refused(&Shape::transfer().tail_item(0).build());
    refused(&Shape::transfer().tail_item(1).build());
}

#[test]
fn another_signature_scheme_is_refused() {
    refused(&Shape::transfer().scheme(1).build());
}

#[test]
fn a_filled_signer_or_msg_field_is_refused() {
    refused(&Shape::transfer().signer_field(&[1]).build());
    refused(&Shape::transfer().msg_field(&[1]).build());
}

#[test]
fn two_signature_entries_are_refused() {
    refused(&Shape::transfer().signature_entries(2).build());
}

#[test]
fn an_address_target_of_the_wrong_length_is_refused() {
    refused(&Shape::transfer().third_target(&[0x22; 19]).build());
}

fn spend_data(index: u32) -> Vec<u8> {
    let mut data = vec![0x92, 0xf5, 0x9d, 0x73];
    data.extend([0; 28]);
    data.extend(index.to_be_bytes());
    data
}

#[derive(Clone)]
struct FrameShape {
    mode: u128,
    flags: u128,
    target: Vec<u8>,
    execution: u128,
    state: u128,
    value: u128,
    data: Vec<u8>,
}

/// Builds the wire bytes field by field, independently of the encoder, so tests can break any one.
#[derive(Clone)]
struct Shape {
    chain: u128,
    nonce_keys: Vec<u128>,
    sender: Vec<u8>,
    frames: Vec<FrameShape>,
    scheme: u128,
    signer: Vec<u8>,
    msg: Vec<u8>,
    signature_entries: usize,
    fees: [u128; 3],
    tails: [Vec<u8>; 2],
}

impl Shape {
    fn with_third(third: FrameShape) -> Self {
        let verify = FrameShape {
            mode: 1,
            flags: 3,
            target: Vec::new(),
            execution: 300_000,
            state: 0,
            value: 0,
            data: vec![0x69, 0x01, 0xf6, 0x68],
        };
        let spend = FrameShape {
            mode: 2,
            flags: 0,
            target: Vec::new(),
            execution: 50_000,
            state: 100_000,
            value: 0,
            data: Vec::new(),
        };
        Self {
            chain: 1337,
            nonce_keys: vec![0],
            sender: SENDER.to_vec(),
            frames: vec![verify, spend, third],
            scheme: 0,
            signer: Vec::new(),
            msg: Vec::new(),
            signature_entries: 1,
            fees: [DEFAULT_FEE, DEFAULT_FEE, 0],
            tails: [Vec::new(), Vec::new()],
        }
    }

    fn transfer() -> Self {
        let mut shape = Self::with_third(FrameShape {
            mode: 2,
            flags: 0,
            target: TO.to_vec(),
            execution: 50_000,
            state: 200_000,
            value: VALUE,
            data: Vec::new(),
        });
        shape.frames[1].data = spend_data(5);
        shape
    }

    fn rotation() -> Self {
        let (root, pub_seed) = next_key();
        let mut data = vec![0xed, 0x29, 0x6e, 0x9c];
        data.extend(root);
        data.extend(pub_seed);
        let mut shape = Self::with_third(FrameShape {
            mode: 2,
            flags: 0,
            target: Vec::new(),
            execution: 80_000,
            state: 200_000,
            value: 0,
            data,
        });
        shape.frames[1].data = spend_data(1008);
        shape
    }

    fn chain(mut self, chain: u128) -> Self {
        self.chain = chain;
        self
    }
    fn nonce_keys(mut self, keys: &[u128]) -> Self {
        self.nonce_keys = keys.to_vec();
        self
    }
    fn sender(mut self, sender: &[u8]) -> Self {
        self.sender = sender.to_vec();
        self
    }
    fn frames(mut self, count: usize) -> Self {
        let third = self.frames[2].clone();
        self.frames.resize(count, third);
        self
    }
    fn verify_flags(mut self, flags: u128) -> Self {
        self.frames[0].flags = flags;
        self
    }
    fn verify_data(mut self, data: &[u8]) -> Self {
        self.frames[0].data = data.to_vec();
        self
    }
    fn verify_target(mut self, target: &[u8]) -> Self {
        self.frames[0].target = target.to_vec();
        self
    }
    fn verify_limits(mut self, execution: u128, state: u128) -> Self {
        self.frames[0].execution = execution;
        self.frames[0].state = state;
        self
    }
    fn spend_target(mut self, target: &[u8]) -> Self {
        self.frames[1].target = target.to_vec();
        self
    }
    fn spend_limits(mut self, execution: u128, state: u128) -> Self {
        self.frames[1].execution = execution;
        self.frames[1].state = state;
        self
    }
    fn spend_data(mut self, data: &[u8]) -> Self {
        self.frames[1].data = data.to_vec();
        self
    }
    fn third_mode(mut self, mode: u128) -> Self {
        self.frames[2].mode = mode;
        self
    }
    fn third_flags(mut self, flags: u128) -> Self {
        self.frames[2].flags = flags;
        self
    }
    fn third_target(mut self, target: &[u8]) -> Self {
        self.frames[2].target = target.to_vec();
        self
    }
    fn third_limits(mut self, execution: u128, state: u128) -> Self {
        self.frames[2].execution = execution;
        self.frames[2].state = state;
        self
    }
    fn third_value(mut self, value: u128) -> Self {
        self.frames[2].value = value;
        self
    }
    fn fees(mut self, priority: u128, max: u128) -> Self {
        self.fees[0] = priority;
        self.fees[1] = max;
        self
    }
    fn blob_fee(mut self, fee: u128) -> Self {
        self.fees[2] = fee;
        self
    }
    fn tail_item(mut self, which: usize) -> Self {
        self.tails[which] = vec![0x01];
        self
    }
    fn scheme(mut self, scheme: u128) -> Self {
        self.scheme = scheme;
        self
    }
    fn signer_field(mut self, value: &[u8]) -> Self {
        self.signer = value.to_vec();
        self
    }
    fn msg_field(mut self, value: &[u8]) -> Self {
        self.msg = value.to_vec();
        self
    }
    fn signature_entries(mut self, count: usize) -> Self {
        self.signature_entries = count;
        self
    }

    fn build(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        put_uint(&mut payload, self.chain);
        let mut keys = Vec::new();
        for key in &self.nonce_keys {
            put_uint(&mut keys, *key);
        }
        put_list(&mut payload, &keys);
        put_uint(&mut payload, u128::from(NONCE));
        put_bytes(&mut payload, &self.sender);

        let mut frames = Vec::new();
        for frame in &self.frames {
            let mut limits = Vec::new();
            put_uint(&mut limits, frame.execution);
            put_uint(&mut limits, frame.state);
            let mut item = Vec::new();
            put_uint(&mut item, frame.mode);
            put_uint(&mut item, frame.flags);
            put_bytes(&mut item, &frame.target);
            put_list(&mut item, &limits);
            put_uint(&mut item, frame.value);
            put_bytes(&mut item, &frame.data);
            put_list(&mut frames, &item);
        }
        put_list(&mut payload, &frames);

        let mut entries = Vec::new();
        for _ in 0..self.signature_entries {
            let mut entry = Vec::new();
            put_uint(&mut entry, self.scheme);
            put_bytes(&mut entry, &self.signer);
            put_bytes(&mut entry, &self.msg);
            put_bytes(&mut entry, &[]);
            put_list(&mut entries, &entry);
        }
        put_list(&mut payload, &entries);

        let mut fees = Vec::new();
        for fee in self.fees {
            put_uint(&mut fees, fee);
        }
        put_list(&mut payload, &fees);
        for tail in &self.tails {
            put_list(&mut payload, tail);
        }

        let mut raw = vec![0x06];
        put_list(&mut raw, &payload);
        raw
    }
}
