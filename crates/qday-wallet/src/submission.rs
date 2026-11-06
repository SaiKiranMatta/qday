//! The object `eth_sendFrameTransaction` takes, as JSON text. Shared by the laptop tool and the browser,
//! so both submit exactly what the device signed.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::frame::{Address, CHAIN_ID, FrameTx, FrameView, MlFrameTx};
use crate::hex;

fn quantity(value: u128) -> String {
    format!("\"{value:#x}\"")
}

fn string(text: &str) -> String {
    format!("\"{text}\"")
}

/// Builds the submission object for a signed transaction.
#[must_use]
pub fn submission_json(tx: &FrameTx, signature: &[u8]) -> String {
    let fees = (tx.max_priority_fee, tx.max_fee);
    build(&tx.frame_views(), &tx.sender, tx.nonce, fees, signature)
}

/// Builds the submission object for a signed two-frame transaction.
#[must_use]
pub fn ml_submission_json(tx: &MlFrameTx, signature: &[u8]) -> String {
    let fees = (tx.max_priority_fee, tx.max_fee);
    build(&tx.frame_views(), &tx.sender, tx.nonce, fees, signature)
}

fn build(
    frame_views: &[FrameView],
    sender: &Address,
    nonce: u64,
    (max_priority_fee, max_fee): (u128, u128),
    signature: &[u8],
) -> String {
    let frames: Vec<String> = frame_views
        .iter()
        .map(|frame| {
            let target = frame
                .target
                .map_or_else(|| String::from("null"), |target| string(&hex::encode(&target)));
            format!(
                "{{\"mode\":{},\"flags\":{},\"target\":{target},\"limits\":{{\"execution\":{},\"state\":{}}},\"value\":{},\"data\":{}}}",
                frame.mode,
                frame.flags,
                quantity(u128::from(frame.execution)),
                quantity(u128::from(frame.state)),
                quantity(frame.value),
                string(&hex::encode(&frame.data)),
            )
        })
        .collect();

    format!(
        "{{\"chainId\":{},\"nonceKeys\":[{}],\"nonceSeq\":{},\"sender\":{},\"frames\":[{}],\"signatures\":[{{\"scheme\":0,\"signer\":\"0x\",\"msg\":\"0x\",\"signature\":{}}}],\"fees\":{{\"maxPriorityFeePerGas\":{},\"maxFeePerGas\":{},\"maxFeePerBlobGas\":\"0x0\"}},\"blobVersionedHashes\":[],\"recentRootReferences\":[]}}",
        quantity(u128::from(CHAIN_ID)),
        quantity(0),
        quantity(u128::from(nonce)),
        string(&hex::encode(sender)),
        frames.join(","),
        string(&hex::encode(signature)),
        quantity(max_priority_fee),
        quantity(max_fee),
    )
}
