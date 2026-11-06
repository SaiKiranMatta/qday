//! Frame transactions (EIP-8141), in the one shape `QDay` signs.

use alloc::vec::Vec;

use qday_xmss::{Hash, keccak256};

use crate::rlp::{Reader, put_bytes, put_list, put_uint};

/// The devnet's chain id.
pub const CHAIN_ID: u64 = 1337;
/// The fee used for both `max_priority_fee` and `max_fee`.
pub const DEFAULT_FEE: u128 = 2_000_000_000;

/// A 20-byte address.
pub type Address = [u8; 20];

/// The first byte of the wire encoding.
const TX_TYPE: u8 = 0x06;
const MODE_VERIFY: u64 = 1;
const MODE_SENDER: u64 = 2;
const VERIFY_FLAGS: u64 = 3;
const SCHEME_ARBITRARY: u64 = 0;
const NONCE_KEY: u64 = 0;

const VALIDATE_SELECTOR: [u8; 4] = [0x69, 0x01, 0xf6, 0x68];
const SPEND_SELECTOR: [u8; 4] = [0x92, 0xf5, 0x9d, 0x73];
const ROTATE_SELECTOR: [u8; 4] = [0xed, 0x29, 0x6e, 0x9c];
const SPEND_DATA_BYTES: usize = 36;
const ROTATE_DATA_BYTES: usize = 68;

/// (execution, state) gas limits of each frame.
const VERIFY_LIMITS: (u64, u64) = (300_000, 0);
const SPEND_LIMITS: (u64, u64) = (50_000, 100_000);
const TRANSFER_LIMITS: (u64, u64) = (50_000, 200_000);
const ROTATE_LIMITS: (u64, u64) = (80_000, 200_000);

/// "QDAY-ROTATE-V1", right-padded with zeros.
const ROTATE_TAG: Hash = *b"QDAY-ROTATE-V1\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";

/// What the third frame does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Sends `value` wei to `to`.
    Transfer {
        /// The recipient.
        to: Address,
        /// The amount in wei.
        value: u128,
    },
    /// Moves the account to a new tree.
    Rotate {
        /// The next tree's root.
        next_root: Hash,
        /// The next tree's public seed.
        next_pub_seed: Hash,
    },
    /// Anything else. The device shows a warning before signing one of these.
    Other {
        /// The call target, or `None` for the sender.
        target: Option<Address>,
        /// The wei sent with the call.
        value: u128,
        /// The call data.
        data: Vec<u8>,
        /// The frame's execution gas limit.
        execution: u64,
        /// The frame's state gas limit.
        state: u64,
    },
}

/// A `QDay` transaction: three frames, of which only the spend index and the action vary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameTx {
    /// The account.
    pub sender: Address,
    /// The transaction nonce.
    pub nonce: u64,
    /// The one-time key this transaction spends.
    pub index: u32,
    /// The third frame.
    pub action: Action,
    /// The priority fee per gas.
    pub max_priority_fee: u128,
    /// The fee cap per gas.
    pub max_fee: u128,
}

/// Why bytes are not a transaction this device will consider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodeError;

impl FrameTx {
    /// A transfer with the default fees.
    #[must_use]
    pub fn transfer(sender: Address, nonce: u64, index: u32, to: Address, value: u128) -> Self {
        Self::new(sender, nonce, index, Action::Transfer { to, value })
    }

    /// A rotation with the default fees.
    #[must_use]
    pub fn rotation(
        sender: Address,
        nonce: u64,
        index: u32,
        next_root: Hash,
        next_pub_seed: Hash,
    ) -> Self {
        Self::new(
            sender,
            nonce,
            index,
            Action::Rotate {
                next_root,
                next_pub_seed,
            },
        )
    }

    fn new(sender: Address, nonce: u64, index: u32, action: Action) -> Self {
        Self {
            sender,
            nonce,
            index,
            action,
            max_priority_fee: DEFAULT_FEE,
            max_fee: DEFAULT_FEE,
        }
    }

    /// The wire bytes with `signature` in place. An empty signature gives the unsigned form.
    #[must_use]
    pub fn encode(&self, signature: &[u8]) -> Vec<u8> {
        let mut payload = Vec::new();
        put_uint(&mut payload, u128::from(CHAIN_ID));
        let mut keys = Vec::new();
        put_uint(&mut keys, u128::from(NONCE_KEY));
        put_list(&mut payload, &keys);
        put_uint(&mut payload, u128::from(self.nonce));
        put_bytes(&mut payload, &self.sender);

        let mut frames = Vec::new();
        for frame in self.frames() {
            frame.encode(&mut frames);
        }
        put_list(&mut payload, &frames);

        let mut entry = Vec::new();
        put_uint(&mut entry, u128::from(SCHEME_ARBITRARY));
        put_bytes(&mut entry, &[]);
        put_bytes(&mut entry, &[]);
        put_bytes(&mut entry, signature);
        let mut entries = Vec::new();
        put_list(&mut entries, &entry);
        put_list(&mut payload, &entries);

        let mut fees = Vec::new();
        put_uint(&mut fees, self.max_priority_fee);
        put_uint(&mut fees, self.max_fee);
        put_uint(&mut fees, 0);
        put_list(&mut payload, &fees);
        put_list(&mut payload, &[]);
        put_list(&mut payload, &[]);

        let mut raw = Vec::with_capacity(payload.len() + 4);
        raw.push(TX_TYPE);
        put_list(&mut raw, &payload);
        raw
    }

    /// The hash of the unsigned form.
    #[must_use]
    pub fn sig_hash(&self) -> Hash {
        keccak256(&[&self.encode(&[])])
    }

    /// The hash the signature covers: the rotation message for a rotation, otherwise `sig_hash`.
    #[must_use]
    pub fn signing_message(&self) -> Hash {
        let sig_hash = self.sig_hash();
        match &self.action {
            Action::Rotate {
                next_root,
                next_pub_seed,
            } => rotation_message(&sig_hash, next_root, next_pub_seed),
            Action::Transfer { .. } | Action::Other { .. } => sig_hash,
        }
    }

    /// Reads an unsigned transaction, refusing everything but the exact `QDay` shape.
    ///
    /// Only the fields that vary are parsed. Re-encoding them must then give back the input byte
    /// for byte, which refuses any change to the fixed parts.
    ///
    /// # Errors
    ///
    /// Fails unless the bytes are the canonical encoding of an unsigned three-frame transaction.
    pub fn decode_unsigned(raw: &[u8]) -> Result<Self, DecodeError> {
        let tx = Self::parse(raw).ok_or(DecodeError)?;
        if tx.encode(&[]) == raw {
            Ok(tx)
        } else {
            Err(DecodeError)
        }
    }

    fn parse(raw: &[u8]) -> Option<Self> {
        let (&first, body) = raw.split_first()?;
        if first != TX_TYPE {
            return None;
        }
        let mut outer = Reader::new(body);
        let mut payload = outer.list().ok()?;
        outer.finish().ok()?;

        payload.uint().ok()?;
        payload.list().ok()?;
        let nonce = u64::try_from(payload.uint().ok()?).ok()?;
        let sender = Address::try_from(payload.bytes().ok()?).ok()?;

        let mut frames = payload.list().ok()?;
        ParsedFrame::read(&mut frames)?;
        let index = spend_index(&ParsedFrame::read(&mut frames)?.data)?;
        let action = ParsedFrame::read(&mut frames)?.into_action();
        frames.finish().ok()?;

        payload.list().ok()?;
        let mut fees = payload.list().ok()?;
        let max_priority_fee = fees.uint().ok()?;
        let max_fee = fees.uint().ok()?;

        Some(Self {
            sender,
            nonce,
            index,
            action,
            max_priority_fee,
            max_fee,
        })
    }

    /// The total gas limit of all three frames.
    #[must_use]
    pub fn gas_limit(&self) -> u128 {
        self.frames()
            .iter()
            .map(|frame| u128::from(frame.limits.0) + u128::from(frame.limits.1))
            .sum()
    }

    /// The most this transaction can pay in fees: `max_fee` times all frame limits.
    #[must_use]
    pub fn max_fee_wei(&self) -> u128 {
        self.max_fee.saturating_mul(self.gas_limit())
    }

    fn frames(&self) -> [Frame; 3] {
        let spend_data = spend_call(self.index);
        let third = match &self.action {
            Action::Transfer { to, value } => Frame {
                mode: MODE_SENDER,
                flags: 0,
                target: Some(*to),
                limits: TRANSFER_LIMITS,
                value: *value,
                data: Vec::new(),
            },
            Action::Rotate {
                next_root,
                next_pub_seed,
            } => Frame {
                mode: MODE_SENDER,
                flags: 0,
                target: None,
                limits: ROTATE_LIMITS,
                value: 0,
                data: rotate_call(next_root, next_pub_seed),
            },
            Action::Other {
                target,
                value,
                data,
                execution,
                state,
            } => Frame {
                mode: MODE_SENDER,
                flags: 0,
                target: *target,
                limits: (*execution, *state),
                value: *value,
                data: data.clone(),
            },
        };
        [
            Frame {
                mode: MODE_VERIFY,
                flags: VERIFY_FLAGS,
                target: None,
                limits: VERIFY_LIMITS,
                value: 0,
                data: VALIDATE_SELECTOR.to_vec(),
            },
            Frame {
                mode: MODE_SENDER,
                flags: 0,
                target: None,
                limits: SPEND_LIMITS,
                value: 0,
                data: spend_data,
            },
            third,
        ]
    }

    /// The frames in the form the submission JSON needs.
    #[must_use]
    pub fn frame_views(&self) -> [FrameView; 3] {
        self.frames().map(|frame| FrameView {
            mode: frame.mode,
            flags: frame.flags,
            target: frame.target,
            execution: frame.limits.0,
            state: frame.limits.1,
            value: frame.value,
            data: frame.data,
        })
    }
}

/// One frame's fields, for tools that present a transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameView {
    /// Frame mode.
    pub mode: u64,
    /// Frame flags.
    pub flags: u64,
    /// Call target, or `None` for the sender.
    pub target: Option<Address>,
    /// Execution gas limit.
    pub execution: u64,
    /// State gas limit.
    pub state: u64,
    /// Wei sent with the call.
    pub value: u128,
    /// Call data.
    pub data: Vec<u8>,
}

struct Frame {
    mode: u64,
    flags: u64,
    target: Option<Address>,
    limits: (u64, u64),
    value: u128,
    data: Vec<u8>,
}

impl Frame {
    fn encode(&self, out: &mut Vec<u8>) {
        let mut limits = Vec::new();
        put_uint(&mut limits, u128::from(self.limits.0));
        put_uint(&mut limits, u128::from(self.limits.1));

        let mut item = Vec::new();
        put_uint(&mut item, u128::from(self.mode));
        put_uint(&mut item, u128::from(self.flags));
        put_bytes(
            &mut item,
            self.target.as_ref().map_or(&[], |target| &target[..]),
        );
        put_list(&mut item, &limits);
        put_uint(&mut item, self.value);
        put_bytes(&mut item, &self.data);
        put_list(out, &item);
    }
}

/// A frame as read from the wire.
struct ParsedFrame<'a> {
    target: Option<Address>,
    limits: (u64, u64),
    value: u128,
    data: &'a [u8],
}

impl<'a> ParsedFrame<'a> {
    fn read(frames: &mut Reader<'a>) -> Option<Self> {
        let mut frame = frames.list().ok()?;
        frame.uint().ok()?;
        frame.uint().ok()?;
        let target = match frame.bytes().ok()? {
            [] => None,
            bytes => Some(Address::try_from(bytes).ok()?),
        };
        let mut limits = frame.list().ok()?;
        let execution = u64::try_from(limits.uint().ok()?).ok()?;
        let state = u64::try_from(limits.uint().ok()?).ok()?;
        limits.finish().ok()?;
        let value = frame.uint().ok()?;
        let data = frame.bytes().ok()?;
        frame.finish().ok()?;
        Some(Self {
            target,
            limits: (execution, state),
            value,
            data,
        })
    }

    /// Names the third frame. Its mode and flags are checked by the re-encoding.
    fn into_action(self) -> Action {
        if let (Some(to), true, TRANSFER_LIMITS) = (self.target, self.data.is_empty(), self.limits)
        {
            return Action::Transfer {
                to,
                value: self.value,
            };
        }
        if let Some((next_root, next_pub_seed)) = self.rotate_key() {
            return Action::Rotate {
                next_root,
                next_pub_seed,
            };
        }
        Action::Other {
            target: self.target,
            value: self.value,
            data: self.data.to_vec(),
            execution: self.limits.0,
            state: self.limits.1,
        }
    }

    fn rotate_key(&self) -> Option<(Hash, Hash)> {
        let is_rotate = self.target.is_none()
            && self.value == 0
            && self.limits == ROTATE_LIMITS
            && self.data.len() == ROTATE_DATA_BYTES
            && self.data.starts_with(&ROTATE_SELECTOR);
        if !is_rotate {
            return None;
        }
        let next_root = Hash::try_from(&self.data[4..36]).ok()?;
        let next_pub_seed = Hash::try_from(&self.data[36..]).ok()?;
        Some((next_root, next_pub_seed))
    }
}

/// Reads the index from a `spendIndex` call. It must fit in 32 bits.
fn spend_index(data: &[u8]) -> Option<u32> {
    if data.len() != SPEND_DATA_BYTES || !data.starts_with(&SPEND_SELECTOR) {
        return None;
    }
    let (high, low) = data[4..].split_at(28);
    if high.iter().any(|&byte| byte != 0) {
        return None;
    }
    Some(u32::from_be_bytes(low.try_into().ok()?))
}

fn spend_call(index: u32) -> Vec<u8> {
    let mut data = Vec::with_capacity(SPEND_DATA_BYTES);
    data.extend_from_slice(&SPEND_SELECTOR);
    data.extend_from_slice(&[0; 28]);
    data.extend_from_slice(&index.to_be_bytes());
    data
}

fn rotate_call(next_root: &Hash, next_pub_seed: &Hash) -> Vec<u8> {
    let mut data = Vec::with_capacity(ROTATE_DATA_BYTES);
    data.extend_from_slice(&ROTATE_SELECTOR);
    data.extend_from_slice(next_root);
    data.extend_from_slice(next_pub_seed);
    data
}

/// The message a rotation signs: `keccak256(tag || sig_hash || next_root || next_pub_seed)`.
#[must_use]
pub fn rotation_message(sig_hash: &Hash, next_root: &Hash, next_pub_seed: &Hash) -> Hash {
    keccak256(&[&ROTATE_TAG, sig_hash, next_root, next_pub_seed])
}
