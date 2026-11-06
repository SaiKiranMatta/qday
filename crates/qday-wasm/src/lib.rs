//! The device's frame transaction code, compiled for the browser. The web app builds every unsigned
//! transaction with this, sends the bytes to the device to sign, and submits what this returns, so the
//! browser, the laptop tool and the device can never disagree about what was signed.

use qday_wallet::frame::{Action, Address, FrameTx};
use qday_wallet::hex;
use qday_wallet::submission::submission_json;
use qday_wallet::tokens;
use wasm_bindgen::prelude::*;

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

fn decode(raw: &[u8]) -> Result<FrameTx, JsError> {
    FrameTx::decode_unsigned(raw).map_err(|_| JsError::new("not an unsigned QDay transaction"))
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

/// The hash the device signs for an unsigned transaction, as 0x hex.
///
/// # Errors
///
/// Fails unless `raw` is an unsigned `QDay` transaction.
#[wasm_bindgen(js_name = signingMessage)]
pub fn signing_message(raw: &[u8]) -> Result<String, JsError> {
    Ok(hex::encode(&decode(raw)?.signing_message()))
}

/// The `eth_sendFrameTransaction` object for an unsigned transaction and the device's signature, as JSON.
///
/// # Errors
///
/// Fails unless `raw` is an unsigned `QDay` transaction.
#[wasm_bindgen]
pub fn submission(raw: &[u8], signature: &[u8]) -> Result<String, JsError> {
    Ok(submission_json(&decode(raw)?, signature))
}

/// What an unsigned transaction does, as JSON, read back from the bytes the device will see.
///
/// # Errors
///
/// Fails unless `raw` is an unsigned `QDay` transaction.
#[wasm_bindgen]
pub fn describe(raw: &[u8]) -> Result<String, JsError> {
    let tx = decode(raw)?;
    let action = match &tx.action {
        Action::Transfer { to, value } => {
            format!(
                "{{\"kind\":\"transfer\",\"to\":\"{}\",\"valueWei\":\"{value}\"}}",
                hex::encode(to)
            )
        }
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
        Action::StealthPay { pool, value, note } => format!(
            "{{\"kind\":\"stealthPay\",\"pool\":\"{}\",\"valueWei\":\"{value}\",\"keyHash\":\"{}\"}}",
            hex::encode(pool),
            hex::encode(&note.key_hash)
        ),
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
    Ok(format!(
        "{{\"sender\":\"{}\",\"nonce\":{},\"index\":{},\"maxFeeWei\":\"{}\",\"action\":{action}}}",
        hex::encode(&tx.sender),
        tx.nonce,
        tx.index,
        tx.max_fee_wei()
    ))
}
