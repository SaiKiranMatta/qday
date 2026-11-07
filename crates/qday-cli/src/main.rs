//! The `qday` laptop tool.

mod device;
mod json;
mod lockdown;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};
use qday_mldsa::blob::{BlobError, KEY_PART_BYTES, prepare, prepare_checked};
use qday_mldsa::stealth::Master;
use qday_mldsa::{KeySeed, Signer, StandardSigner};
use qday_wallet::backup::words_to_seed;
use qday_wallet::frame::{Address, FrameTx, MlFrameTx};
use qday_wallet::hex;
use qday_wallet::protocol::scheme_name;
use qday_wallet::stealth::{
    Announcement, Ciphertext, MetaAddress, StealthSpend, finds, key_for, meta_address_for, pay_to,
    signer_for, spend_hash, view_seed_for,
};
use qday_xmss::{Hash, KeySet, Tree};

#[derive(Parser)]
#[command(name = "qday", about = "Laptop tool for the QDay wallet")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print a tree's root and public seed
    Pubkey {
        seed: String,
        #[arg(long, default_value_t = 0)]
        tree: u32,
    },
    /// Sign a 32-byte message with a laptop key
    Sign {
        seed: String,
        index: u32,
        message: String,
        #[arg(long, default_value_t = 0)]
        tree: u32,
    },
    /// Turn 24 backup words into the seed
    WordsToSeed { words: Vec<String> },
    /// Print a transfer's sig hash
    FrameHash(Transfer),
    /// Print a signed transfer as submission JSON
    FrameJson(SignedTransfer),
    /// Print a signed transfer as wire bytes
    FrameRaw(SignedTransfer),
    /// Print a rotation's sig hash
    FrameRotateHash(Rotation),
    /// Print the message a rotation signs
    FrameRotateMessage(Rotation),
    /// Print a signed rotation as submission JSON
    FrameRotateJson(SignedRotation),
    /// Print a signed rotation as wire bytes
    FrameRotateRaw(SignedRotation),
    #[command(flatten)]
    Ml(MlCommand),
    /// Print the sig hash of an account's first transaction, which deploys it
    FrameCreateHash(Create),
    /// Print a signed account creation as submission JSON
    FrameCreateJson(SignedCreate),
    /// Create the device's account: sign its deploying transaction on the device and print submission JSON
    DeviceCreate {
        port: String,
        /// The account's address, from the factory's accountAddress(root, pubSeed)
        sender: String,
        factory: String,
    },
    /// Print the device's keys, then any prepared next key
    DevicePubkey { port: String },
    #[command(flatten)]
    Stealth(StealthCommand),
    #[command(flatten)]
    Lockdown(LockdownCommand),
    /// Print the device's signing scheme
    DeviceScheme { port: String },
    /// Print the device's stored ML-DSA public key
    DeviceMlpubkey { port: String },
    /// Sign a two-frame transfer on the device and print submission JSON
    DeviceMlSend(DeviceTransfer),
    /// Print what the device logs
    DeviceLog { port: String, seconds: u64 },
    /// Sign a transfer on the device and print submission JSON
    DeviceSend(DeviceTransfer),
    /// Sign a rotation on the device and print submission JSON
    DeviceRotate {
        port: String,
        sender: String,
        nonce: u64,
    },
    /// Build the next tree on the device
    DevicePrepare { port: String },
    /// Switch the device to a prepared tree
    DeviceActivate { port: String, tree: u32 },
    /// Replace the keys on the device
    DeviceRestore {
        port: String,
        seed: String,
        tree: u32,
        chain_next_index: u32,
    },
}

/// ML-DSA keys and transfers.
#[derive(Subcommand)]
enum LockdownCommand {
    /// Report the chip's security fuses. This only reads them
    DeviceLockdown { port: String },
    /// Print every lockdown command without running any
    LockdownPlan {
        #[command(flatten)]
        inputs: lockdown::Inputs,
        #[arg(long)]
        port: String,
    },
    /// Make the keys and images. Nothing reaches a chip
    LockdownPrepare(lockdown::Inputs),
    /// Run the burn against simulated fuses. Nothing reaches a chip
    LockdownRehearse {
        #[command(flatten)]
        inputs: lockdown::Inputs,
        /// The file that holds the simulated fuses
        #[arg(long)]
        fuses: std::path::PathBuf,
    },
    /// Prototype: don't run this unless you explicitly mean to. Locks a real chip for good
    LockdownBurn {
        #[command(flatten)]
        inputs: lockdown::Inputs,
        #[arg(long)]
        port: String,
        /// The chip's MAC address, so only the chip you mean can be locked
        #[arg(long)]
        mac: String,
    },
}

fn run_lockdown(command: LockdownCommand) -> Result<String> {
    match command {
        LockdownCommand::DeviceLockdown { port } => {
            let (fuses, sealed) = device::open(&port)?.lockdown()?;
            Ok(lockdown::report(&fuses, sealed))
        }
        LockdownCommand::LockdownPlan { inputs, port } => Ok(lockdown::plan(&inputs, &port)),
        LockdownCommand::LockdownPrepare(inputs) => lockdown::prepare(&inputs),
        LockdownCommand::LockdownRehearse { inputs, fuses } => lockdown::rehearse(&inputs, &fuses),
        LockdownCommand::LockdownBurn { inputs, port, mac } => lockdown::burn(&inputs, &port, &mac),
    }
}

#[derive(Subcommand)]
enum MlCommand {
    /// Print an ML-DSA public key
    MldsaPubkey {
        seed: String,
        #[arg(long, default_value_t = 0)]
        account: u32,
    },
    /// Sign a 32-byte message with an ML-DSA laptop key
    MldsaSign {
        seed: String,
        message: String,
        rnd: String,
        #[arg(long, default_value_t = 0)]
        account: u32,
    },
    /// Check an ML-DSA public key and print the blob to deploy
    MldsaBlob { pk: String },
    /// Print a two-frame transfer's sig hash
    MlFrameHash(MlTransfer),
    /// Print a signed two-frame transfer as submission JSON
    MlFrameJson(SignedMlTransfer),
    /// Print a signed two-frame transfer as wire bytes
    MlFrameRaw(SignedMlTransfer),
}

fn run_ml(command: MlCommand) -> Result<String> {
    match command {
        MlCommand::MldsaPubkey { seed, account } => {
            let key_seed = KeySeed::derive(&hash(&seed)?, account);
            Ok(hex::encode(
                &StandardSigner::from_key_seed(&key_seed).public_key(),
            ))
        }
        MlCommand::MldsaSign {
            seed,
            message,
            rnd,
            account,
        } => {
            let key_seed = KeySeed::derive(&hash(&seed)?, account);
            let signature = StandardSigner::from_key_seed(&key_seed)
                .sign(&hash(&message)?, &hash(&rnd)?)
                .map_err(|error| anyhow!("signing failed: {error:?}"))?;
            Ok(hex::encode(&signature))
        }
        MlCommand::MldsaBlob { pk } => mldsa_blob(&pk),
        MlCommand::MlFrameHash(transfer) => Ok(hex::encode(&transfer.tx()?.sig_hash())),
        MlCommand::MlFrameJson(args) => {
            let tx = args.transfer.tx()?;
            Ok(json::ml_submission(&tx, &signature(&args.signature)?).to_string())
        }
        MlCommand::MlFrameRaw(args) => Ok(hex::encode(
            &args.transfer.tx()?.encode(&signature(&args.signature)?),
        )),
    }
}

/// Stealth payments through the lattice pool.
#[derive(Subcommand)]
enum StealthCommand {
    /// Print the shared matrix's data contract
    StealthMatrix,
    /// Print the stealth address for a laptop key
    StealthAddress { seed: String },
    /// Print the view seed for a laptop key
    StealthView { seed: String },
    /// Make a payment to an address: prints the key hash, view tag and ciphertext
    StealthPay { address: String, random: String },
    /// Print the key hash and ciphertext of each announcement in a file that's for this view seed
    StealthScan {
        view: String,
        address: String,
        file: String,
    },
    /// Print the key a spend gives the pool
    StealthKey {
        view: String,
        address: String,
        ciphertext: String,
    },
    /// Sign a spend of a stealth note with a laptop key
    StealthSign {
        seed: String,
        #[command(flatten)]
        spend: StealthNote,
        rnd: String,
    },
    /// Ask the device for its stealth address
    DeviceStealthAddress { port: String },
    /// Ask the device for its view seed
    DeviceViewKey { port: String },
    /// Sign a spend of a stealth note on the device
    DeviceStealthSpend {
        port: String,
        #[command(flatten)]
        spend: StealthNote,
    },
}

#[derive(Args)]
struct StealthNote {
    ciphertext: String,
    key_hash: String,
    pool: String,
    to: String,
    fee_wei: u128,
}

impl StealthNote {
    fn parsed(&self) -> Result<StealthSpend> {
        Ok(StealthSpend {
            ciphertext: ciphertext(&self.ciphertext)?,
            key_hash: hash(&self.key_hash)?,
            pool: address(&self.pool)?,
            to: address(&self.to)?,
            fee: self.fee_wei,
        })
    }
}

fn ciphertext(text: &str) -> Result<Ciphertext> {
    hex::decode_array(text).context("a ciphertext is 1,120 bytes of 0x-prefixed hex")
}

fn meta_address(text: &str) -> Result<MetaAddress> {
    hex::decode_array(text).context("a stealth address is 4,160 bytes of 0x-prefixed hex")
}

/// The data contract holding the shared matrix: a zero byte, then the matrix.
fn matrix() -> String {
    let public = Master::from_seed(&[0; 32]).one_time(&[0; 64]).public_key();
    let blob = prepare(&public);
    let mut contract = vec![0];
    contract.extend_from_slice(&blob[1 + KEY_PART_BYTES..]);
    hex::encode(&contract)
}

/// Finds the announcements in a file that are for this view seed. Each line holds a key hash, a view
/// tag and a ciphertext. Other people's payments fail the tag almost every time, which saves
/// deriving a key for them.
fn scan(view: &str, meta: &str, file: &str) -> Result<String> {
    let (view, meta) = (hash(view)?, meta_address(meta)?);
    let text = std::fs::read_to_string(file).context("couldn't read the announcements")?;
    let mut found = Vec::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let [key_hash, tag, text] = fields[..] else {
            bail!("an announcement is a key hash, a view tag and a ciphertext");
        };
        let announced = Announcement {
            ciphertext: ciphertext(text)?,
            key_hash: hash(key_hash)?,
            view_tag: tag.parse()?,
        };
        if finds(&view, &meta, &announced).context("the stealth address is malformed")? {
            found.push(format!("{} {text}", hex::encode(&announced.key_hash)));
        }
    }
    Ok(found.join("\n"))
}

fn run_stealth(command: StealthCommand) -> Result<String> {
    match command {
        StealthCommand::StealthMatrix => Ok(matrix()),
        StealthCommand::StealthAddress { seed } => {
            Ok(hex::encode(&meta_address_for(&hash(&seed)?, 0)))
        }
        StealthCommand::StealthView { seed } => Ok(hex::encode(&view_seed_for(&hash(&seed)?, 0))),
        StealthCommand::StealthPay { address, random } => {
            let random: [u8; 64] =
                hex::decode_array(&random).context("the randomness is 64 bytes of hex")?;
            let paid = pay_to(&meta_address(&address)?, &random)
                .context("the stealth address is malformed")?;
            Ok(format!(
                "{} {} {}",
                hex::encode(&paid.key_hash),
                paid.view_tag,
                hex::encode(&paid.ciphertext)
            ))
        }
        StealthCommand::StealthScan {
            view,
            address,
            file,
        } => scan(&view, &address, &file),
        StealthCommand::StealthKey {
            view,
            address,
            ciphertext: sealed,
        } => {
            let key = key_for(
                &hash(&view)?,
                &meta_address(&address)?,
                &ciphertext(&sealed)?,
            )
            .context("the stealth address is malformed")?;
            Ok(hex::encode(&key))
        }
        StealthCommand::StealthSign { seed, spend, rnd } => {
            let spend = spend.parsed()?;
            let signer = signer_for(&hash(&seed)?, 0, &spend.ciphertext, &spend.key_hash)
                .context("that ciphertext isn't for that key")?;
            let message = spend_hash(&spend.pool, &spend.key_hash, &spend.to, spend.fee);
            let signature = signer
                .sign(&message, &hash(&rnd)?)
                .map_err(|error| anyhow!("signing failed: {error:?}"))?;
            Ok(hex::encode(&signature))
        }
        StealthCommand::DeviceStealthAddress { port } => {
            Ok(hex::encode(&device::open(&port)?.stealth_address()?))
        }
        StealthCommand::DeviceViewKey { port } => {
            Ok(hex::encode(&device::open(&port)?.view_key()?))
        }
        StealthCommand::DeviceStealthSpend { port, spend } => {
            let signature = device::open(&port)?.spend_stealth(&spend.parsed()?)?;
            Ok(hex::encode(&signature))
        }
    }
}

#[derive(Args)]
struct DeviceTransfer {
    port: String,
    sender: String,
    nonce: u64,
    to: String,
    value_wei: u128,
}

#[derive(Args)]
struct Transfer {
    sender: String,
    nonce: u64,
    index: u32,
    to: String,
    value_wei: u128,
}

#[derive(Args)]
struct SignedTransfer {
    #[command(flatten)]
    transfer: Transfer,
    signature: String,
}

#[derive(Args)]
struct Create {
    /// The account's address, from the factory's accountAddress(root, pubSeed)
    sender: String,
    index: u32,
    factory: String,
    root: String,
    pub_seed: String,
}

#[derive(Args)]
struct SignedCreate {
    #[command(flatten)]
    create: Create,
    signature: String,
}

impl Create {
    fn tx(&self) -> Result<FrameTx> {
        Ok(FrameTx::create(
            address(&self.sender)?,
            self.index,
            address(&self.factory)?,
            hash(&self.root)?,
            hash(&self.pub_seed)?,
        ))
    }
}

#[derive(Args)]
struct Rotation {
    sender: String,
    nonce: u64,
    index: u32,
    next_root: String,
    next_pub_seed: String,
}

#[derive(Args)]
struct SignedRotation {
    #[command(flatten)]
    rotation: Rotation,
    signature: String,
}

#[derive(Args)]
struct MlTransfer {
    sender: String,
    nonce: u64,
    to: String,
    value_wei: u128,
}

#[derive(Args)]
struct SignedMlTransfer {
    #[command(flatten)]
    transfer: MlTransfer,
    signature: String,
}

impl MlTransfer {
    fn tx(&self) -> Result<MlFrameTx> {
        Ok(MlFrameTx::transfer(
            address(&self.sender)?,
            self.nonce,
            address(&self.to)?,
            self.value_wei,
        ))
    }
}

impl Transfer {
    fn tx(&self) -> Result<FrameTx> {
        Ok(FrameTx::transfer(
            address(&self.sender)?,
            self.nonce,
            self.index,
            address(&self.to)?,
            self.value_wei,
        ))
    }
}

impl Rotation {
    fn tx(&self) -> Result<FrameTx> {
        Ok(FrameTx::rotation(
            address(&self.sender)?,
            self.nonce,
            self.index,
            hash(&self.next_root)?,
            hash(&self.next_pub_seed)?,
        ))
    }
}

fn address(text: &str) -> Result<Address> {
    hex::decode_array(text).with_context(|| format!("{text} is not a 20-byte address"))
}

fn hash(text: &str) -> Result<Hash> {
    hex::decode_array(text).with_context(|| format!("{text} is not 32 bytes of hex"))
}

fn signature(text: &str) -> Result<Vec<u8>> {
    hex::decode(text).context("the signature is not hex")
}

fn pubkey(seed: &str, tree: u32) -> Result<String> {
    let keys = KeySet::derive(&hash(seed)?, tree);
    let built = Tree::build(&keys, |_| {});
    Ok(format!(
        "{} {}",
        hex::encode(&built.root()),
        hex::encode(keys.pub_seed())
    ))
}

fn sign(seed: &str, index: u32, message: &str, tree: u32) -> Result<String> {
    let keys = KeySet::derive(&hash(seed)?, tree);
    let built = Tree::build(&keys, |_| {});
    if index >= qday_xmss::LEAVES {
        bail!("key {index} is not in the tree");
    }
    let signature = keys
        .sign(
            &built.root(),
            index,
            &hash(message)?,
            &built.auth_path(index),
        )
        .map_err(|error| anyhow!("signing failed: {error:?}"))?;
    Ok(hex::encode(&signature))
}

fn mldsa_blob(pk: &str) -> Result<String> {
    let key = hex::decode(pk).context("the key is not hex")?;
    let blob = prepare_checked(&key).map_err(|error| match error {
        BlobError::WrongLength => anyhow!("a key must be exactly 1,312 bytes, not {}", key.len()),
        BlobError::Degenerate => anyhow!("this is a degenerate key, so anyone could forge for it"),
        BlobError::Mismatch => anyhow!("the blob doesn't match the key"),
    })?;
    Ok(hex::encode(&blob))
}

fn run(command: Command) -> Result<String> {
    match command {
        Command::Pubkey { seed, tree } => pubkey(&seed, tree),
        Command::Sign {
            seed,
            index,
            message,
            tree,
        } => sign(&seed, index, &message, tree),
        Command::WordsToSeed { words } => words_to_seed(&words.join(" "))
            .map(|seed| hex::encode(&seed))
            .context("these are not 24 valid backup words"),
        Command::FrameHash(transfer) => Ok(hex::encode(&transfer.tx()?.sig_hash())),
        Command::FrameJson(args) => {
            let tx = args.transfer.tx()?;
            Ok(json::submission(&tx, &signature(&args.signature)?).to_string())
        }
        Command::FrameRaw(args) => Ok(hex::encode(
            &args.transfer.tx()?.encode(&signature(&args.signature)?),
        )),
        Command::FrameRotateHash(rotation) => Ok(hex::encode(&rotation.tx()?.sig_hash())),
        Command::FrameRotateMessage(rotation) => Ok(hex::encode(&rotation.tx()?.signing_message())),
        Command::FrameRotateJson(args) => {
            let tx = args.rotation.tx()?;
            Ok(json::submission(&tx, &signature(&args.signature)?).to_string())
        }
        Command::Ml(ml) => run_ml(ml),
        Command::FrameCreateHash(create) => Ok(hex::encode(&create.tx()?.sig_hash())),
        Command::FrameCreateJson(args) => {
            let tx = args.create.tx()?;
            Ok(json::submission(&tx, &signature(&args.signature)?).to_string())
        }
        Command::DeviceCreate {
            port,
            sender,
            factory,
        } => Ok(device::open(&port)?
            .create(address(&sender)?, address(&factory)?)?
            .to_string()),
        Command::DevicePubkey { port } => Ok(device::open(&port)?.status()?.line()),
        Command::Stealth(stealth) => run_stealth(stealth),
        Command::Lockdown(lockdown) => run_lockdown(lockdown),
        Command::DeviceScheme { port } => {
            Ok(scheme_name(device::open(&port)?.scheme()?).to_string())
        }
        Command::DeviceMlpubkey { port } => Ok(hex::encode(&device::open(&port)?.ml_pubkey()?)),
        Command::DeviceMlSend(args) => {
            let (sender, to) = (address(&args.sender)?, address(&args.to)?);
            Ok(device::open(&args.port)?
                .ml_send(sender, args.nonce, to, args.value_wei)?
                .to_string())
        }
        Command::DeviceLog { port, seconds } => {
            device::open(&port)?.log(seconds)?;
            Ok(String::new())
        }
        Command::DeviceSend(args) => {
            let (sender, to) = (address(&args.sender)?, address(&args.to)?);
            Ok(device::open(&args.port)?
                .send(sender, args.nonce, to, args.value_wei)?
                .to_string())
        }
        Command::DeviceRotate {
            port,
            sender,
            nonce,
        } => Ok(device::open(&port)?
            .rotate(address(&sender)?, nonce)?
            .to_string()),
        Command::DevicePrepare { port } => Ok(device::open(&port)?.prepare()?.line()),
        Command::DeviceActivate { port, tree } => {
            device::open(&port)?.activate(tree)?;
            Ok("activated".to_string())
        }
        Command::DeviceRestore {
            port,
            seed,
            tree,
            chain_next_index,
        } => {
            device::open(&port)?.restore(hash(&seed)?, tree, chain_next_index)?;
            Ok("restored".to_string())
        }
        Command::FrameRotateRaw(args) => Ok(hex::encode(
            &args.rotation.tx()?.encode(&signature(&args.signature)?),
        )),
    }
}

fn main() -> Result<()> {
    println!("{}", run(Cli::parse().command)?);
    Ok(())
}
