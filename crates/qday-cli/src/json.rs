//! The object `eth_sendFrameTransaction` takes.

use qday_wallet::frame::FrameTx;
use qday_wallet::submission::submission_json;
use serde_json::Value;

/// Builds the submission object for a signed transaction, from the builder the browser also uses.
pub fn submission(tx: &FrameTx, signature: &[u8]) -> Value {
    serde_json::from_str(&submission_json(tx, signature)).expect("the shared builder writes valid JSON")
}
