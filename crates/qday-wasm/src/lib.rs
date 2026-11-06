//! The device's frame transaction code, compiled for the browser. The web app builds every unsigned
//! transaction with this, sends the bytes to the device to sign, and submits what this returns, so the
//! browser, the laptop tool and the device can never disagree about what was signed.

use qday_mldsa::Signer;
use qday_mldsa::blob::prepare_checked;
use qday_wallet::frame::{Action, Address, FrameTx, MlFrameTx, StealthNote};
use qday_wallet::hex;
use qday_wallet::stealth::{
    Announcement, CIPHERTEXT_BYTES, Ciphertext, MetaAddress, finds, key_for, meta_address_for,
    pay_to, signer_for, spend_hash, view_seed_for,
};
use qday_wallet::submission::{ml_submission_json, submission_json};
use qday_wallet::tokens;
use qday_xmss::{Hash, KeySet, LEAVES, Tree};
use wasm_bindgen::prelude::*;

/// The account a stealth address belongs to, as the laptop tool uses.
const ACCOUNT: u32 = 0;

fn address(text: &str) -> Result<Address, JsError> {
    hex::decode_array(text)
        .ok_or_else(|| JsError::new(&format!("{text} is not a 20-byte 0x address")))
}

fn hash(text: &str) -> Result<[u8; 32], JsError> {
    hex::decode_array(text)
        .ok_or_else(|| JsError::new(&format!("{text} is not 32 bytes of 0x hex")))
}

fn wei(text: &str) -> Result<u128, JsError> {
    text.parse()
        .map_err(|_| JsError::new(&format!("{text} is not an amount in wei")))
}

fn units(text: &str) -> Result<u128, JsError> {
    text.parse()
        .map_err(|_| JsError::new(&format!("{text} is not a whole number of token units")))
}

fn meta(text: &str) -> Result<MetaAddress, JsError> {
    hex::decode_array(text).ok_or_else(|| JsError::new("not a stealth address"))
}

fn ciphertext(text: &str) -> Result<Ciphertext, JsError> {
    hex::decode_array(text).ok_or_else(|| JsError::new("not a stealth payment's ciphertext"))
}

fn note(key_hash: &str, view_tag: u8, sealed: &str) -> Result<StealthNote, JsError> {
    let ciphertext = ciphertext(sealed)?.to_vec();
    debug_assert_eq!(ciphertext.len(), CIPHERTEXT_BYTES);
    Ok(StealthNote {
        key_hash: hash(key_hash)?,
        view_tag,
        ciphertext,
    })
}

/// An unsigned transaction of either scheme.
enum Tx {
    Xmss(FrameTx),
    Ml(MlFrameTx),
}

fn decode(raw: &[u8]) -> Result<Tx, JsError> {
    FrameTx::decode_unsigned(raw)
        .map(Tx::Xmss)
        .or_else(|_| MlFrameTx::decode_unsigned(raw).map(Tx::Ml))
        .map_err(|_| JsError::new("not an unsigned QDay transaction"))
}

/// The account's first transaction, which deploys it through `factory`. Returns the unsigned bytes.
///
/// # Errors
///
/// Fails on malformed addresses or keys.
#[wasm_bindgen(js_name = createTx)]
pub fn create_tx(
    sender: &str,
    index: u32,
    factory: &str,
    root: &str,
    pub_seed: &str,
) -> Result<Vec<u8>, JsError> {
    Ok(FrameTx::create(
        address(sender)?,
        index,
        address(factory)?,
        hash(root)?,
        hash(pub_seed)?,
    )
    .encode(&[]))
}

/// An ETH transfer. `value_wei` is a decimal string. Returns the unsigned bytes.
///
/// # Errors
///
/// Fails on malformed addresses or amounts.
#[wasm_bindgen(js_name = transferTx)]
pub fn transfer_tx(
    sender: &str,
    nonce: u64,
    index: u32,
    to: &str,
    value_wei: &str,
) -> Result<Vec<u8>, JsError> {
    Ok(FrameTx::transfer(
        address(sender)?,
        nonce,
        index,
        address(to)?,
        wei(value_wei)?,
    )
    .encode(&[]))
}

/// An ERC-20 transfer of `amount` (a decimal string, in the token's smallest unit). Returns the unsigned bytes.
///
/// # Errors
///
/// Fails on malformed addresses or amounts.
#[wasm_bindgen(js_name = tokenTransferTx)]
pub fn token_transfer_tx(
    sender: &str,
    nonce: u64,
    index: u32,
    token: &str,
    to: &str,
    amount: &str,
) -> Result<Vec<u8>, JsError> {
    Ok(FrameTx::token_transfer(
        address(sender)?,
        nonce,
        index,
        address(token)?,
        address(to)?,
        units(amount)?,
    )
    .encode(&[]))
}

/// A stealth payment from an XMSS account. Returns the unsigned bytes.
///
/// # Errors
///
/// Fails on malformed addresses, amounts or note fields.
#[wasm_bindgen(js_name = stealthPayTx)]
#[allow(clippy::too_many_arguments)]
pub fn stealth_pay_tx(
    sender: &str,
    nonce: u64,
    index: u32,
    pool: &str,
    value_wei: &str,
    key_hash: &str,
    view_tag: u8,
    sealed: &str,
) -> Result<Vec<u8>, JsError> {
    Ok(FrameTx::stealth_pay(
        address(sender)?,
        nonce,
        index,
        address(pool)?,
        wei(value_wei)?,
        note(key_hash, view_tag, sealed)?,
    )
    .encode(&[]))
}

/// An ETH transfer from an ML-DSA account. `value_wei` is a decimal string. Returns the unsigned bytes.
///
/// # Errors
///
/// Fails on malformed addresses or amounts.
#[wasm_bindgen(js_name = mlTransferTx)]
pub fn ml_transfer_tx(
    sender: &str,
    nonce: u64,
    to: &str,
    value_wei: &str,
) -> Result<Vec<u8>, JsError> {
    Ok(MlFrameTx::transfer(address(sender)?, nonce, address(to)?, wei(value_wei)?).encode(&[]))
}

/// A stealth payment from an ML-DSA account. Returns the unsigned bytes.
///
/// # Errors
///
/// Fails on malformed addresses, amounts or note fields.
#[wasm_bindgen(js_name = mlStealthPayTx)]
pub fn ml_stealth_pay_tx(
    sender: &str,
    nonce: u64,
    pool: &str,
    value_wei: &str,
    key_hash: &str,
    view_tag: u8,
    sealed: &str,
) -> Result<Vec<u8>, JsError> {
    Ok(MlFrameTx::stealth_pay(
        address(sender)?,
        nonce,
        address(pool)?,
        wei(value_wei)?,
        note(key_hash, view_tag, sealed)?,
    )
    .encode(&[]))
}

/// The key blob an ML-DSA account reads, as 0x hex.
///
/// # Errors
///
/// Fails on a key of the wrong length, or a degenerate key.
#[wasm_bindgen(js_name = mlBlob)]
pub fn ml_blob(public_key: &str) -> Result<String, JsError> {
    let public = hex::decode(public_key).ok_or_else(|| JsError::new("not an ML-DSA key"))?;
    let blob = prepare_checked(&public).map_err(|_| JsError::new("not an ML-DSA key"))?;
    Ok(hex::encode(&blob))
}

/// Makes a payment to a stealth address with 64 random bytes. Returns the note as JSON.
///
/// # Errors
///
/// Fails on a malformed address.
#[wasm_bindgen(js_name = stealthPay)]
pub fn stealth_pay(address: &str, random: &[u8]) -> Result<String, JsError> {
    let random: &[u8; 64] = random
        .try_into()
        .map_err(|_| JsError::new("a payment needs 64 random bytes"))?;
    let paid =
        pay_to(&meta(address)?, random).ok_or_else(|| JsError::new("not a stealth address"))?;
    Ok(format!(
        "{{\"keyHash\":\"{}\",\"viewTag\":{},\"ciphertext\":\"{}\"}}",
        hex::encode(&paid.key_hash),
        paid.view_tag,
        hex::encode(&paid.ciphertext)
    ))
}

/// True if an announcement is a payment to the owner of this view seed and address.
///
/// # Errors
///
/// Fails on malformed input.
#[wasm_bindgen(js_name = stealthFinds)]
pub fn stealth_finds(
    view: &str,
    address: &str,
    key_hash: &str,
    view_tag: u8,
    sealed: &str,
) -> Result<bool, JsError> {
    let announced = Announcement {
        ciphertext: ciphertext(sealed)?,
        key_hash: hash(key_hash)?,
        view_tag,
    };
    finds(&hash(view)?, &meta(address)?, &announced)
        .ok_or_else(|| JsError::new("not a stealth address"))
}

/// The key a spend gives the pool, as 0x hex.
///
/// # Errors
///
/// Fails on malformed input.
#[wasm_bindgen(js_name = stealthKey)]
pub fn stealth_key(view: &str, address: &str, sealed: &str) -> Result<String, JsError> {
    let key = key_for(&hash(view)?, &meta(address)?, &ciphertext(sealed)?)
        .ok_or_else(|| JsError::new("not a stealth address"))?;
    Ok(hex::encode(&key))
}

/// The hash the device signs for an unsigned transaction, as 0x hex.
///
/// # Errors
///
/// Fails unless `raw` is an unsigned `QDay` transaction.
#[wasm_bindgen(js_name = signingMessage)]
pub fn signing_message(raw: &[u8]) -> Result<String, JsError> {
    Ok(hex::encode(&match decode(raw)? {
        Tx::Xmss(tx) => tx.signing_message(),
        Tx::Ml(tx) => tx.sig_hash(),
    }))
}

/// The `eth_sendFrameTransaction` object for an unsigned transaction and the device's signature, as JSON.
///
/// # Errors
///
/// Fails unless `raw` is an unsigned `QDay` transaction.
#[wasm_bindgen]
pub fn submission(raw: &[u8], signature: &[u8]) -> Result<String, JsError> {
    Ok(match decode(raw)? {
        Tx::Xmss(tx) => submission_json(&tx, signature),
        Tx::Ml(tx) => ml_submission_json(&tx, signature),
    })
}

/// What an unsigned transaction does, as JSON, read back from the bytes the device will see.
///
/// # Errors
///
/// Fails unless `raw` is an unsigned `QDay` transaction.
#[wasm_bindgen]
pub fn describe(raw: &[u8]) -> Result<String, JsError> {
    Ok(match decode(raw)? {
        Tx::Xmss(tx) => describe_xmss(&tx),
        Tx::Ml(tx) => describe_ml(&tx),
    })
}

fn stealth_pay_json(pool: &Address, value: u128, note: &StealthNote) -> String {
    format!(
        "{{\"kind\":\"stealthPay\",\"pool\":\"{}\",\"valueWei\":\"{value}\",\"keyHash\":\"{}\"}}",
        hex::encode(pool),
        hex::encode(&note.key_hash)
    )
}

fn transfer_json(to: &Address, value: u128) -> String {
    format!(
        "{{\"kind\":\"transfer\",\"to\":\"{}\",\"valueWei\":\"{value}\"}}",
        hex::encode(to)
    )
}

fn describe_ml(tx: &MlFrameTx) -> String {
    let action = tx.note.as_ref().map_or_else(
        || transfer_json(&tx.to, tx.value),
        |note| stealth_pay_json(&tx.to, tx.value, note),
    );
    format!(
        "{{\"sender\":\"{}\",\"nonce\":{},\"index\":null,\"maxFeeWei\":\"{}\",\"action\":{action}}}",
        hex::encode(&tx.sender),
        tx.nonce,
        tx.max_fee_wei()
    )
}

fn describe_xmss(tx: &FrameTx) -> String {
    let action = match &tx.action {
        Action::Transfer { to, value } => transfer_json(to, *value),
        Action::TokenTransfer { token, to, amount } => {
            // The device's own token list decides what its screen shows, so the page can show the same.
            let (symbol, decimals) = tokens::known(token).map_or_else(
                || ("null".to_string(), "null".to_string()),
                |known| (format!("\"{}\"", known.symbol), known.decimals.to_string()),
            );
            format!(
                "{{\"kind\":\"token\",\"token\":\"{}\",\"to\":\"{}\",\"amount\":\"{amount}\",\"symbol\":{symbol},\"decimals\":{decimals}}}",
                hex::encode(token),
                hex::encode(to)
            )
        }
        Action::StealthPay { pool, value, note } => stealth_pay_json(pool, *value, note),
        Action::Rotate { next_root, .. } => format!(
            "{{\"kind\":\"rotate\",\"nextRoot\":\"{}\"}}",
            hex::encode(next_root)
        ),
        Action::Create { factory, root, .. } => format!(
            "{{\"kind\":\"create\",\"factory\":\"{}\",\"root\":\"{}\"}}",
            hex::encode(factory),
            hex::encode(root)
        ),
        Action::Other { target, value, .. } => format!(
            "{{\"kind\":\"other\",\"target\":{},\"valueWei\":\"{value}\"}}",
            target.map_or_else(
                || "null".to_string(),
                |t| format!("\"{}\"", hex::encode(&t))
            )
        ),
    };
    format!(
        "{{\"sender\":\"{}\",\"nonce\":{},\"index\":{},\"maxFeeWei\":\"{}\",\"action\":{action}}}",
        hex::encode(&tx.sender),
        tx.nonce,
        tx.index,
        tx.max_fee_wei()
    )
}

/// A wallet's keys held in the page, for the demo wallet. The page must keep its own one-time key
/// counter: nothing here stops it signing with a key twice.
#[wasm_bindgen]
pub struct SoftKeys {
    seed: Hash,
    keys: KeySet,
    tree: Tree,
}

#[wasm_bindgen]
impl SoftKeys {
    /// Derives the keys and builds the tree from a 32-byte seed.
    ///
    /// # Errors
    ///
    /// Fails on a seed of the wrong length.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: &str) -> Result<Self, JsError> {
        let seed: Hash =
            hex::decode_array(seed).ok_or_else(|| JsError::new("not a 32-byte seed"))?;
        let keys = KeySet::derive(&seed, 0);
        let tree = Tree::build(&keys, |_| {});
        Ok(Self { seed, keys, tree })
    }

    /// The tree's root, as 0x hex.
    #[must_use]
    pub fn root(&self) -> String {
        hex::encode(&self.tree.root())
    }

    /// The public seed, as 0x hex.
    #[wasm_bindgen(js_name = pubSeed)]
    #[must_use]
    pub fn pub_seed(&self) -> String {
        hex::encode(self.keys.pub_seed())
    }

    /// Signs a 32-byte message with one-time key `index`, as 0x hex.
    ///
    /// # Errors
    ///
    /// Fails on a bad message, or an index past the end of the tree.
    pub fn sign(&self, index: u32, message: &str) -> Result<String, JsError> {
        if index >= LEAVES {
            return Err(JsError::new("that key index is past the end of the tree"));
        }
        let signature = self
            .keys
            .sign(
                &self.tree.root(),
                index,
                &hash(message)?,
                &self.tree.auth_path(index),
            )
            .map_err(|_| JsError::new("that key index can't sign"))?;
        Ok(hex::encode(&signature))
    }

    /// The stealth address, as 0x hex.
    #[wasm_bindgen(js_name = stealthAddress)]
    #[must_use]
    pub fn stealth_address(&self) -> String {
        hex::encode(&meta_address_for(&self.seed, ACCOUNT))
    }

    /// The seed that finds payments and can't spend them, as 0x hex.
    #[wasm_bindgen(js_name = viewKey)]
    #[must_use]
    pub fn view_key(&self) -> String {
        hex::encode(&view_seed_for(&self.seed, ACCOUNT))
    }

    /// Signs a spend of a stealth note with 32 random bytes, as 0x hex. `fee` is a decimal string.
    ///
    /// # Errors
    ///
    /// Fails on malformed input, or if the note isn't this wallet's.
    #[wasm_bindgen(js_name = spendStealth)]
    pub fn spend_stealth(
        &self,
        sealed: &str,
        key_hash: &str,
        pool: &str,
        to: &str,
        fee: &str,
        random: &[u8],
    ) -> Result<String, JsError> {
        let key_hash = hash(key_hash)?;
        let random: &Hash = random
            .try_into()
            .map_err(|_| JsError::new("a spend needs 32 random bytes"))?;
        let signer = signer_for(&self.seed, ACCOUNT, &ciphertext(sealed)?, &key_hash)
            .ok_or_else(|| JsError::new("that note isn't this wallet's"))?;
        let message = spend_hash(&address(pool)?, &key_hash, &address(to)?, wei(fee)?);
        let signature = signer
            .sign(&message, random)
            .map_err(|_| JsError::new("signing failed"))?;
        Ok(hex::encode(&signature))
    }
}
