//! The `qday` laptop tool.

mod device;
mod json;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};
use qday_mldsa::blob::{BlobError, prepare_checked};
use qday_mldsa::{KeySeed, Signer, StandardSigner};
use qday_wallet::backup::words_to_seed;
use qday_wallet::frame::{Address, FrameTx, MlFrameTx};
use qday_wallet::hex;
use qday_wallet::invoice::{Spend, keys_for, spend_hash};
use qday_wallet::protocol::scheme_name;
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
    /// Print the device's keys, then any prepared next key
    DevicePubkey { port: String },
    #[command(flatten)]
    Invoice(InvoiceCommand),
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

/// Fresh one-time keys, paid through the hash pool.
#[derive(Subcommand)]
enum InvoiceCommand {
    /// Print an invoice's leaf and public seed, from a laptop key
    InvoiceKey {
        seed: String,
        number: u32,
        #[arg(long, default_value_t = 0)]
        account: u32,
    },
    /// Sign a spend of an invoice's whole note with a laptop key
    InvoiceSign {
        seed: String,
        #[command(flatten)]
        spend: InvoiceSpend,
        #[arg(long, default_value_t = 0)]
        account: u32,
    },
    /// Ask the device for an invoice's leaf and public seed
    DeviceInvoice { port: String, number: u32 },
    /// Sign a spend of an invoice's whole note on the device
    DeviceInvoiceSpend {
        port: String,
        #[command(flatten)]
        spend: InvoiceSpend,
    },
}

#[derive(Args)]
struct InvoiceSpend {
    number: u32,
    pool: String,
    to: String,
    fee_wei: u128,
}

impl InvoiceSpend {
    fn parsed(&self) -> Result<Spend> {
        Ok(Spend {
            number: self.number,
            pool: address(&self.pool)?,
            to: address(&self.to)?,
            fee: self.fee_wei,
        })
    }
}

fn run_invoice(command: InvoiceCommand) -> Result<String> {
    match command {
        InvoiceCommand::InvoiceKey {
            seed,
            number,
            account,
        } => {
            let keys = keys_for(&hash(&seed)?, account, number);
            Ok(format!(
                "{} {}",
                hex::encode(&keys.leaf(0)),
                hex::encode(keys.pub_seed())
            ))
        }
        InvoiceCommand::InvoiceSign {
            seed,
            spend,
            account,
        } => {
            let spend = spend.parsed()?;
            let keys = keys_for(&hash(&seed)?, account, spend.number);
            let message = spend_hash(&spend.pool, &keys.leaf(0), &spend.to, spend.fee);
            Ok(hex::encode(&keys.sign_one_time(&message)))
        }
        InvoiceCommand::DeviceInvoice { port, number } => {
            let invoice = device::open(&port)?.invoice(number)?;
            Ok(format!(
                "{} {}",
                hex::encode(&invoice.leaf),
                hex::encode(&invoice.pub_seed)
            ))
        }
        InvoiceCommand::DeviceInvoiceSpend { port, spend } => {
            let signature = device::open(&port)?.spend_invoice(spend.parsed()?)?;
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
        Command::MldsaPubkey { seed, account } => {
            let key_seed = KeySeed::derive(&hash(&seed)?, account);
            Ok(hex::encode(
                &StandardSigner::from_key_seed(&key_seed).public_key(),
            ))
        }
        Command::MldsaSign {
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
        Command::MldsaBlob { pk } => mldsa_blob(&pk),
        Command::MlFrameHash(transfer) => Ok(hex::encode(&transfer.tx()?.sig_hash())),
        Command::MlFrameJson(args) => {
            let tx = args.transfer.tx()?;
            Ok(json::ml_submission(&tx, &signature(&args.signature)?).to_string())
        }
        Command::MlFrameRaw(args) => Ok(hex::encode(
            &args.transfer.tx()?.encode(&signature(&args.signature)?),
        )),
        Command::DevicePubkey { port } => Ok(device::open(&port)?.status()?.line()),
        Command::Invoice(invoice) => run_invoice(invoice),
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
