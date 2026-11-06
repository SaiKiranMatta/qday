//! Corrupted input from the laptop: the decoders must refuse it without panicking, and anything
//! they accept must be the one canonical encoding of what they decoded.

use qday_wallet::frame::{Action, FrameTx};
use qday_wallet::protocol::{Line, LineBuffer, Request, parse_request, request_line};

/// Mutated inputs per starting transaction.
const ROUNDS: u32 = 20_000;

/// A small deterministic generator, so a failure always reproduces.
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.next() % bound as u64).expect("bound fits in usize")
    }

    fn byte(&mut self) -> u8 {
        self.next().to_le_bytes()[0]
    }
}

fn samples() -> Vec<FrameTx> {
    let sender = [0x11; 20];
    let to = [0x22; 20];
    let mut other = FrameTx::transfer(sender, 7, 5, to, 3);
    other.action = Action::Other {
        target: Some(to),
        value: 3,
        data: vec![0xa9, 0x05, 0x9c, 0xbb, 1, 2, 3],
        execution: 50_000,
        state: 200_000,
    };
    vec![
        FrameTx::transfer(sender, 7, 5, to, 1_000_000_000_000_000),
        FrameTx::transfer(sender, 0, 0, to, 0),
        FrameTx::rotation(sender, 7, 1008, [0x33; 32], [0x44; 32]),
        other,
    ]
}

/// Flips, overwrites, inserts or removes a few bytes.
fn mutate(rng: &mut XorShift, input: &[u8]) -> Vec<u8> {
    let mut out = input.to_vec();
    for _ in 0..=rng.below(3) {
        if out.is_empty() {
            out.push(rng.byte());
            continue;
        }
        let at = rng.below(out.len());
        match rng.below(4) {
            0 => out[at] ^= 1 << rng.below(8),
            1 => out[at] = rng.byte(),
            2 => out.insert(at, rng.byte()),
            _ => {
                out.remove(at);
            }
        }
    }
    out
}

#[test]
fn mutated_transactions_never_panic_and_only_canonical_bytes_decode() {
    let mut rng = XorShift(0x9e37_79b9_7f4a_7c15);
    for tx in samples() {
        let raw = tx.encode(&[]);
        for _ in 0..ROUNDS {
            let input = mutate(&mut rng, &raw);
            if let Ok(decoded) = FrameTx::decode_unsigned(&input) {
                assert_eq!(
                    decoded.encode(&[]),
                    input,
                    "accepted a non-canonical encoding"
                );
            }
        }
    }
}

#[test]
fn truncated_transactions_are_refused() {
    for tx in samples() {
        let raw = tx.encode(&[]);
        for length in 0..raw.len() {
            assert!(FrameTx::decode_unsigned(&raw[..length]).is_err());
        }
    }
}

#[test]
fn mutated_request_lines_never_panic() {
    let mut rng = XorShift(0x2545_f491_4f6c_dd1d);
    let raw = FrameTx::transfer([0x11; 20], 7, 5, [0x22; 20], 3).encode(&[]);
    let requests = [
        Request::SignFrame(raw),
        Request::Restore {
            seed: [0xab; 32],
            tree: 0,
            chain_next_index: 42,
        },
        Request::Activate(1),
        Request::PubKey,
    ];
    for request in &requests {
        let written = request_line(request);
        let line = written.trim_end_matches('\n');
        assert_eq!(parse_request(line), Some(Ok(request.clone())));
        for _ in 0..ROUNDS {
            let bytes = mutate(&mut rng, line.as_bytes());
            if let Ok(text) = core::str::from_utf8(&bytes) {
                let _ = parse_request(text);
            }
        }
    }
}

#[test]
fn random_bytes_through_the_line_buffer_never_panic() {
    let mut rng = XorShift(0xdead_beef_cafe_f00d);
    let mut buffer = LineBuffer::default();
    for _ in 0..200_000 {
        let byte = if rng.below(64) == 0 {
            b'\n'
        } else {
            rng.byte()
        };
        if let Some(Line::Text(text)) = buffer.push(byte) {
            let _ = parse_request(text);
        }
    }
}
