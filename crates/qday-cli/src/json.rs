//! The object `eth_sendFrameTransaction` takes.

use qday_wallet::frame::{CHAIN_ID, FrameTx};
use qday_wallet::hex;
use serde_json::{Value, json};

fn quantity(value: impl Into<u128>) -> String {
    format!("{:#x}", value.into())
}

/// Builds the submission object for a signed transaction.
pub fn submission(tx: &FrameTx, signature: &[u8]) -> Value {
    let frames: Vec<Value> = tx
        .frame_views()
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
        "nonceSeq": quantity(tx.nonce),
        "sender": hex::encode(&tx.sender),
        "frames": frames,
        "signatures": [{
            "scheme": 0,
            "signer": "0x",
            "msg": "0x",
            "signature": hex::encode(signature),
        }],
        "fees": {
            "maxPriorityFeePerGas": quantity(tx.max_priority_fee),
            "maxFeePerGas": quantity(tx.max_fee),
            "maxFeePerBlobGas": "0x0",
        },
        "blobVersionedHashes": [],
        "recentRootReferences": [],
    })
}
