//! The object `eth_sendFrameTransaction` takes.

use qday_wallet::frame::{Address, CHAIN_ID, FrameTx, FrameView, MlFrameTx};
use qday_wallet::hex;
use serde_json::{Value, json};

fn quantity(value: impl Into<u128>) -> String {
    format!("{:#x}", value.into())
}

/// Builds the submission object for a signed transaction.
pub fn submission(tx: &FrameTx, signature: &[u8]) -> Value {
    let fees = (tx.max_priority_fee, tx.max_fee);
    build(&tx.frame_views(), &tx.sender, tx.nonce, fees, signature)
}

/// Builds the submission object for a signed two-frame transaction.
pub fn ml_submission(tx: &MlFrameTx, signature: &[u8]) -> Value {
    let fees = (tx.max_priority_fee, tx.max_fee);
    build(&tx.frame_views(), &tx.sender, tx.nonce, fees, signature)
}

fn build(
    frame_views: &[FrameView],
    sender: &Address,
    nonce: u64,
    (max_priority_fee, max_fee): (u128, u128),
    signature: &[u8],
) -> Value {
    let frames: Vec<Value> = frame_views
        .iter()
        .map(|frame| {
            json!({
                "mode": frame.mode,
                "flags": frame.flags,
                "target": frame.target.map(|target| hex::encode(&target)),
                "limits": {
                    "execution": quantity(frame.execution),
                    "state": quantity(frame.state),
                },
                "value": quantity(frame.value),
                "data": hex::encode(&frame.data),
            })
        })
        .collect();

    json!({
        "chainId": quantity(CHAIN_ID),
        "nonceKeys": [quantity(0_u64)],
        "nonceSeq": quantity(nonce),
        "sender": hex::encode(sender),
        "frames": frames,
        "signatures": [{
            "scheme": 0,
            "signer": "0x",
            "msg": "0x",
            "signature": hex::encode(signature),
        }],
        "fees": {
            "maxPriorityFeePerGas": quantity(max_priority_fee),
            "maxFeePerGas": quantity(max_fee),
            "maxFeePerBlobGas": "0x0",
        },
        "blobVersionedHashes": [],
        "recentRootReferences": [],
    })
}
