//! The public key blob the on-chain verifier reads, and the checks that make one safe to deploy.
//! The verifier can't check any of this itself, so these checks are mandatory before deployment.

use alloc::vec;
use alloc::vec::Vec;

use sha3::digest::{ExtendableOutput, Update, XofReader};
use sha3::{Shake128, Shake256};

use crate::{PUBLIC_KEY_BYTES, PublicKey};

/// Length of a blob.
pub const BLOB_BYTES: usize = 20_545;
/// Length of the part of a blob that belongs to one key.
pub const KEY_PART_BYTES: usize = 4160;

const Q: u64 = 8_380_417;
const COEFFICIENTS: usize = 256;
const ROWS: usize = 4;
/// Bits dropped from `t`, so a coefficient of `t1` stands for `t1 * 2^13`.
const D: u32 = 13;
const GAMMA2: i64 = (8_380_417 - 1) / 88;
const ZETA: u64 = 1753;
const RHO_BYTES: usize = 32;
const ROW_BYTES: usize = 320;
const WORD_BYTES: usize = 32;
const LANES: usize = 8;
const LANE_BYTES: usize = 4;
const TR_BYTES: usize = 64;
/// A key needs at least this many large coefficients. This is a floor to refuse beneath.
const LARGE_FLOOR: usize = ROWS * COEFFICIENTS / 2;

type Poly = [u64; COEFFICIENTS];

/// Why a public key or blob must not be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlobError {
    /// The key isn't exactly 1,312 bytes.
    WrongLength,
    /// The key is degenerate, so anyone could forge signatures for it.
    Degenerate,
    /// The blob isn't exactly what this key prepares to.
    Mismatch,
}

fn power(base: u64, exponent: u32) -> u64 {
    let mut result = 1;
    for _ in 0..exponent {
        result = result * base % Q;
    }
    result
}

/// The FIPS 204 forward NTT, with coefficients in the standard order.
fn ntt(poly: &mut Poly) {
    let mut step = 0_u8;
    let mut length = COEFFICIENTS / 2;
    while length >= 1 {
        let mut start = 0;
        while start < COEFFICIENTS {
            step += 1;
            let zeta = power(ZETA, u32::from(step.reverse_bits()));
            for j in start..start + length {
                let product = zeta * poly[j + length] % Q;
                poly[j + length] = (poly[j] + Q - product) % Q;
                poly[j] = (poly[j] + product) % Q;
            }
            start += 2 * length;
        }
        length /= 2;
    }
}

/// `t1` for one row of a key, as ten-bit coefficients packed little-endian.
fn row_coefficients(public: &PublicKey, row: usize) -> [u16; COEFFICIENTS] {
    let start = RHO_BYTES + row * ROW_BYTES;
    let bytes = &public[start..start + ROW_BYTES];
    core::array::from_fn(|index| {
        let bit = index * 10;
        let low = u16::from(bytes[bit / 8]);
        let high = bytes.get(bit / 8 + 1).copied().map_or(0, u16::from);
        (low | high << 8) >> (bit % 8) & 0x3FF
    })
}

/// A polynomial of `A`, sampled in the NTT domain by rejection.
fn expand_a_entry(rho: &[u8], column: u8, row: u8) -> Poly {
    let mut shake = Shake128::default();
    shake.update(rho);
    shake.update(&[column, row]);
    let mut reader = shake.finalize_xof();
    let mut poly = [0; COEFFICIENTS];
    let mut filled = 0;
    while filled < COEFFICIENTS {
        let mut bytes = [0; 3];
        reader.read(&mut bytes);
        let value =
            u64::from(bytes[0]) | u64::from(bytes[1]) << 8 | u64::from(bytes[2] & 0x7F) << 16;
        if value < Q {
            poly[filled] = value;
            filled += 1;
        }
    }
    poly
}

/// Appends 32 words of eight 32-bit lanes, each word big-endian with lane 0 at its low end.
fn push_compact(blob: &mut Vec<u8>, poly: &Poly) {
    for lanes in poly.chunks_exact(LANES) {
        let mut word = [0; WORD_BYTES];
        for (lane, &coefficient) in lanes.iter().enumerate() {
            let end = WORD_BYTES - lane * LANE_BYTES;
            let value = u32::try_from(coefficient).unwrap_or(0);
            word[end - LANE_BYTES..end].copy_from_slice(&value.to_be_bytes());
        }
        blob.extend_from_slice(&word);
    }
}

/// The part of a blob that belongs to this key alone: `tr`, then `t1` in the NTT domain. The rest of
/// a blob is the matrix, which keys can share.
#[must_use]
pub fn key_part(public: &PublicKey) -> Vec<u8> {
    let mut part = Vec::with_capacity(KEY_PART_BYTES);
    let mut shake = Shake256::default();
    shake.update(public);
    let mut tr = vec![0; TR_BYTES];
    shake.finalize_xof().read(&mut tr);
    part.extend_from_slice(&tr);

    for row in 0..ROWS {
        let mut poly = row_coefficients(public, row).map(|value| (u64::from(value) << D) % Q);
        ntt(&mut poly);
        push_compact(&mut part, &poly);
    }
    part
}

/// The exact blob for a public key.
#[must_use]
pub fn prepare(public: &PublicKey) -> Vec<u8> {
    let mut blob = Vec::with_capacity(BLOB_BYTES);
    blob.push(0);
    blob.extend_from_slice(&key_part(public));

    let rho = &public[..RHO_BYTES];
    for row in 0_u8..4 {
        for column in 0_u8..4 {
            push_compact(&mut blob, &expand_a_entry(rho, column, row));
        }
    }
    blob
}

/// A coefficient's size once lifted back and centred around zero.
fn centred_lift(value: u16) -> i64 {
    let lifted = i64::try_from((u64::from(value) << D) % Q).unwrap_or(0);
    let modulus = i64::try_from(Q).unwrap_or(0);
    if lifted > modulus / 2 {
        lifted - modulus
    } else {
        lifted
    }
}

/// Whether a key fails the degenerate key floor: too few of its coefficients are large. Passing
/// doesn't prove a key safe. Only deriving it from a seed does that.
#[must_use]
pub fn is_degenerate(public: &PublicKey) -> bool {
    let large = (0..ROWS)
        .flat_map(|row| row_coefficients(public, row))
        .filter(|&value| centred_lift(value).abs() > GAMMA2)
        .count();
    large < LARGE_FLOOR
}

/// Checks a key given as raw bytes, and returns the blob to deploy.
///
/// # Errors
///
/// Fails if the key is the wrong length or degenerate.
pub fn prepare_checked(public: &[u8]) -> Result<Vec<u8>, BlobError> {
    let public: &PublicKey = public.try_into().map_err(|_| BlobError::WrongLength)?;
    debug_assert_eq!(public.len(), PUBLIC_KEY_BYTES);
    if is_degenerate(public) {
        return Err(BlobError::Degenerate);
    }
    Ok(prepare(public))
}

/// Checks that `blob` is byte for byte what a genuine key prepares to.
///
/// # Errors
///
/// Fails if the key is the wrong length or degenerate, or if the blob differs.
pub fn check_blob(public: &[u8], blob: &[u8]) -> Result<(), BlobError> {
    if prepare_checked(public)? == blob {
        Ok(())
    } else {
        Err(BlobError::Mismatch)
    }
}
