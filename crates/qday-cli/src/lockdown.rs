//! Locking the chip: every step, in order, and what guards it. Burning fuses can't be undone, so
//! this only reads the chip unless it's told to burn, and the order is tested. See
//! `docs/lockdown.md`.

use std::fmt::Write as _;
use std::io::{BufRead, Write as _};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::Args;
use qday_wallet::lockdown::{Check, Fuses};

/// Printed ahead of the plan, and so ahead of any real burn.
const WARNING: &str = "PROTOTYPE: this has never run on a chip. Don't run it unless you \
                       explicitly mean to lock this board for good.\n\n";
const CHIP: &str = "esp32s3";
/// Where the partition table starts, so also the most room the bootloader can have.
const TABLE_AT: u64 = 0x8000;
const APP_AT: u64 = 0x1_0000;
/// The size of the `factory` partition.
const APP_BYTES: u64 = 0x30_0000;

const FLASH_KEY: &str = "flash-encryption.key";
const SIGNING_KEY: &str = "secure-boot-signing.pem";
const SEAL_KEY: &str = "seal.key";
const DIGEST: &str = "secure-boot-digest.bin";
const APP_IMAGE: &str = "app.bin";
const SIGNED_BOOTLOADER: &str = "bootloader-signed.bin";
const SIGNED_APP: &str = "app-signed.bin";
const BOOTLOADER_OUT: &str = "bootloader-encrypted.bin";
const TABLE_OUT: &str = "partition-table-encrypted.bin";
const APP_OUT: &str = "app-encrypted.bin";

/// Everything the steps work on.
#[derive(Args, Clone)]
pub struct Inputs {
    /// Where the keys and images go. Keep this out of the repository
    #[arg(long)]
    pub dir: PathBuf,
    /// The firmware ELF
    #[arg(long)]
    pub elf: PathBuf,
    /// The bootloader built from tools/lockdown/bootloader
    #[arg(long)]
    pub bootloader: PathBuf,
    /// The binary partition table
    #[arg(long)]
    pub partition_table: PathBuf,
}

/// The tool a step runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    /// Keys, signing and encryption, on the laptop.
    Espsecure,
    /// The chip's fuses.
    Espefuse,
    /// Images, and the chip's flash.
    Esptool,
}

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Self::Espsecure => "espsecure",
            Self::Espefuse => "espefuse",
            Self::Esptool => "esptool",
        }
    }
}

/// One command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// What runs it.
    pub tool: Tool,
    /// Its arguments, after the tool's own connection options.
    pub args: Vec<String>,
    /// What it's for.
    pub note: &'static str,
    /// A key this makes. The step is skipped if it exists, because a key that's already burned
    /// into a chip must never be replaced.
    pub creates: Option<PathBuf>,
}

impl Step {
    fn new(tool: Tool, note: &'static str, args: &[&str]) -> Self {
        Self {
            tool,
            args: args.iter().map(ToString::to_string).collect(),
            note,
            creates: None,
        }
    }

    fn creating(mut self, path: PathBuf) -> Self {
        self.creates = Some(path);
        self
    }

    /// Whether this step changes the chip in a way nothing can undo.
    #[must_use]
    pub fn is_permanent(&self) -> bool {
        self.tool == Tool::Espefuse
    }
}

fn text(path: &Path) -> String {
    path.display().to_string()
}

/// The steps that make the keys.
fn key_steps(inputs: &Inputs) -> Vec<Step> {
    let file = |name: &str| text(&inputs.dir.join(name));
    let signing_key = file(SIGNING_KEY);
    let key = |note, name: &str| {
        Step::new(
            Tool::Espsecure,
            note,
            &["generate-flash-encryption-key", &file(name)],
        )
        .creating(inputs.dir.join(name))
    };
    let signing_args = [
        "generate-signing-key",
        "--version",
        "2",
        "--scheme",
        "rsa3072",
        &signing_key,
    ];
    let digest = file(DIGEST);
    let digest_args = [
        "digest-sbv2-public-key",
        "--keyfile",
        &signing_key,
        "--output",
        &digest,
    ];
    vec![
        key("Make the flash encryption key", FLASH_KEY),
        key("Make the seal key", SEAL_KEY),
        Step::new(
            Tool::Espsecure,
            "Make the firmware signing key",
            &signing_args,
        )
        .creating(inputs.dir.join(SIGNING_KEY)),
        Step::new(
            Tool::Espsecure,
            "Hash the signing key for the chip",
            &digest_args,
        ),
    ]
}

/// The steps that build, sign and encrypt the images. Signing comes first, because the chip
/// checks the signature on what it decrypts.
fn image_steps(inputs: &Inputs) -> Vec<Step> {
    let file = |name: &str| text(&inputs.dir.join(name));
    let (flash_key, signing_key) = (file(FLASH_KEY), file(SIGNING_KEY));
    let sign = |note, output: &str, input: &str| {
        let output = file(output);
        let args = [
            "sign-data",
            "--version",
            "2",
            "--keyfile",
            &signing_key,
            "--output",
            &output,
            input,
        ];
        Step::new(Tool::Espsecure, note, &args)
    };
    let encrypt = |note, address: u64, output: &str, input: &str| {
        let (address, output) = (format!("{address:#x}"), file(output));
        let args = [
            "encrypt-flash-data",
            "--aes-xts",
            "--keyfile",
            &flash_key,
            "--address",
            &address,
            "--output",
            &output,
            input,
        ];
        Step::new(Tool::Espsecure, note, &args)
    };
    let (image, elf) = (file(APP_IMAGE), text(&inputs.elf));
    let image_args = [
        "--chip",
        CHIP,
        "elf2image",
        "--secure-pad-v2",
        "--flash-mode",
        "dio",
        "--flash-size",
        "16MB",
        "--output",
        &image,
        &elf,
    ];
    let bootloader = text(&inputs.bootloader);
    let table = text(&inputs.partition_table);
    vec![
        Step::new(
            Tool::Esptool,
            "Build the app image, padded for its signature",
            &image_args,
        ),
        sign("Sign the bootloader", SIGNED_BOOTLOADER, &bootloader),
        sign("Sign the app", SIGNED_APP, &image),
        encrypt(
            "Encrypt the bootloader",
            0,
            BOOTLOADER_OUT,
            &file(SIGNED_BOOTLOADER),
        ),
        encrypt("Encrypt the partition table", TABLE_AT, TABLE_OUT, &table),
        encrypt("Encrypt the app", APP_AT, APP_OUT, &file(SIGNED_APP)),
    ]
}

/// The steps that only touch the laptop.
#[must_use]
pub fn prepare_steps(inputs: &Inputs) -> Vec<Step> {
    let mut steps = key_steps(inputs);
    steps.extend(image_steps(inputs));
    steps
}

/// The steps that change the chip, in the only safe order. The keys go in first, because burning
/// the last fuses closes the door on read protection. The firmware goes in before any switch is
/// burned. Secure download goes last, because `espefuse` can't reach the chip after it.
#[must_use]
pub fn burn_steps(inputs: &Inputs) -> Vec<Step> {
    let file = |name: &str| text(&inputs.dir.join(name));
    let burn_key = |note, block, name: &str, purpose| {
        Step::new(
            Tool::Espefuse,
            note,
            &["burn-key", block, &file(name), purpose],
        )
    };
    let burn = |note, fuses: &[&str]| {
        let mut args = vec!["burn-efuse"];
        args.extend_from_slice(fuses);
        Step::new(Tool::Espefuse, note, &args)
    };
    vec![
        Step::new(
            Tool::Esptool,
            "Erase the flash, so no plain seed survives. This erases the wallet",
            &["erase-flash"],
        ),
        burn_key(
            "Burn the flash encryption key",
            "BLOCK_KEY0",
            FLASH_KEY,
            "XTS_AES_128_KEY",
        ),
        burn_key(
            "Burn the signing key's hash",
            "BLOCK_KEY1",
            DIGEST,
            "SECURE_BOOT_DIGEST0",
        ),
        burn_key("Burn the seal key", "BLOCK_KEY2", SEAL_KEY, "HMAC_UP"),
        Step::new(
            Tool::Esptool,
            "Write the signed, encrypted firmware",
            &[
                "write-flash",
                "0x0",
                &file(BOOTLOADER_OUT),
                "0x8000",
                &file(TABLE_OUT),
                "0x10000",
                &file(APP_OUT),
            ],
        ),
        burn(
            "Turn on flash encryption for good",
            &["SPI_BOOT_CRYPT_CNT", "7"],
        ),
        burn("Turn on secure boot", &["SECURE_BOOT_EN", "0x1"]),
        burn(
            "Close the debug and download paths",
            &[
                "DIS_DOWNLOAD_ICACHE",
                "0x1",
                "DIS_DOWNLOAD_DCACHE",
                "0x1",
                "DIS_PAD_JTAG",
                "0x1",
                "DIS_USB_JTAG",
                "0x1",
                "SOFT_DIS_JTAG",
                "7",
                "DIS_DIRECT_BOOT",
                "0x1",
                "DIS_DOWNLOAD_MANUAL_ENCRYPT",
                "0x1",
                "SECURE_BOOT_AGGRESSIVE_REVOKE",
                "0x1",
            ],
        ),
        burn(
            "Revoke the two unused signing key slots",
            &[
                "SECURE_BOOT_KEY_REVOKE1",
                "0x1",
                "SECURE_BOOT_KEY_REVOKE2",
                "0x1",
            ],
        ),
        Step::new(
            Tool::Espefuse,
            "Write-protect the cache and download fuses",
            &["write-protect-efuse", "DIS_ICACHE"],
        ),
        Step::new(
            Tool::Espefuse,
            "Write-protect the read protection fuses",
            &["write-protect-efuse", "RD_DIS"],
        ),
        burn(
            "Limit download mode to writing flash. This must stay last",
            &["ENABLE_SECURITY_DOWNLOAD", "0x1"],
        ),
    ]
}

/// Where the burn steps go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// `espefuse`'s simulated fuses, kept in a file. Nothing reaches a chip.
    Rehearsal(PathBuf),
    /// A real chip on a port.
    Chip(String),
}

/// The full command for a step, or `None` if it has no meaning for this target.
#[must_use]
pub fn command_line(step: &Step, target: &Target) -> Option<Vec<String>> {
    let mut line = vec![step.tool.name().to_string()];
    match (step.tool, target) {
        (Tool::Espsecure, _) => {}
        (Tool::Esptool, _) if is_image_step(step) => {}
        (Tool::Esptool, Target::Rehearsal(_)) => return None,
        (Tool::Espefuse, Target::Rehearsal(file)) => {
            let options = [
                "--chip",
                CHIP,
                "--virt",
                "--do-not-confirm",
                "--path-efuse-file",
            ];
            line.extend(options.map(String::from));
            line.push(text(file));
        }
        (Tool::Esptool | Tool::Espefuse, Target::Chip(port)) => {
            line.extend(["--chip", CHIP, "--port", port].map(String::from));
        }
    }
    line.extend(step.args.iter().cloned());
    Some(line)
}

/// Whether a step builds an image on the laptop, rather than talking to a chip.
fn is_image_step(step: &Step) -> bool {
    step.args.iter().any(|arg| arg == "elf2image")
}

/// Runs one step, stopping everything if it fails.
fn run_step(step: &Step, target: &Target) -> Result<()> {
    if step.creates.as_ref().is_some_and(|path| path.exists()) {
        println!("Kept the existing key: {}", step.note);
        return Ok(());
    }
    let Some(line) = command_line(step, target) else {
        return Ok(());
    };
    println!("{}\n  {}", step.note, line.join(" "));
    let status = Command::new(&line[0])
        .args(&line[1..])
        .status()
        .with_context(|| format!("couldn't start {}", line[0]))?;
    if !status.success() {
        bail!("stopped at \"{}\": nothing after it has run", step.note);
    }
    Ok(())
}

/// Refuses images that can't work, before anything reaches the chip.
fn check_sizes(bootloader: u64, app: u64) -> Result<()> {
    if bootloader > TABLE_AT {
        bail!("the bootloader is {bootloader} bytes and would run into the partition table");
    }
    if app > APP_BYTES {
        bail!("the app is {app} bytes and the partition holds {APP_BYTES}");
    }
    Ok(())
}

fn check_images(inputs: &Inputs) -> Result<()> {
    let size = |name: &str| -> Result<u64> {
        let path = inputs.dir.join(name);
        let metadata = std::fs::metadata(&path)
            .with_context(|| format!("{} is missing: run lockdown-prepare", path.display()))?;
        Ok(metadata.len())
    };
    for name in [FLASH_KEY, SEAL_KEY, DIGEST, TABLE_OUT] {
        size(name)?;
    }
    check_sizes(size(BOOTLOADER_OUT)?, size(APP_OUT)?)
}

fn write_steps(out: &mut String, title: &str, steps: &[Step], target: &Target) {
    let _ = writeln!(out, "{title}:");
    for step in steps {
        let mark = if step.is_permanent() {
            "PERMANENT: "
        } else {
            ""
        };
        let line = command_line(step, target).map_or_else(String::new, |line| line.join(" "));
        let _ = writeln!(out, "  {mark}{}\n    {line}", step.note);
    }
}

/// Every command, without running any.
#[must_use]
pub fn plan(inputs: &Inputs, port: &str) -> String {
    let target = Target::Chip(port.to_string());
    let mut out = String::from(WARNING);
    write_steps(&mut out, "On the laptop", &prepare_steps(inputs), &target);
    write_steps(&mut out, "On the chip", &burn_steps(inputs), &target);
    out
}

/// Makes the keys and the images. Nothing reaches a chip.
///
/// # Errors
///
/// Fails if a step fails.
pub fn prepare(inputs: &Inputs) -> Result<String> {
    std::fs::create_dir_all(&inputs.dir)
        .with_context(|| format!("couldn't make {}", inputs.dir.display()))?;
    let target = Target::Rehearsal(inputs.dir.join("unused"));
    for step in prepare_steps(inputs) {
        run_step(&step, &target)?;
    }
    check_images(inputs)?;
    Ok("Keys and images are ready. Nothing has touched a chip.".to_string())
}

/// Runs the burn steps against simulated fuses in `fuses`.
///
/// # Errors
///
/// Fails if a step fails, which is the point: a bad fuse name shows up here, not on a chip.
pub fn rehearse(inputs: &Inputs, fuses: &Path) -> Result<String> {
    check_images(inputs)?;
    let target = Target::Rehearsal(fuses.to_path_buf());
    for step in burn_steps(inputs) {
        run_step(&step, &target)?;
    }
    Ok(format!(
        "Rehearsal finished. Simulated fuses: {}",
        fuses.display()
    ))
}

/// The words someone must type before a real burn.
fn burn_phrase(mac: &str) -> String {
    format!("LOCK {mac} FOREVER")
}

/// Whether `esptool read-mac`'s output names this chip.
fn names_chip(output: &str, mac: &str) -> bool {
    output.to_lowercase().contains(&mac.to_lowercase())
}

/// Locks a real chip. There's no way back from this.
///
/// # Errors
///
/// Fails, having changed nothing, if the images aren't ready, the chip isn't the one named, or the
/// confirmation doesn't match. Fails part way if a step fails.
pub fn burn(inputs: &Inputs, port: &str, mac: &str) -> Result<String> {
    check_images(inputs)?;
    let output = Command::new(Tool::Esptool.name())
        .args(["--chip", CHIP, "--port", port, "read-mac"])
        .output()
        .context("couldn't start esptool")?;
    if !output.status.success() || !names_chip(&String::from_utf8_lossy(&output.stdout), mac) {
        bail!("the chip on {port} isn't {mac}: nothing was changed");
    }
    println!("{}", plan(inputs, port));
    let phrase = burn_phrase(mac);
    print!("This can't be undone. To go on, type \"{phrase}\": ");
    std::io::stdout().flush()?;
    let mut typed = String::new();
    std::io::stdin().lock().read_line(&mut typed)?;
    if typed.trim_end() != phrase {
        bail!("that didn't match: nothing was changed");
    }
    let target = Target::Chip(port.to_string());
    for step in burn_steps(inputs) {
        run_step(&step, &target)?;
    }
    std::fs::remove_file(inputs.dir.join(SEAL_KEY)).context("couldn't delete the seal key")?;
    Ok(
        "Locked. The seal key is deleted. Restart the board and restore from your words."
            .to_string(),
    )
}

/// What the device says about itself, in words.
#[must_use]
pub fn report(fuses: &Fuses, sealed: bool) -> String {
    let yes_no = |value| if value { "yes" } else { "no" };
    let missing: Vec<&str> = fuses.missing().map(Check::name).collect();
    let mut out = format!(
        "chip: {}\nflash encrypted: {}\nsecure boot: {}\nseed sealed: {}",
        fuses.level().name(),
        yes_no(fuses.flash_encrypted()),
        yes_no(fuses.secure_boot),
        yes_no(sealed),
    );
    if !missing.is_empty() {
        let _ = write!(out, "\nmissing: {}", missing.join(", "));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> Inputs {
        Inputs {
            dir: PathBuf::from("/keys"),
            elf: PathBuf::from("/fw.elf"),
            bootloader: PathBuf::from("/bootloader.bin"),
            partition_table: PathBuf::from("/table.bin"),
        }
    }

    /// Where a step mentioning `word` comes in the burn.
    fn at(word: &str) -> usize {
        burn_steps(&inputs())
            .iter()
            .position(|step| step.args.iter().any(|arg| arg == word))
            .unwrap_or_else(|| panic!("no step names {word}"))
    }

    fn prepared_at(word: &str) -> usize {
        prepare_steps(&inputs())
            .iter()
            .position(|step| step.note == word)
            .unwrap_or_else(|| panic!("no step called {word}"))
    }

    fn all_words() -> Vec<String> {
        let inputs = inputs();
        let mut steps = prepare_steps(&inputs);
        steps.extend(burn_steps(&inputs));
        steps.into_iter().flat_map(|step| step.args).collect()
    }

    #[test]
    fn every_key_is_in_before_any_switch_is_burned() {
        for key in ["XTS_AES_128_KEY", "SECURE_BOOT_DIGEST0", "HMAC_UP"] {
            assert!(at(key) < at("SPI_BOOT_CRYPT_CNT"), "{key}");
            assert!(at(key) < at("SECURE_BOOT_EN"), "{key}");
        }
    }

    #[test]
    fn the_flash_is_erased_before_anything_else() {
        assert_eq!(at("erase-flash"), 0);
    }

    #[test]
    fn the_firmware_is_written_before_the_chip_starts_enforcing() {
        assert!(at("write-flash") < at("SPI_BOOT_CRYPT_CNT"));
        assert!(at("write-flash") < at("SECURE_BOOT_EN"));
    }

    #[test]
    fn flash_encryption_comes_before_secure_boot() {
        assert!(at("SPI_BOOT_CRYPT_CNT") < at("SECURE_BOOT_EN"));
    }

    #[test]
    fn read_protection_is_closed_only_after_every_secret_key() {
        assert!(at("HMAC_UP") < at("RD_DIS"));
        assert!(at("XTS_AES_128_KEY") < at("RD_DIS"));
    }

    #[test]
    fn secure_download_is_the_very_last_step() {
        assert_eq!(
            at("ENABLE_SECURITY_DOWNLOAD"),
            burn_steps(&inputs()).len() - 1
        );
    }

    #[test]
    fn the_wallets_usb_port_is_never_turned_off() {
        let words = all_words();
        assert!(!words.iter().any(|word| word == "DIS_USB_SERIAL_JTAG"));
        assert!(!words.iter().any(|word| word == "USB_PHY_SEL"));
    }

    #[test]
    fn download_mode_stays_available_for_signed_updates() {
        assert!(!all_words().iter().any(|word| word == "DIS_DOWNLOAD_MODE"));
    }

    #[test]
    fn flash_encryption_keys_avoid_the_block_that_cant_hold_them() {
        assert!(!all_words().iter().any(|word| word == "BLOCK_KEY5"));
    }

    #[test]
    fn no_two_keys_share_a_block() {
        let mut blocks: Vec<String> = burn_steps(&inputs())
            .into_iter()
            .filter(|step| step.args[0] == "burn-key")
            .map(|step| step.args[1].clone())
            .collect();
        let count = blocks.len();
        blocks.sort();
        blocks.dedup();
        assert_eq!((count, blocks.len()), (3, 3));
    }

    #[test]
    fn the_plan_burns_a_fuse_for_every_check() {
        let words = all_words();
        for fuse in [
            "SPI_BOOT_CRYPT_CNT",
            "SECURE_BOOT_EN",
            "HMAC_UP",
            "DIS_PAD_JTAG",
            "DIS_USB_JTAG",
            "SOFT_DIS_JTAG",
            "DIS_DIRECT_BOOT",
            "DIS_DOWNLOAD_MANUAL_ENCRYPT",
            "DIS_DOWNLOAD_ICACHE",
            "DIS_DOWNLOAD_DCACHE",
            "ENABLE_SECURITY_DOWNLOAD",
        ] {
            assert!(words.iter().any(|word| word == fuse), "{fuse}");
        }
    }

    #[test]
    fn images_are_signed_before_they_are_encrypted() {
        assert!(prepared_at("Sign the app") < prepared_at("Encrypt the app"));
        assert!(prepared_at("Sign the bootloader") < prepared_at("Encrypt the bootloader"));
    }

    #[test]
    fn keys_are_never_made_twice() {
        let making = prepare_steps(&inputs())
            .into_iter()
            .filter(|step| step.args[0].starts_with("generate"));
        for step in making {
            assert!(step.creates.is_some(), "{}", step.note);
        }
    }

    #[test]
    fn a_rehearsal_cant_reach_a_chip() {
        let target = Target::Rehearsal(PathBuf::from("/fuses.bin"));
        for step in burn_steps(&inputs()) {
            let Some(line) = command_line(&step, &target) else {
                assert_eq!(step.tool, Tool::Esptool);
                continue;
            };
            assert!(line.iter().any(|word| word == "--virt"), "{}", step.note);
            assert!(!line.iter().any(|word| word == "--port"), "{}", step.note);
        }
    }

    #[test]
    fn a_real_burn_keeps_the_tools_own_confirmation() {
        let target = Target::Chip("/dev/port".to_string());
        for step in burn_steps(&inputs()) {
            let line = command_line(&step, &target).expect("every step runs on a chip");
            assert!(!line.iter().any(|word| word == "--do-not-confirm"));
            assert!(!line.iter().any(|word| word == "--virt"));
        }
    }

    #[test]
    fn every_fuse_step_counts_as_permanent() {
        for step in burn_steps(&inputs()) {
            assert_eq!(step.is_permanent(), step.tool == Tool::Espefuse);
        }
    }

    #[test]
    fn a_bootloader_that_reaches_the_table_is_refused() {
        assert!(check_sizes(TABLE_AT, APP_BYTES).is_ok());
        assert!(check_sizes(TABLE_AT + 1, 0).is_err());
        assert!(check_sizes(0, APP_BYTES + 1).is_err());
    }

    #[test]
    fn the_chip_must_match_whatever_the_case() {
        assert!(names_chip("MAC: AA:BB:CC:DD:EE:FF\n", "aa:bb:cc:dd:ee:ff"));
        assert!(!names_chip("MAC: AA:BB:CC:DD:EE:00\n", "aa:bb:cc:dd:ee:ff"));
    }

    #[test]
    fn the_plan_opens_with_the_warning() {
        assert!(plan(&inputs(), "/dev/port").starts_with(WARNING));
    }

    #[test]
    fn the_plan_marks_what_cant_be_undone() {
        let plan = plan(&inputs(), "/dev/port");
        assert!(plan.contains("PERMANENT: Turn on secure boot"));
        assert!(!plan.contains("PERMANENT: Sign the app"));
    }

    #[test]
    fn a_new_chip_is_reported_plainly() {
        let report = report(&Fuses::default(), false);
        assert!(report.starts_with("chip: open\n"));
        assert!(report.contains("seed sealed: no"));
        assert!(report.contains("missing: flash-encryption, secure-boot"));
    }
}
