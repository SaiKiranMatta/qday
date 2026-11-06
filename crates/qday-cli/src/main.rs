//! The `qday` laptop tool.

mod device;
mod json;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};
use qday_wallet::backup::words_to_seed;
use qday_wallet::frame::{Address, FrameTx};
use qday_wallet::hex;
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
    /// Print what the device logs
    DeviceLog { port: String, seconds: u64 },
    /// Sign a transfer on the device and print submission JSON
    DeviceSend {
        port: String,
        sender: String,
        nonce: u64,
        to: String,
        value_wei: u128,
    },
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
        Command::DeviceLog { port, seconds } => {
            device::open(&port)?.log(seconds)?;
            Ok(String::new())
        }
        Command::DeviceSend {
            port,
            sender,
            nonce,
            to,
            value_wei,
        } => {
            let (sender, to) = (address(&sender)?, address(&to)?);
            Ok(device::open(&port)?
                .send(sender, nonce, to, value_wei)?
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
