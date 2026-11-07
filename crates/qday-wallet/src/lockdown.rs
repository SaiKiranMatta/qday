//! The chip's security fuses, as the device reports them. Reading these changes nothing.

/// The key purpose of a seal key: an HMAC whose result software can read.
pub const HMAC_UP: u8 = 8;
/// Key blocks on the chip.
pub const KEY_BLOCKS: usize = 6;
/// Bytes of [`Fuses`] on the wire.
pub const FUSE_BYTES: usize = 3 + KEY_BLOCKS;
/// Every bit of the three-bit counters, which can then never change again.
const ALL_COUNT_BITS: u8 = 0b111;

const SECURE_BOOT: u16 = 1 << 0;
const PAD_JTAG_OFF: u16 = 1 << 1;
const USB_JTAG_OFF: u16 = 1 << 2;
const DIRECT_BOOT_OFF: u16 = 1 << 3;
const MANUAL_ENCRYPT_OFF: u16 = 1 << 4;
const DOWNLOAD_ICACHE_OFF: u16 = 1 << 5;
const DOWNLOAD_DCACHE_OFF: u16 = 1 << 6;
const SECURE_DOWNLOAD: u16 = 1 << 7;
const COUNT_MASK: u8 = 0b111;
const SOFT_JTAG_SHIFT: u8 = 3;

/// How much of the lockdown is in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// Nothing is burned.
    Open,
    /// Some fuses are burned and some aren't.
    Partial,
    /// Every check passes.
    Locked,
}

impl Level {
    /// The word for this level.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Partial => "partial",
            Self::Locked => "locked",
        }
    }
}

/// One thing a locked chip must have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    /// Firmware in flash is encrypted, for good.
    FlashEncryption,
    /// Only signed firmware runs.
    SecureBoot,
    /// A key for sealing the seed and the passcode is in the chip.
    SealKey,
    /// JTAG on the pins is off.
    PadJtag,
    /// JTAG over USB is off. The USB serial port the wallet talks through stays on.
    UsbJtag,
    /// JTAG can't be turned back on by software.
    SoftJtag,
    /// The chip can't boot code straight from flash, past the bootloader.
    DirectBoot,
    /// Download mode can't encrypt data with the chip's key.
    ManualEncrypt,
    /// Download mode can't read flash through the cache.
    DownloadCache,
    /// Download mode can only write flash, never read it.
    SecureDownload,
}

impl Check {
    /// Every check.
    pub const ALL: [Self; 10] = [
        Self::FlashEncryption,
        Self::SecureBoot,
        Self::SealKey,
        Self::PadJtag,
        Self::UsbJtag,
        Self::SoftJtag,
        Self::DirectBoot,
        Self::ManualEncrypt,
        Self::DownloadCache,
        Self::SecureDownload,
    ];

    /// The word for this check.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::FlashEncryption => "flash-encryption",
            Self::SecureBoot => "secure-boot",
            Self::SealKey => "seal-key",
            Self::PadJtag => "pad-jtag",
            Self::UsbJtag => "usb-jtag",
            Self::SoftJtag => "soft-jtag",
            Self::DirectBoot => "direct-boot",
            Self::ManualEncrypt => "manual-encrypt",
            Self::DownloadCache => "download-cache",
            Self::SecureDownload => "secure-download",
        }
    }
}

/// The fuses that matter, as read from the chip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each one is a fuse on the chip"
)]
pub struct Fuses {
    /// `SPI_BOOT_CRYPT_CNT`.
    pub crypt_count: u8,
    /// `SECURE_BOOT_EN`.
    pub secure_boot: bool,
    /// `DIS_PAD_JTAG`.
    pub pad_jtag_off: bool,
    /// `DIS_USB_JTAG`.
    pub usb_jtag_off: bool,
    /// `SOFT_DIS_JTAG`.
    pub soft_jtag: u8,
    /// `DIS_DIRECT_BOOT`.
    pub direct_boot_off: bool,
    /// `DIS_DOWNLOAD_MANUAL_ENCRYPT`.
    pub manual_encrypt_off: bool,
    /// `DIS_DOWNLOAD_ICACHE`.
    pub download_icache_off: bool,
    /// `DIS_DOWNLOAD_DCACHE`.
    pub download_dcache_off: bool,
    /// `ENABLE_SECURITY_DOWNLOAD`.
    pub secure_download: bool,
    /// `KEY_PURPOSE_0` to `KEY_PURPOSE_5`.
    pub key_purposes: [u8; KEY_BLOCKS],
}

impl Fuses {
    /// Whether the chip decrypts flash now. An odd number of bits means on.
    #[must_use]
    pub fn flash_encrypted(&self) -> bool {
        self.crypt_count.count_ones() % 2 == 1
    }

    /// The key block that holds the seal key.
    #[must_use]
    pub fn seal_key(&self) -> Option<u8> {
        let block = self
            .key_purposes
            .iter()
            .position(|&purpose| purpose == HMAC_UP)?;
        u8::try_from(block).ok()
    }

    /// Whether one check passes.
    #[must_use]
    pub fn passes(&self, check: Check) -> bool {
        match check {
            // With every bit burned it can't be flipped back off, unlike a single bit.
            Check::FlashEncryption => self.crypt_count == ALL_COUNT_BITS,
            Check::SecureBoot => self.secure_boot,
            Check::SealKey => self.seal_key().is_some(),
            Check::PadJtag => self.pad_jtag_off,
            Check::UsbJtag => self.usb_jtag_off,
            Check::SoftJtag => self.soft_jtag.count_ones() % 2 == 1,
            Check::DirectBoot => self.direct_boot_off,
            Check::ManualEncrypt => self.manual_encrypt_off,
            Check::DownloadCache => self.download_icache_off && self.download_dcache_off,
            Check::SecureDownload => self.secure_download,
        }
    }

    /// The checks that fail.
    pub fn missing(&self) -> impl Iterator<Item = Check> + '_ {
        Check::ALL.into_iter().filter(|&check| !self.passes(check))
    }

    /// How much of the lockdown is in place.
    #[must_use]
    pub fn level(&self) -> Level {
        match self.missing().count() {
            0 => Level::Locked,
            count if count == Check::ALL.len() => Level::Open,
            _ => Level::Partial,
        }
    }

    /// The fuses as bytes, for the USB line.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; FUSE_BYTES] {
        let flags = [
            (self.secure_boot, SECURE_BOOT),
            (self.pad_jtag_off, PAD_JTAG_OFF),
            (self.usb_jtag_off, USB_JTAG_OFF),
            (self.direct_boot_off, DIRECT_BOOT_OFF),
            (self.manual_encrypt_off, MANUAL_ENCRYPT_OFF),
            (self.download_icache_off, DOWNLOAD_ICACHE_OFF),
            (self.download_dcache_off, DOWNLOAD_DCACHE_OFF),
            (self.secure_download, SECURE_DOWNLOAD),
        ]
        .into_iter()
        .filter(|(on, _)| *on)
        .fold(0, |all, (_, flag)| all | flag);
        let mut bytes = [0; FUSE_BYTES];
        bytes[..2].copy_from_slice(&flags.to_be_bytes());
        bytes[2] =
            (self.crypt_count & COUNT_MASK) | ((self.soft_jtag & COUNT_MASK) << SOFT_JTAG_SHIFT);
        bytes[3..].copy_from_slice(&self.key_purposes);
        bytes
    }

    /// Reads fuses back from [`Self::to_bytes`].
    #[must_use]
    pub fn from_bytes(bytes: &[u8; FUSE_BYTES]) -> Self {
        let flags = u16::from_be_bytes([bytes[0], bytes[1]]);
        let on = |flag| flags & flag != 0;
        let mut key_purposes = [0; KEY_BLOCKS];
        key_purposes.copy_from_slice(&bytes[3..]);
        Self {
            crypt_count: bytes[2] & COUNT_MASK,
            secure_boot: on(SECURE_BOOT),
            pad_jtag_off: on(PAD_JTAG_OFF),
            usb_jtag_off: on(USB_JTAG_OFF),
            soft_jtag: (bytes[2] >> SOFT_JTAG_SHIFT) & COUNT_MASK,
            direct_boot_off: on(DIRECT_BOOT_OFF),
            manual_encrypt_off: on(MANUAL_ENCRYPT_OFF),
            download_icache_off: on(DOWNLOAD_ICACHE_OFF),
            download_dcache_off: on(DOWNLOAD_DCACHE_OFF),
            secure_download: on(SECURE_DOWNLOAD),
            key_purposes,
        }
    }
}
