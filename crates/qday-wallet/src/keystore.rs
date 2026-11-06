//! The two key slots. Each holds a tree and a header, so a power cut always leaves one working.

use alloc::vec;

use qday_xmss::{HEIGHT, Hash, KeySet, LEAVES, NODES, PublicKey, Tree, keccak256, node_slot};

use crate::flash::{Flash, SECTOR, layout};

const MAGIC: &[u8; 8] = b"QDAYKEY1";
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
const SECTORS_PER_SLOT: u32 = layout::KEY_SLOT_SIZE / SECTOR;

/// A checked header.
struct Header {
    bytes: [u8; HEADER],
}

impl Header {
    fn new(
        seed: &Hash,
        tree_number: u32,
        first_index: u32,
        root: &Hash,
        pub_seed: &Hash,
        nodes_hash: &Hash,
    ) -> Self {
        let mut bytes = [0; HEADER];
        bytes[..8].copy_from_slice(MAGIC);
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

    fn parse(bytes: [u8; HEADER]) -> Option<Self> {
        let valid = bytes.starts_with(MAGIC)
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
}

impl<F: Flash> KeyStore<F> {
    /// A store on `flash`.
    pub fn new(flash: F) -> Self {
        Self { flash }
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
        let mut nodes = vec![0; NODE_BYTES];
        self.read(slot_at(slot) + NODES_AT, &mut nodes)?;
        let root_at = NODE_BYTES - 32;
        let whole =
            keccak256(&[&nodes]) == header.nodes_hash() && nodes[root_at..] == header.key().root;
        Ok(whole.then_some(header))
    }

    fn header(&mut self, slot: u8) -> Result<Option<Header>, StoreError> {
        let mut bytes = [0; HEADER];
        self.read(slot_at(slot), &mut bytes)?;
        Ok(Header::parse(bytes))
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

        let keys = KeySet::derive(seed, tree_number);
        let header = Header::new(
            seed,
            tree_number,
            first_index,
            &tree.root(),
            keys.pub_seed(),
            &keccak256(&[nodes]),
        );
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
    /// Fails if the flash can't be read.
    pub fn seed(&mut self, slot: u8) -> Result<Hash, StoreError> {
        Ok(self.header(slot)?.ok_or(StoreError)?.seed())
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

fn activation_at(slot: u8, record: u32) -> u32 {
    slot_at(slot) + ACTIVATION_AT + record * 32
}
