//! One-time ML-DSA-44 keys that a sender can derive and only the recipient can sign for, following
//! Spirit (ePrint 2023/1148, Figure 8). Everyone shares one matrix, and a recipient publishes `t` in
//! full. A sender turns a shared secret into small `d1` and `d2`, and computes `t + A d1 + d2`. The
//! recipient's signing secret is `s1 + d1` and `s2 + d2`.
//!
//! Those secrets are twice the usual width, so `fips204` can't sign with them. This signer is FIPS
//! 204's, with rejection bounds doubled to match, and its signatures pass the standard verifier
//! unchanged.
//!
//! This variant hasn't been reviewed by a cryptographer, and it isn't constant time. The signing
//! secret and the shared secret together give back the master secret.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "lattice arithmetic moves between fixed-width integers on purpose"
)]

use qday_xmss::Hash;
use sha3::digest::{ExtendableOutput, Update, XofReader};
use sha3::{Shake128, Shake256};
use zeroize::Zeroize;

use crate::{PUBLIC_KEY_BYTES, PublicKey, SIGNATURE_BYTES, SignError, Signature, Signer};

const N: usize = 256;
const ROWS: usize = 4;
const Q: i32 = 8_380_417;
const Q_INVERSE: i32 = 58_728_449;
const D: u32 = 13;
const TAU: usize = 39;
const GAMMA1: i32 = 1 << 17;
const GAMMA2: i32 = (Q - 1) / 88;
const OMEGA: usize = 80;
/// ML-DSA-44's beta is `TAU * 2` for a secret of width 2. A secret of width 4 doubles it.
const WIDE_BETA: i32 = 156;
const SMALL_WIDTH: u8 = 2;
/// Signing retries on rejection, and this gives up long past anything that should happen.
const MAX_ATTEMPTS: u32 = 2000;

const T_BITS: usize = 23;
const T1_ROW_BYTES: usize = 320;
const W1_ROW_BYTES: usize = 192;
const Z_ROW_BYTES: usize = 576;
const C_TILDE_BYTES: usize = 32;
const RHO_BYTES: usize = 32;
const TR_BYTES: usize = 64;
const MU_BYTES: usize = 64;

/// `2^64 / 256`, in Montgomery form, for the end of the inverse transform.
const INVERSE_SCALE: i32 = 41_978;

const MATRIX_LABEL: &[u8] = b"QDAY-STEALTH-MATRIX-V0";
const SMALL_LABEL: &[u8] = b"QDAY-STEALTH-SMALL-V0";
const SIGNING_LABEL: &[u8] = b"QDAY-STEALTH-SIGNING-V0";

/// Length of a published master key.
pub const PUBLIC_T_BYTES: usize = ROWS * N * T_BITS / 8;
/// Length of a payment's shared secret.
pub const SECRET_BYTES: usize = 64;

/// A published master key.
pub type PublicT = [u8; PUBLIC_T_BYTES];

type Poly = [i32; N];
type Vector = [Poly; ROWS];
type Matrix = [Vector; ROWS];

const fn zetas() -> [i32; N] {
    let mut table = [0; N];
    let mut index = 0;
    while index < N {
        let mut power = 1_u64;
        let mut step = 0;
        let exponent = (index as u8).reverse_bits();
        while step < exponent {
            power = power * 1753 % Q as u64;
            step += 1;
        }
        // Montgomery form, centred.
        let form = ((power << 32) % Q as u64) as i32;
        table[index] = if form > Q / 2 { form - Q } else { form };
        index += 1;
    }
    table
}

const ZETAS: [i32; N] = zetas();

fn montgomery(value: i64) -> i32 {
    let low = (value as i32).wrapping_mul(Q_INVERSE);
    ((value - i64::from(low) * i64::from(Q)) >> 32) as i32
}

/// Reduces into `[0, Q)`.
fn canonical(value: i32) -> i32 {
    value.rem_euclid(Q)
}

/// Reduces into `(-Q/2, Q/2]`.
fn centred(value: i32) -> i32 {
    let value = canonical(value);
    if value > Q / 2 { value - Q } else { value }
}

fn ntt(poly: &mut Poly) {
    let mut step = 0;
    let mut length = N / 2;
    while length > 0 {
        let mut start = 0;
        while start < N {
            step += 1;
            let zeta = i64::from(ZETAS[step]);
            for j in start..start + length {
                let product = montgomery(zeta * i64::from(poly[j + length]));
                poly[j + length] = poly[j] - product;
                poly[j] += product;
            }
            start += 2 * length;
        }
        length /= 2;
    }
}

/// The inverse transform, which also multiplies by the Montgomery factor.
fn inverse_ntt(poly: &mut Poly) {
    let mut step = N;
    let mut length = 1;
    while length < N {
        let mut start = 0;
        while start < N {
            step -= 1;
            let zeta = -i64::from(ZETAS[step]);
            for j in start..start + length {
                let held = poly[j];
                poly[j] = held + poly[j + length];
                let difference = held - poly[j + length];
                poly[j + length] = montgomery(zeta * i64::from(difference));
            }
            start += 2 * length;
        }
        length *= 2;
    }
    for value in poly.iter_mut() {
        *value = montgomery(i64::from(INVERSE_SCALE) * i64::from(*value));
    }
}

/// `a * b`, both in the NTT domain, divided by the Montgomery factor.
fn pointwise(a: &Poly, b: &Poly) -> Poly {
    core::array::from_fn(|i| montgomery(i64::from(a[i]) * i64::from(b[i])))
}

/// `c * s`, with `c` and `s` in the NTT domain, as canonical coefficients.
fn product(c: &Poly, s: &Poly) -> Poly {
    let mut out = pointwise(c, s);
    inverse_ntt(&mut out);
    out.map(canonical)
}

fn transformed(vector: &Vector) -> Vector {
    let mut out = *vector;
    for poly in &mut out {
        ntt(poly);
    }
    out
}

/// `A v`, with `v` in the NTT domain, as canonical coefficients.
fn times_matrix(matrix: &Matrix, vector: &Vector) -> Vector {
    core::array::from_fn(|row| {
        let mut sum = [0_i64; N];
        for column in 0..ROWS {
            let term = pointwise(&matrix[row][column], &vector[column]);
            for (total, value) in sum.iter_mut().zip(term) {
                *total += i64::from(value);
            }
        }
        let mut poly = sum.map(|value| (value % i64::from(Q)) as i32);
        inverse_ntt(&mut poly);
        poly.map(canonical)
    })
}

fn add(a: &Vector, b: &Vector) -> Vector {
    core::array::from_fn(|row| core::array::from_fn(|i| canonical(a[row][i] + b[row][i])))
}

fn shake256(parts: &[&[u8]], out: &mut [u8]) {
    let mut shake = Shake256::default();
    for part in parts {
        shake.update(part);
    }
    shake.finalize_xof().read(out);
}

/// The matrix seed every key shares. It comes from a public label, so nobody chose it.
#[must_use]
pub fn shared_rho() -> Hash {
    let mut rho = [0; RHO_BYTES];
    shake256(&[MATRIX_LABEL], &mut rho);
    rho
}

fn expand_a(rho: &[u8]) -> Matrix {
    let mut matrix = [[[0; N]; ROWS]; ROWS];
    for (row, polys) in matrix.iter_mut().enumerate() {
        for (column, poly) in polys.iter_mut().enumerate() {
            let mut shake = Shake128::default();
            shake.update(rho);
            shake.update(&[column as u8, row as u8]);
            let mut reader = shake.finalize_xof();
            let mut filled = 0;
            while filled < N {
                let mut bytes = [0; 3];
                reader.read(&mut bytes);
                let value = i32::from(bytes[0])
                    | i32::from(bytes[1]) << 8
                    | i32::from(bytes[2] & 0x7F) << 16;
                if value < Q {
                    poly[filled] = value;
                    filled += 1;
                }
            }
        }
    }
    matrix
}

/// A vector of coefficients in `[-2, 2]`, from a seed.
fn small_vector(seed: &[u8], tag: u8) -> Vector {
    let mut vector = [[0; N]; ROWS];
    let span = 2 * SMALL_WIDTH + 1;
    // The largest multiple of `span` a byte can hold, so every value is equally likely.
    let limit = u8::MAX - u8::MAX % span;
    for (row, poly) in vector.iter_mut().enumerate() {
        let mut shake = Shake256::default();
        shake.update(SMALL_LABEL);
        shake.update(seed);
        shake.update(&[tag, row as u8]);
        let mut reader = shake.finalize_xof();
        let mut filled = 0;
        while filled < N {
            let mut byte = [0];
            reader.read(&mut byte);
            if byte[0] < limit {
                poly[filled] = i32::from(byte[0] % span) - i32::from(SMALL_WIDTH);
                filled += 1;
            }
        }
    }
    vector
}

/// `A s1 + s2`.
fn public_vector(matrix: &Matrix, s1: &Vector, s2: &Vector) -> Vector {
    add(&times_matrix(matrix, &transformed(s1)), s2)
}

fn pack_t(t: &Vector) -> PublicT {
    let mut out = [0; PUBLIC_T_BYTES];
    let mut bit = 0;
    for &value in t.as_flattened() {
        for offset in 0..T_BITS {
            if (value >> offset) & 1 == 1 {
                out[bit / 8] |= 1 << (bit % 8);
            }
            bit += 1;
        }
    }
    out
}

fn unpack_t(bytes: &PublicT) -> Option<Vector> {
    let mut t = [[0; N]; ROWS];
    let mut bit = 0;
    for value in t.as_flattened_mut() {
        for offset in 0..T_BITS {
            *value |= i32::from(bytes[bit / 8] >> (bit % 8) & 1) << offset;
            bit += 1;
        }
        if *value >= Q {
            return None;
        }
    }
    Some(t)
}

/// Splits `t` into `t1` and `t0`.
fn power2_round(value: i32) -> (i32, i32) {
    let high = (value + (1 << (D - 1)) - 1) >> D;
    (high, value - (high << D))
}

fn encode_public(t: &Vector) -> PublicKey {
    let mut key = [0; PUBLIC_KEY_BYTES];
    key[..RHO_BYTES].copy_from_slice(&shared_rho());
    for (row, poly) in t.iter().enumerate() {
        let bytes = &mut key[RHO_BYTES + row * T1_ROW_BYTES..][..T1_ROW_BYTES];
        for (index, &value) in poly.iter().enumerate() {
            let high = power2_round(value).0;
            let bit = index * 10;
            bytes[bit / 8] |= (high << (bit % 8)) as u8;
            bytes[bit / 8 + 1] |= (high >> (8 - bit % 8)) as u8;
        }
    }
    key
}

fn one_time_public(t: &Vector, matrix: &Matrix, d1: &Vector, d2: &Vector) -> Vector {
    add(t, &public_vector(matrix, d1, d2))
}

/// Derives a payment's key from the published master key and the shared secret. This is the
/// sender's side, and it needs no secret of the recipient's. `None` if the master key is malformed.
#[must_use]
pub fn derive_public(t: &PublicT, secret: &[u8; SECRET_BYTES]) -> Option<PublicKey> {
    let t = unpack_t(t)?;
    let matrix = expand_a(&shared_rho());
    let derived = one_time_public(
        &t,
        &matrix,
        &small_vector(secret, 1),
        &small_vector(secret, 2),
    );
    Some(encode_public(&derived))
}

/// A recipient's master secret. Wiped on drop, and deliberately not `Debug`.
pub struct Master {
    seed: Hash,
    s1: Vector,
    s2: Vector,
}

impl Master {
    /// The master secret for a seed.
    #[must_use]
    pub fn from_seed(seed: &Hash) -> Self {
        Self {
            seed: *seed,
            s1: small_vector(seed, 1),
            s2: small_vector(seed, 2),
        }
    }

    /// The master key to publish. Unlike an ordinary public key, none of `t` is left out.
    #[must_use]
    pub fn public(&self) -> PublicT {
        pack_t(&public_vector(&expand_a(&shared_rho()), &self.s1, &self.s2))
    }

    /// The signer for a payment.
    #[must_use]
    pub fn one_time(&self, secret: &[u8; SECRET_BYTES]) -> OneTimeSigner {
        let d1 = small_vector(secret, 1);
        let d2 = small_vector(secret, 2);
        let wide = |a: &Vector, b: &Vector| -> Vector {
            core::array::from_fn(|row| core::array::from_fn(|i| a[row][i] + b[row][i]))
        };
        let mut key = [0; 32];
        shake256(&[SIGNING_LABEL, &self.seed, secret], &mut key);
        OneTimeSigner::new(&wide(&self.s1, &d1), &wide(&self.s2, &d2), key)
    }
}

impl Drop for Master {
    fn drop(&mut self) {
        self.seed.zeroize();
        self.s1.zeroize();
        self.s2.zeroize();
    }
}

/// Signs for one payment's key. Wiped on drop, and deliberately not `Debug`.
pub struct OneTimeSigner {
    s1: Vector,
    s2: Vector,
    t0: Vector,
    key: Hash,
    public: PublicKey,
    tr: [u8; TR_BYTES],
}

impl OneTimeSigner {
    fn new(s1: &Vector, s2: &Vector, key: Hash) -> Self {
        let matrix = expand_a(&shared_rho());
        let t = public_vector(&matrix, s1, s2);
        let public = encode_public(&t);
        let mut tr = [0; TR_BYTES];
        shake256(&[&public], &mut tr);
        Self {
            s1: *s1,
            s2: *s2,
            t0: t.map(|poly| poly.map(|value| power2_round(value).1)),
            key,
            public,
            tr,
        }
    }
}

impl Drop for OneTimeSigner {
    fn drop(&mut self) {
        self.s1.zeroize();
        self.s2.zeroize();
        self.t0.zeroize();
        self.key.zeroize();
    }
}

/// Splits a canonical coefficient into its high and low parts.
fn decompose(value: i32) -> (i32, i32) {
    let mut low = value % (2 * GAMMA2);
    if low > GAMMA2 {
        low -= 2 * GAMMA2;
    }
    if value - low == Q - 1 {
        (0, low - 1)
    } else {
        ((value - low) / (2 * GAMMA2), low)
    }
}

fn expand_mask(seed: &[u8], counter: u16) -> Poly {
    let mut bytes = [0; Z_ROW_BYTES];
    shake256(&[seed, &counter.to_le_bytes()], &mut bytes);
    core::array::from_fn(|index| {
        let bit = index * 18;
        let at = bit / 8;
        let word =
            u32::from(bytes[at]) | u32::from(bytes[at + 1]) << 8 | u32::from(bytes[at + 2]) << 16;
        GAMMA1 - ((word >> (bit % 8)) & 0x3_FFFF) as i32
    })
}

fn sample_in_ball(c_tilde: &[u8]) -> Poly {
    let mut shake = Shake256::default();
    shake.update(c_tilde);
    let mut reader = shake.finalize_xof();
    let mut sign_bytes = [0; 8];
    reader.read(&mut sign_bytes);
    let mut signs = u64::from_le_bytes(sign_bytes);
    let mut poly = [0; N];
    for index in N - TAU..N {
        let swap = loop {
            let mut byte = [0];
            reader.read(&mut byte);
            if usize::from(byte[0]) <= index {
                break usize::from(byte[0]);
            }
        };
        poly[index] = poly[swap];
        poly[swap] = 1 - 2 * (signs & 1) as i32;
        signs >>= 1;
    }
    poly
}

fn encode_w1(w1: &[[u8; N]; ROWS]) -> [u8; ROWS * W1_ROW_BYTES] {
    let mut out = [0; ROWS * W1_ROW_BYTES];
    let mut bit = 0;
    for &value in w1.as_flattened() {
        out[bit / 8] |= value << (bit % 8);
        if bit % 8 > 2 {
            out[bit / 8 + 1] |= value >> (8 - bit % 8);
        }
        bit += 6;
    }
    out
}

fn encode_signature(c_tilde: &[u8], z: &Vector, hints: &[[bool; N]; ROWS]) -> Signature {
    let mut out = [0; SIGNATURE_BYTES];
    out[..C_TILDE_BYTES].copy_from_slice(c_tilde);
    let mut bit = C_TILDE_BYTES * 8;
    for &value in z.as_flattened() {
        let packed = (GAMMA1 - value) as u32;
        for offset in 0..18 {
            if (packed >> offset) & 1 == 1 {
                out[bit / 8] |= 1 << (bit % 8);
            }
            bit += 1;
        }
    }
    let hint_at = bit / 8;
    let mut written = 0;
    for (row, poly) in hints.iter().enumerate() {
        for (index, &set) in poly.iter().enumerate() {
            if set {
                out[hint_at + written] = index as u8;
                written += 1;
            }
        }
        out[hint_at + OMEGA + row] = written as u8;
    }
    out
}

impl Signer for OneTimeSigner {
    fn public_key(&self) -> PublicKey {
        self.public
    }

    fn sign_unchecked(&self, message: &Hash, rnd: &Hash) -> Result<Signature, SignError> {
        let matrix = expand_a(&self.public[..RHO_BYTES]);
        let mut mu = [0; MU_BYTES];
        shake256(&[&self.tr, &[0, 0], message], &mut mu);
        let mut mask_seed = [0; 64];
        shake256(&[&self.key, rnd, &mu], &mut mask_seed);

        let mut s1_hat = transformed(&self.s1);
        let mut s2_hat = transformed(&self.s2);
        let t0_hat = transformed(&self.t0);

        let mut result = Err(SignError::Exhausted);
        let mut counter = 0_u16;
        for _ in 0..MAX_ATTEMPTS {
            let y: Vector =
                core::array::from_fn(|row| expand_mask(&mask_seed, counter + row as u16));
            counter += ROWS as u16;

            let w = times_matrix(&matrix, &transformed(&y));
            let w1: [[u8; N]; ROWS] =
                core::array::from_fn(|row| w[row].map(|value| decompose(value).0 as u8));
            let mut c_tilde = [0; C_TILDE_BYTES];
            shake256(&[&mu, &encode_w1(&w1)], &mut c_tilde);
            let mut c_hat = sample_in_ball(&c_tilde);
            ntt(&mut c_hat);

            let z: Vector = core::array::from_fn(|row| {
                let cs1 = product(&c_hat, &s1_hat[row]);
                core::array::from_fn(|i| centred(y[row][i] + cs1[i]))
            });
            if z.as_flattened()
                .iter()
                .any(|v| v.abs() >= GAMMA1 - WIDE_BETA)
            {
                continue;
            }

            let r: Vector = core::array::from_fn(|row| {
                let cs2 = product(&c_hat, &s2_hat[row]);
                core::array::from_fn(|i| canonical(w[row][i] - cs2[i]))
            });
            let low_too_big = r
                .as_flattened()
                .iter()
                .any(|&v| decompose(v).1.abs() >= GAMMA2 - WIDE_BETA);
            if low_too_big {
                continue;
            }

            let ct0: Vector = core::array::from_fn(|row| product(&c_hat, &t0_hat[row]));
            if ct0
                .as_flattened()
                .iter()
                .any(|&v| centred(v).abs() >= GAMMA2)
            {
                continue;
            }

            let mut hints = [[false; N]; ROWS];
            let mut weight = 0;
            for row in 0..ROWS {
                for i in 0..N {
                    let moved = canonical(r[row][i] + ct0[row][i]);
                    if decompose(r[row][i]).0 != decompose(moved).0 {
                        hints[row][i] = true;
                        weight += 1;
                    }
                }
            }
            if weight > OMEGA {
                continue;
            }

            result = Ok(encode_signature(&c_tilde, &z, &hints));
            break;
        }
        s1_hat.zeroize();
        s2_hat.zeroize();
        mask_seed.zeroize();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::{Master, OneTimeSigner, WIDE_BETA, small_vector};
    use crate::{Signer, verify};

    const SECRET: [u8; 64] = [7; 64];
    const MESSAGE: [u8; 32] = [0x11; 32];

    /// All a sender knows is the shared secret, and the key they computed for it.
    #[test]
    fn the_shared_secret_alone_cant_sign() {
        let key = Master::from_seed(&[0x42; 32])
            .one_time(&SECRET)
            .public_key();
        let thief = OneTimeSigner::new(
            &small_vector(&SECRET, 1),
            &small_vector(&SECRET, 2),
            [0; 32],
        );
        let signature = thief.sign_unchecked(&MESSAGE, &[1; 32]).expect("it signs");
        assert!(!verify(&key, &MESSAGE, &signature));
    }

    /// Without the shared secret, the master secret can't sign for a payment's key either.
    #[test]
    fn the_master_alone_cant_sign() {
        let master = Master::from_seed(&[0x42; 32]);
        let key = master.one_time(&SECRET).public_key();
        let alone = OneTimeSigner::new(&master.s1, &master.s2, [0; 32]);
        let signature = alone.sign_unchecked(&MESSAGE, &[1; 32]).expect("it signs");
        assert!(!verify(&key, &MESSAGE, &signature));
    }

    #[test]
    fn small_vectors_stay_in_range() {
        let vector = small_vector(&SECRET, 1);
        assert!(vector.as_flattened().iter().all(|value| value.abs() <= 2));
        assert!(vector.as_flattened().contains(&2));
        assert!(vector.as_flattened().contains(&-2));
    }

    #[test]
    fn a_one_time_secret_stays_inside_the_doubled_bound() {
        let signer = Master::from_seed(&[0x42; 32]).one_time(&SECRET);
        let widest = signer
            .s1
            .as_flattened()
            .iter()
            .chain(signer.s2.as_flattened())
            .map(|value| value.abs())
            .max();
        assert!(widest.is_some_and(|value| value <= 4));
        assert_eq!(WIDE_BETA, 39 * 4);
    }

    #[test]
    fn a_published_key_round_trips() {
        let t = Master::from_seed(&[0x42; 32]).public();
        let unpacked = super::unpack_t(&t).expect("it's well formed");
        assert_eq!(super::pack_t(&unpacked), t);
    }

    #[test]
    fn the_transforms_undo_each_other() {
        let mut poly = [0; 256];
        for (index, value) in poly.iter_mut().enumerate() {
            *value = index as i32 * 31 + 1;
        }
        let original = poly;
        super::ntt(&mut poly);
        super::inverse_ntt(&mut poly);
        // The inverse leaves the Montgomery factor in, so take it back out.
        let restored = poly.map(|value| super::canonical(super::montgomery(i64::from(value))));
        assert_eq!(restored, original);
    }
}
