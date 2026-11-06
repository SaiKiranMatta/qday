//! ML-DSA-44 (FIPS 204, pure mode, empty context) behind a small API. This is the only crate that
//! may use `fips204`, so a custom signer can replace it without touching the wallet.

#![no_std]

extern crate alloc;

pub mod blob;

use fips204::ml_dsa_44;
use fips204::traits::{KeyGen, SerDes, Signer as _, Verifier as _};
use qday_xmss::{Hash, keccak256};
use zeroize::Zeroize;

/// Length of a public key.
pub const PUBLIC_KEY_BYTES: usize = 1312;
/// Length of a signature.
pub const SIGNATURE_BYTES: usize = 2420;

/// An encoded public key.
pub type PublicKey = [u8; PUBLIC_KEY_BYTES];
/// An encoded signature.
pub type Signature = [u8; SIGNATURE_BYTES];

/// "QDAY-MLDSA-KEY-V1", right-padded with zeros.
const KEY_TAG: Hash = *b"QDAY-MLDSA-KEY-V1\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
/// Pure mode with an empty context.
const CONTEXT: &[u8] = &[];

/// Why a signature wasn't produced. None of these are a crash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignError {
    /// Signing gave up after its attempt limit.
    Exhausted,
    /// The signature didn't verify, so it was held back as a possible fault.
    FaultDetected,
}

/// The 32 bytes a key is generated from. Wiped on drop, and deliberately not `Debug`.
pub struct KeySeed(Hash);

impl KeySeed {
    /// The key seed for `account` of a master seed.
    #[must_use]
    pub fn derive(seed: &Hash, account: u32) -> Self {
        Self(keccak256(&[&KEY_TAG, seed, &account.to_be_bytes()]))
    }

    /// Wraps a key seed directly.
    #[must_use]
    pub fn from_bytes(bytes: Hash) -> Self {
        Self(bytes)
    }

    /// The bytes, for checking against published vectors.
    #[must_use]
    pub fn expose(&self) -> &Hash {
        &self.0
    }
}

impl Drop for KeySeed {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Something that can sign. A custom signer implements `public_key` and `sign_unchecked`, and gets
/// the fault guard in `sign` for free.
pub trait Signer {
    /// The public key.
    fn public_key(&self) -> PublicKey;

    /// Signs without checking the result. Callers should use `sign`.
    ///
    /// # Errors
    ///
    /// Fails if signing gives up.
    fn sign_unchecked(&self, message: &Hash, rnd: &Hash) -> Result<Signature, SignError>;

    /// Signs `message` with fresh randomness `rnd`, and verifies the result before releasing it.
    ///
    /// # Errors
    ///
    /// Fails if signing gives up, or if the signature doesn't verify.
    fn sign(&self, message: &Hash, rnd: &Hash) -> Result<Signature, SignError> {
        let signature = self.sign_unchecked(message, rnd)?;
        if verify(&self.public_key(), message, &signature) {
            Ok(signature)
        } else {
            Err(SignError::FaultDetected)
        }
    }
}

/// The standard FIPS 204 signer. The secret key wipes itself on drop.
pub struct StandardSigner {
    secret: ml_dsa_44::PrivateKey,
    public: PublicKey,
}

impl StandardSigner {
    /// Generates the key for a key seed.
    #[must_use]
    pub fn from_key_seed(key_seed: &KeySeed) -> Self {
        let (public, secret) = ml_dsa_44::KG::keygen_from_seed(&key_seed.0);
        Self {
            secret,
            public: public.into_bytes(),
        }
    }
}

impl Signer for StandardSigner {
    fn public_key(&self) -> PublicKey {
        self.public
    }

    fn sign_unchecked(&self, message: &Hash, rnd: &Hash) -> Result<Signature, SignError> {
        self.secret
            .try_sign_with_seed(rnd, message, CONTEXT)
            .map_err(|_| SignError::Exhausted)
    }
}

/// Checks a signature. Anything malformed is simply not valid.
#[must_use]
pub fn verify(public: &PublicKey, message: &Hash, signature: &Signature) -> bool {
    ml_dsa_44::PublicKey::try_from_bytes(*public)
        .is_ok_and(|key| key.verify(message, signature, CONTEXT))
}
