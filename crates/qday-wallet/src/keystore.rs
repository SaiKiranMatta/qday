//! The two key slots. Each holds a tree and a header, so a power cut always leaves one working.

use alloc::boxed::Box;

use qday_xmss::{
    HEIGHT, Hash, Hasher, KeySet, LEAVES, NODES, PublicKey, Tree, keccak256, node_slot,
};
use zeroize::Zeroize;

use crate::flash::{Flash, SECTOR, layout};
use crate::seal::Sealer;

const MAGIC: &[u8; 8] = b"QDAYKEY1";
/// A header whose seed field holds the seed masked with the chip's key.
const SEALED_MAGIC: &[u8; 8] = b"QDAYKEY2";
const SEAL_DOMAIN: &[u8] = b"QDAY-SEAL-V1";
/// Bytes of a header covered by its check.
const HEADER_BODY: usize = 144;
/// A whole header: the body, then the keccak256 of the body.
const HEADER: usize = HEADER_BODY + 32;
const ACTIVATION_AT: u32 = 256;
const ACTIVATION: usize = 32;
/// Room to retry an activation that was torn by a power cut.
const ACTIVATIONS: u32 = 8;
/// Where the nodes start in a slot, after the header sector.
const NODES_AT: u32 = SECTOR;
const NODE_BYTES: usize = NODES * 32;
/// Bytes of nodes hashed at a time. A whole number of nodes, so the root ends a chunk.
const NODE_CHUNK: usize = 1024;
const SECTORS_PER_SLOT: u32 = layout::KEY_SLOT_SIZE / SECTOR;

/// A checked header.
struct Header {
    bytes: [u8; HEADER],
}

impl Header {
    fn new(
        magic: [u8; 8],
        seed: &Hash,
        tree_number: u32,
        first_index: u32,
        root: &Hash,
        pub_seed: &Hash,
        nodes_hash: &Hash,
    ) -> Self {
        let mut bytes = [0; HEADER];
        bytes[..8].copy_from_slice(&magic);
        bytes[8..40].copy_from_slice(seed);
        bytes[40..44].copy_from_slice(&tree_number.to_be_bytes());
        bytes[44..48].copy_from_slice(&first_index.to_be_bytes());
        bytes[48..80].copy_from_slice(root);
        bytes[80..112].copy_from_slice(pub_seed);
        bytes[112..HEADER_BODY].copy_from_slice(nodes_hash);
        let check = keccak256(&[&bytes[..HEADER_BODY]]);
        bytes[HEADER_BODY..].copy_from_slice(&check);
        Self { bytes }
    }

    fn parse(bytes: [u8; HEADER], magic: [u8; 8]) -> Option<Self> {
        let valid = bytes.starts_with(&magic)
            && keccak256(&[&bytes[..HEADER_BODY]])[..] == bytes[HEADER_BODY..];
        valid.then_some(Self { bytes })
    }

    fn word(&self, at: usize) -> u32 {
        let mut word = [0; 4];
        word.copy_from_slice(&self.bytes[at..at + 4]);
        u32::from_be_bytes(word)
    }

    fn hash(&self, at: usize) -> Hash {
        let mut hash = [0; 32];
        hash.copy_from_slice(&self.bytes[at..at + 32]);
        hash
    }

    /// The seed field, which is masked in a sealed header.
    fn seed(&self) -> Hash {
        self.hash(8)
    }

    fn tree_number(&self) -> u32 {
        self.word(40)
    }

    fn first_index(&self) -> u32 {
        self.word(44)
    }

    fn key(&self) -> PublicKey {
        PublicKey {
            root: self.hash(48),
            pub_seed: self.hash(80),
        }
    }

    fn nodes_hash(&self) -> Hash {
        self.hash(112)
    }

    fn check(&self) -> &[u8] {
        &self.bytes[HEADER_BODY..]
    }

    /// An activation record, bound to this header so it can't move to another tree.
    fn activation(&self, serial: u32, counter_base: u32) -> [u8; ACTIVATION] {
        let mut record = [0; ACTIVATION];
        record[..4].copy_from_slice(&serial.to_be_bytes());
        record[4..8].copy_from_slice(&counter_base.to_be_bytes());
        let check = keccak256(&[&record[..8], self.check()]);
        record[8..].copy_from_slice(&check[..ACTIVATION - 8]);
        record
    }

    /// Reads back a record as (serial, counter base).
    fn read_activation(&self, record: &[u8]) -> Option<(u32, u32)> {
        let serial = u32::from_be_bytes(record[..4].try_into().ok()?);
        let counter_base = u32::from_be_bytes(record[4..8].try_into().ok()?);
        (self.activation(serial, counter_base)[..] == *record).then_some((serial, counter_base))
    }
}

fn slot_at(slot: u8) -> u32 {
    layout::KEY_SLOTS + u32::from(slot) * layout::KEY_SLOT_SIZE
}

/// Storage failed or holds something unusable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoreError;

/// The key that signs now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveKey {
    /// Which slot it is in.
    pub slot: u8,
    /// The tree number.
    pub tree: u32,
    /// The index the counter base maps to.
    pub first_index: u32,
    /// The counter value when this tree became active.
    pub counter_base: u32,
    /// Orders activations: the highest is active.
    pub serial: u32,
    /// The public key.
    pub key: PublicKey,
}

/// A finished tree waiting to be activated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedKey {
    /// Which slot it is in.
    pub slot: u8,
    /// The tree number.
    pub tree: u32,
    /// The public key.
    pub key: PublicKey,
}

/// What the two slots hold.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Slots {
    /// The key that signs now.
    pub active: Option<ActiveKey>,
    /// The next key, if one is ready.
    pub prepared: Option<PreparedKey>,
}

impl Slots {
    /// The slot a new tree may be written to: never the active one.
    #[must_use]
    pub fn free_slot(&self) -> u8 {
        match self.active {
            Some(active) => 1 - active.slot,
            None => 0,
        }
    }
}

/// Reads and writes the key slots.
pub struct KeyStore<F> {
    flash: F,
    sealer: Option<Box<dyn Sealer>>,
}

impl<F: Flash> KeyStore<F> {
    /// A store on `flash`.
    pub fn new(flash: F) -> Self {
        Self {
            flash,
            sealer: None,
        }
    }

    /// Seals the seed with the chip's key. A sealed store and a plain one ignore each other's
    /// keys, so a seed can't be swapped in or out of the seal by editing the flash.
    #[must_use]
    pub fn sealed_by(mut self, sealer: Box<dyn Sealer>) -> Self {
        self.sealer = Some(sealer);
        self
    }

    /// Whether seeds are sealed.
    #[must_use]
    pub fn is_sealed(&self) -> bool {
        self.sealer.is_some()
    }

    fn magic(&self) -> [u8; 8] {
        if self.is_sealed() {
            *SEALED_MAGIC
        } else {
            *MAGIC
        }
    }

    /// The mask for one key. The key's public half picks it, so no two seeds share a mask, and
    /// nothing new has to be stored. A plain store's mask is all zeros.
    fn mask(&mut self, key: &PublicKey) -> Result<Hash, StoreError> {
        let Some(sealer) = &mut self.sealer else {
            return Ok([0; 32]);
        };
        let mut message = [0; SEAL_DOMAIN.len() + 64];
        let (domain, rest) = message.split_at_mut(SEAL_DOMAIN.len());
        let (root, pub_seed) = rest.split_at_mut(32);
        domain.copy_from_slice(SEAL_DOMAIN);
        root.copy_from_slice(&key.root);
        pub_seed.copy_from_slice(&key.pub_seed);
        sealer.keyed_hash(&message).map_err(|_| StoreError)
    }

    /// Whether neither slot has ever finished a header of any kind.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be read.
    pub fn is_blank(&mut self) -> Result<bool, StoreError> {
        for slot in 0..2 {
            let mut magic = [0; MAGIC.len()];
            self.read(slot_at(slot), &mut magic)?;
            if magic.iter().any(|&byte| byte != 0xFF) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Reads both slots. A slot with any damage is treated as empty.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be read.
    pub fn scan(&mut self) -> Result<Slots, StoreError> {
        let mut slots = Slots::default();
        let mut unactivated = None;
        for slot in 0..2 {
            let Some(header) = self.valid_header(slot)? else {
                continue;
            };
            if let Some((serial, counter_base)) = self.activation_of(slot, &header)? {
                if slots.active.is_none_or(|active| serial > active.serial) {
                    slots.active = Some(ActiveKey {
                        slot,
                        tree: header.tree_number(),
                        first_index: header.first_index(),
                        counter_base,
                        serial,
                        key: header.key(),
                    });
                }
            } else {
                unactivated = Some(PreparedKey {
                    slot,
                    tree: header.tree_number(),
                    key: header.key(),
                });
            }
        }
        slots.prepared = unactivated;
        Ok(slots)
    }

    /// Reads a slot's header, if it and its tree are whole.
    fn valid_header(&mut self, slot: u8) -> Result<Option<Header>, StoreError> {
        let Some(header) = self.header(slot)? else {
            return Ok(None);
        };
        // Hashed in pieces, because the device has no room for a second copy of the tree.
        let start = slot_at(slot) + NODES_AT;
        let mut hasher = Hasher::default();
        let mut chunk = [0; NODE_CHUNK];
        let mut last = [0; 32];
        let mut left = NODE_BYTES;
        let mut offset = start;
        while left > 0 {
            let size = left.min(NODE_CHUNK);
            self.read(offset, &mut chunk[..size])?;
            hasher.update(&chunk[..size]);
            last.copy_from_slice(&chunk[size - 32..size]);
            offset += u32::try_from(size).map_err(|_| StoreError)?;
            left -= size;
        }
        let whole = hasher.finish() == header.nodes_hash() && last == header.key().root;
        Ok(whole.then_some(header))
    }

    fn header(&mut self, slot: u8) -> Result<Option<Header>, StoreError> {
        let mut bytes = [0; HEADER];
        self.read(slot_at(slot), &mut bytes)?;
        Ok(Header::parse(bytes, self.magic()))
    }

    /// The newest activation record of a slot.
    fn activation_of(
        &mut self,
        slot: u8,
        header: &Header,
    ) -> Result<Option<(u32, u32)>, StoreError> {
        let mut found: Option<(u32, u32)> = None;
        for record in 0..ACTIVATIONS {
            let mut bytes = [0; ACTIVATION];
            self.read(activation_at(slot, record), &mut bytes)?;
            if let Some(entry) = header.read_activation(&bytes)
                && found.is_none_or(|(serial, _)| entry.0 > serial)
            {
                found = Some(entry);
            }
        }
        Ok(found)
    }

    fn read(&mut self, offset: u32, buffer: &mut [u8]) -> Result<(), StoreError> {
        self.flash.read(offset, buffer).map_err(|_| StoreError)
    }

    /// Writes a tree into `slot`. The header goes last, so a cut leaves the slot empty.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be written.
    pub fn write_tree(
        &mut self,
        slot: u8,
        seed: &Hash,
        tree_number: u32,
        first_index: u32,
        tree: &Tree,
    ) -> Result<(), StoreError> {
        let keys = KeySet::derive(seed, tree_number);
        let key = PublicKey {
            root: tree.root(),
            pub_seed: *keys.pub_seed(),
        };
        // Before anything is erased, so a chip that can't seal changes nothing.
        let mut stored = self.mask(&key)?;
        xor_into(&mut stored, seed);

        let start = slot_at(slot);
        for sector in 0..SECTORS_PER_SLOT {
            self.flash
                .erase(start + sector * SECTOR)
                .map_err(|_| StoreError)?;
        }

        let nodes = tree.nodes().as_flattened();
        for (offset, chunk) in (0..)
            .step_by(SECTOR as usize)
            .zip(nodes.chunks(SECTOR as usize))
        {
            self.flash
                .write(start + NODES_AT + offset, chunk)
                .map_err(|_| StoreError)?;
        }

        let header = Header::new(
            self.magic(),
            &stored,
            tree_number,
            first_index,
            &key.root,
            &key.pub_seed,
            &keccak256(&[nodes]),
        );
        stored.zeroize();
        self.flash
            .write(start, &header.bytes)
            .map_err(|_| StoreError)
    }

    /// Makes `slot` the active key, with its counter base set to `counter_base`. One small record
    /// does the whole switch, so a cut leaves either the old key or the new one.
    ///
    /// # Errors
    ///
    /// Fails if the record can't be written.
    pub fn activate(&mut self, slot: u8, serial: u32, counter_base: u32) -> Result<(), StoreError> {
        let header = self.header(slot)?.ok_or(StoreError)?;
        for record in 0..ACTIVATIONS {
            let offset = activation_at(slot, record);
            let mut bytes = [0; ACTIVATION];
            self.read(offset, &mut bytes)?;
            if bytes.iter().all(|&byte| byte == 0xFF) {
                let data = header.activation(serial, counter_base);
                return self.flash.write(offset, &data).map_err(|_| StoreError);
            }
        }
        Err(StoreError)
    }

    /// Reads the seed stored with `slot`.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be read, or if a sealed seed doesn't open with this chip's key. A
    /// wrong seed never comes back, because keys made from one would sign for nobody.
    pub fn seed(&mut self, slot: u8) -> Result<Hash, StoreError> {
        let header = self.header(slot)?.ok_or(StoreError)?;
        if !self.is_sealed() {
            return Ok(header.seed());
        }
        let key = header.key();
        let mut seed = self.mask(&key)?;
        xor_into(&mut seed, &header.seed());
        if KeySet::derive(&seed, header.tree_number()).pub_seed() == &key.pub_seed {
            Ok(seed)
        } else {
            seed.zeroize();
            Err(StoreError)
        }
    }

    /// Reads the auth path for key `index` of `slot`.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be read.
    pub fn auth_path(&mut self, slot: u8, index: u32) -> Result<[Hash; HEIGHT], StoreError> {
        if index >= LEAVES {
            return Err(StoreError);
        }
        let mut path = [[0; 32]; HEIGHT];
        for (height, node) in (0..).zip(&mut path) {
            let position = node_slot(height, (index >> height) ^ 1);
            let position = u32::try_from(position).map_err(|_| StoreError)?;
            self.read(slot_at(slot) + NODES_AT + position * 32, node)?;
        }
        Ok(path)
    }

    /// Erases both slots, starting with the headers that hold the seed.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be erased.
    pub fn wipe(&mut self) -> Result<(), StoreError> {
        for slot in 0..2 {
            self.flash.erase(slot_at(slot)).map_err(|_| StoreError)?;
        }
        for slot in 0..2 {
            for sector in 1..SECTORS_PER_SLOT {
                self.flash
                    .erase(slot_at(slot) + sector * SECTOR)
                    .map_err(|_| StoreError)?;
            }
        }
        Ok(())
    }
}

fn xor_into(target: &mut Hash, other: &Hash) {
    for (byte, mask) in target.iter_mut().zip(other) {
        *byte ^= mask;
    }
}

fn activation_at(slot: u8, record: u32) -> u32 {
    slot_at(slot) + ACTIVATION_AT + record * 32
}
