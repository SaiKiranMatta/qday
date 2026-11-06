//! Amounts and addresses as the screens show them. Amounts are exact, never rounded.

use alloc::format;
use alloc::string::String;

use qday_wallet::frame::Address;
use qday_wallet::hex;
use qday_xmss::keccak256;

const WEI_PER_ETH: u128 = 1_000_000_000_000_000_000;

/// An amount in ETH with every significant digit: `0.5 ETH`, `1234.5678 ETH`.
#[must_use]
pub fn eth(wei: u128) -> String {
    let whole = wei / WEI_PER_ETH;
    let fraction = wei % WEI_PER_ETH;
    if fraction == 0 {
        return format!("{whole} ETH");
    }
    let digits = format!("{fraction:018}");
    format!("{whole}.{} ETH", digits.trim_end_matches('0'))
}

/// An amount of a token with `decimals` places, every significant digit: `125.5 USDC`.
#[must_use]
pub fn token_amount(amount: u128, decimals: u8, symbol: &str) -> String {
    let places = usize::from(decimals);
    let (whole, fraction) = 10_u128
        .checked_pow(u32::from(decimals))
        .map_or((0, amount), |unit| (amount / unit, amount % unit));
    if fraction == 0 {
        return format!("{whole} {symbol}");
    }
    let digits = format!("{fraction:0places$}");
    format!("{whole}.{} {symbol}", digits.trim_end_matches('0'))
}

/// An address with its EIP-55 mixed-case checksum.
#[must_use]
pub fn checksum_address(address: &Address) -> String {
    let lower = hex::encode(address);
    let digits = &lower[2..];
    let hash = keccak256(&[digits.as_bytes()]);
    let mut out = String::with_capacity(lower.len());
    out.push_str("0x");
    for (position, digit) in digits.chars().enumerate() {
        let byte = hash[position / 2];
        let nibble = if position % 2 == 0 {
            byte >> 4
        } else {
            byte & 15
        };
        out.push(if nibble >= 8 {
            digit.to_ascii_uppercase()
        } else {
            digit
        });
    }
    out
}

/// An address shortened to the parts people compare: `0x7099...79C8`.
#[must_use]
pub fn short_address(address: &Address) -> String {
    let full = checksum_address(address);
    format!("{}...{}", &full[..6], &full[38..])
}

/// A wait as minutes and seconds: `5:00`.
#[must_use]
pub fn wait_text(ms: u64) -> String {
    let seconds = ms.div_ceil(1000);
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::{checksum_address, eth, short_address, token_amount, wait_text};

    #[test]
    fn token_amounts_use_the_tokens_decimals() {
        assert_eq!(token_amount(125_500_000, 6, "USDC"), "125.5 USDC");
        assert_eq!(token_amount(2_500_000_000, 6, "USDC"), "2500 USDC");
        assert_eq!(token_amount(1, 8, "WBTC"), "0.00000001 WBTC");
    }

    #[test]
    fn amounts_are_exact() {
        assert_eq!(eth(0), "0 ETH");
        assert_eq!(eth(1), "0.000000000000000001 ETH");
        assert_eq!(eth(1_000_000_000_000_000), "0.001 ETH");
        assert_eq!(eth(1_500_000_000_000_000_000), "1.5 ETH");
        assert_eq!(eth(1_234_000_000_000_000_000_000), "1234 ETH");
        assert_eq!(eth(1_000_000_000_000_000_001), "1.000000000000000001 ETH");
    }

    #[test]
    fn checksums_match_the_eip_55_examples() {
        let address = [
            0x5a, 0xae, 0xb6, 0x05, 0x3f, 0x3e, 0x94, 0xc9, 0xb9, 0xa0, 0x9f, 0x33, 0x66, 0x94,
            0x35, 0xe7, 0xef, 0x1b, 0xea, 0xed,
        ];
        assert_eq!(
            checksum_address(&address),
            "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed"
        );
        assert_eq!(short_address(&address), "0x5aAe...eAed");
    }

    #[test]
    fn waits_read_as_minutes_and_seconds() {
        assert_eq!(wait_text(30_000), "0:30");
        assert_eq!(wait_text(300_000), "5:00");
        assert_eq!(wait_text(3_600_000), "60:00");
        assert_eq!(wait_text(1), "0:01");
    }
}
