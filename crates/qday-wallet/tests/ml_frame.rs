//! The two-frame transaction for the ML-DSA account, built here field by field so the encoder is
//! checked against something independent of it.

use qday_wallet::frame::{DEFAULT_FEE, DecodeError, FrameTx, MlFrameTx};
use qday_wallet::rlp::{put_bytes, put_list, put_uint};
use qday_xmss::keccak256;

const SENDER: [u8; 20] = [0x11; 20];
const TO: [u8; 20] = [0x22; 20];
const VALUE: u128 = 1_000_000_000_000_000;

#[derive(Clone)]
struct FrameShape {
    mode: u128,
    flags: u128,
    target: Vec<u8>,
    limits: Vec<u128>,
    value: u128,
    data: Vec<u8>,
}

#[derive(Clone)]
struct Shape {
    chain: u128,
    keys: Vec<u128>,
    nonce: u128,
    sender: Vec<u8>,
    frames: Vec<FrameShape>,
    scheme: u128,
    signature: Vec<u8>,
    entries: usize,
    fees: Vec<u128>,
    tail: [Vec<u8>; 2],
}

impl Shape {
    fn new() -> Self {
        Self {
            chain: 1337,
            keys: vec![0],
            nonce: 7,
            sender: SENDER.to_vec(),
            frames: vec![
                FrameShape {
                    mode: 1,
                    flags: 3,
                    target: Vec::new(),
                    limits: vec![1_500_000, 0],
                    value: 0,
                    data: vec![0x69, 0x01, 0xf6, 0x68],
                },
                FrameShape {
                    mode: 2,
                    flags: 0,
                    target: TO.to_vec(),
                    limits: vec![50_000, 200_000],
                    value: VALUE,
                    data: Vec::new(),
                },
            ],
            scheme: 0,
            signature: Vec::new(),
            entries: 1,
            fees: vec![DEFAULT_FEE, DEFAULT_FEE, 0],
            tail: [Vec::new(), Vec::new()],
        }
    }

    fn with(mut self, change: impl FnOnce(&mut Self)) -> Self {
        change(&mut self);
        self
    }

    fn build(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        put_uint(&mut payload, self.chain);
        let mut keys = Vec::new();
        for key in &self.keys {
            put_uint(&mut keys, *key);
        }
        put_list(&mut payload, &keys);
        put_uint(&mut payload, self.nonce);
        put_bytes(&mut payload, &self.sender);

        let mut frames = Vec::new();
        for frame in &self.frames {
            let mut limits = Vec::new();
            for limit in &frame.limits {
                put_uint(&mut limits, *limit);
            }
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
        for _ in 0..self.entries {
            let mut entry = Vec::new();
            put_uint(&mut entry, self.scheme);
            put_bytes(&mut entry, &[]);
            put_bytes(&mut entry, &[]);
            put_bytes(&mut entry, &self.signature);
            put_list(&mut entries, &entry);
        }
        put_list(&mut payload, &entries);

        let mut fees = Vec::new();
        for fee in &self.fees {
            put_uint(&mut fees, *fee);
        }
        put_list(&mut payload, &fees);
        for tail in &self.tail {
            put_list(&mut payload, tail);
        }

        let mut raw = vec![0x06];
        put_list(&mut raw, &payload);
        raw
    }
}

/// One way of breaking the shape.
type Change = Box<dyn Fn(&mut Shape)>;

fn tx() -> MlFrameTx {
    MlFrameTx::transfer(SENDER, 7, TO, VALUE)
}

fn refused(raw: &[u8]) {
    assert_eq!(MlFrameTx::decode_unsigned(raw), Err(DecodeError));
}

#[test]
fn the_encoder_matches_the_field_by_field_shape() {
    assert_eq!(tx().encode(&[]), Shape::new().build());
}

#[test]
fn the_signature_goes_in_the_signature_entry() {
    let signature = vec![0xAB; 2420];
    let shape = Shape::new().with(|shape| shape.signature.clone_from(&signature));
    assert_eq!(tx().encode(&signature), shape.build());
}

#[test]
fn the_sig_hash_is_the_hash_of_the_unsigned_bytes() {
    assert_eq!(tx().sig_hash(), keccak256(&[&Shape::new().build()]));
}

#[test]
fn the_sig_hash_covers_every_field() {
    let hashes = [
        tx().sig_hash(),
        MlFrameTx::transfer([0x12; 20], 7, TO, VALUE).sig_hash(),
        MlFrameTx::transfer(SENDER, 8, TO, VALUE).sig_hash(),
        MlFrameTx::transfer(SENDER, 7, [0x23; 20], VALUE).sig_hash(),
        MlFrameTx::transfer(SENDER, 7, TO, VALUE + 1).sig_hash(),
    ];
    for (i, hash) in hashes.iter().enumerate() {
        assert!(!hashes[i + 1..].contains(hash));
    }
}

#[test]
fn an_unsigned_transaction_round_trips() {
    let tx = tx();
    assert_eq!(MlFrameTx::decode_unsigned(&tx.encode(&[])), Ok(tx));
}

#[test]
fn a_zero_value_round_trips() {
    let tx = MlFrameTx::transfer(SENDER, 0, TO, 0);
    assert_eq!(MlFrameTx::decode_unsigned(&tx.encode(&[])), Ok(tx));
}

#[test]
fn fees_are_read_from_the_transaction() {
    let raw = Shape::new()
        .with(|shape| shape.fees = vec![1, 5, 0])
        .build();
    let tx = MlFrameTx::decode_unsigned(&raw).expect("the shape is allowed");
    assert_eq!((tx.max_priority_fee, tx.max_fee), (1, 5));
}

#[test]
fn the_maximum_fee_covers_both_frames() {
    assert_eq!(tx().gas_limit(), 1_750_000);
    assert_eq!(tx().max_fee_wei(), 1_750_000 * DEFAULT_FEE);
}

#[test]
fn the_frames_are_presented_in_order() {
    let [verify, transfer] = tx().frame_views();
    assert_eq!((verify.mode, verify.flags, verify.target), (1, 3, None));
    assert_eq!((verify.execution, verify.state), (1_500_000, 0));
    assert_eq!(verify.data, [0x69, 0x01, 0xf6, 0x68]);
    assert_eq!(
        (transfer.mode, transfer.flags, transfer.target),
        (2, 0, Some(TO))
    );
    assert_eq!(
        (transfer.execution, transfer.state, transfer.value),
        (50_000, 200_000, VALUE)
    );
    assert!(transfer.data.is_empty());
}

#[test]
fn the_two_shapes_never_decode_as_each_other() {
    let xmss = FrameTx::transfer(SENDER, 7, 5, TO, VALUE).encode(&[]);
    refused(&xmss);
    assert_eq!(
        FrameTx::decode_unsigned(&tx().encode(&[])),
        Err(DecodeError)
    );
}

#[test]
fn every_truncation_is_refused() {
    let raw = tx().encode(&[]);
    for length in 0..raw.len() {
        refused(&raw[..length]);
    }
}

#[test]
fn trailing_bytes_are_refused() {
    let mut raw = tx().encode(&[]);
    raw.push(0);
    refused(&raw);
}

#[test]
fn a_wrong_type_byte_is_refused() {
    let mut raw = tx().encode(&[]);
    raw[0] = 0x05;
    refused(&raw);
}

#[test]
fn a_signed_transaction_is_refused() {
    refused(&tx().encode(&[1; 2420]));
}

#[test]
fn every_changed_field_is_refused() {
    let cases: Vec<Change> = vec![
        Box::new(|shape| shape.chain = 1),
        Box::new(|shape| shape.keys = vec![1]),
        Box::new(|shape| shape.keys = vec![0, 0]),
        Box::new(|shape| shape.keys = vec![]),
        Box::new(|shape| shape.sender = vec![0x11; 19]),
        Box::new(|shape| shape.scheme = 1),
        Box::new(|shape| shape.entries = 2),
        Box::new(|shape| shape.entries = 0),
        Box::new(|shape| shape.fees = vec![DEFAULT_FEE, DEFAULT_FEE, 1]),
        Box::new(|shape| shape.tail[0] = vec![1]),
        Box::new(|shape| shape.tail[1] = vec![1]),
        // The verify frame.
        Box::new(|shape| shape.frames[0].mode = 2),
        Box::new(|shape| shape.frames[0].flags = 1),
        Box::new(|shape| shape.frames[0].target = TO.to_vec()),
        Box::new(|shape| shape.frames[0].limits = vec![1_499_999, 0]),
        Box::new(|shape| shape.frames[0].limits = vec![1_500_000, 1]),
        Box::new(|shape| shape.frames[0].value = 1),
        Box::new(|shape| shape.frames[0].data = vec![0x69, 0x01, 0xf6, 0x69]),
        // The transfer frame.
        Box::new(|shape| shape.frames[1].mode = 1),
        Box::new(|shape| shape.frames[1].flags = 1),
        Box::new(|shape| shape.frames[1].target = Vec::new()),
        Box::new(|shape| shape.frames[1].target = vec![0x22; 19]),
        Box::new(|shape| shape.frames[1].limits = vec![50_001, 200_000]),
        Box::new(|shape| shape.frames[1].limits = vec![50_000, 200_001]),
        Box::new(|shape| shape.frames[1].limits = vec![50_000]),
        Box::new(|shape| shape.frames[1].data = vec![1]),
        // The frame count.
        Box::new(|shape| {
            shape.frames.pop();
        }),
        Box::new(|shape| {
            let extra = shape.frames[1].clone();
            shape.frames.push(extra);
        }),
    ];
    for change in &cases {
        refused(&Shape::new().with(|shape| change(shape)).build());
    }
}
