//! Numbers and addresses as the screens show them. Amounts are exact, never rounded.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use qday_wallet::frame::Address;
use qday_wallet::hex;
use qday_xmss::Hash;

const WEI_PER_ETH: u128 = 1_000_000_000_000_000_000;
const WEI_DIGITS: usize = 18;

/// Puts separators into a string of digits.
fn with_separators(digits: &str) -> String {
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (position, digit) in digits.chars().enumerate() {
        if position > 0 && (digits.len() - position) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// An amount of wei in ETH, with every significant digit.
#[must_use]
pub fn eth(wei: u128) -> String {
    let whole = with_separators(&(wei / WEI_PER_ETH).to_string());
    let fraction = wei % WEI_PER_ETH;
    if fraction == 0 {
        return format!("{whole} ETH");
    }
    let digits = format!("{fraction:0WEI_DIGITS$}");
    format!("{whole}.{} ETH", digits.trim_end_matches('0'))
}

/// An amount of a token with `decimals` places, with every significant digit: "125.5 USDC".
#[must_use]
pub fn token(amount: u128, decimals: u8, symbol: &str) -> String {
    let places = usize::from(decimals);
    let unit = 10_u128.checked_pow(u32::from(decimals));
    let (whole, fraction) = unit.map_or((0, amount), |unit| (amount / unit, amount % unit));
    let whole = with_separators(&whole.to_string());
    if fraction == 0 {
        return format!("{whole} {symbol}");
    }
    let digits = format!("{fraction:0places$}");
    format!("{whole}.{} {symbol}", digits.trim_end_matches('0'))
}

/// An amount in a token's smallest unit, for tokens the device doesn't know: "125,500,000 units".
#[must_use]
pub fn base_units(amount: u128) -> String {
    format!("{} units", with_separators(&amount.to_string()))
}

/// A count with thousands separators.
#[must_use]
pub fn grouped(value: u32) -> String {
    with_separators(&value.to_string())
}

/// A key number with its leading zeros split off, so they can be greyed: ("#00", "67").
#[must_use]
pub fn key_number(index: u32) -> (String, String) {
    let digits = format!("{index:04}");
    let zeros = digits.len() - index.to_string().len();
    let (grey, plain) = digits.split_at(zeros);
    (format!("#{grey}"), plain.to_string())
}

/// An address in groups of four, split into two lines of five groups.
#[must_use]
pub fn address_lines(address: &Address) -> (String, String) {
    let text = hex::encode(address);
    let digits = &text[2..];
    let groups = |part: &str| {
        let chunks: Vec<&str> = part
            .as_bytes()
            .chunks(4)
            .filter_map(|chunk| core::str::from_utf8(chunk).ok())
            .collect();
        chunks.join(" ")
    };
    (
        format!("0x{}", groups(&digits[..20])),
        groups(&digits[20..]),
    )
}

/// An account key as (start, middle, end), so the middle can be greyed.
#[must_use]
pub fn key_parts(key: &Hash) -> (String, String, String) {
    let text = hex::encode(key);
    let end = text.len() - 4;
    (
        text[..6].to_string(),
        text[6..14].to_string(),
        text[end..].to_string(),
    )
}

/// A wait in minutes and seconds.
#[must_use]
pub fn duration(ms: u64) -> String {
    let seconds = ms.div_ceil(1000);
    match (seconds / 60, seconds % 60) {
        (0, left) => format!("{left} s"),
        (minutes, 0) => format!("{minutes} min"),
        (minutes, left) => format!("{minutes} min {left} s"),
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::{address_lines, base_units, duration, eth, grouped, key_number, key_parts, token};

    #[test]
    fn whole_and_fractional_ether_are_exact() {
        assert_eq!(eth(0), "0 ETH");
        assert_eq!(eth(1), "0.000000000000000001 ETH");
        assert_eq!(eth(1_000_000_000_000_000), "0.001 ETH");
        assert_eq!(eth(1_000_000_000_000_000_000), "1 ETH");
        assert_eq!(eth(1_500_000_000_000_000_000), "1.5 ETH");
        assert_eq!(eth(1_234_000_000_000_000_000_000), "1,234 ETH");
        assert_eq!(eth(1_000_000_000_000_000_001), "1.000000000000000001 ETH");
    }

    #[test]
    fn the_largest_value_is_not_truncated() {
        assert_eq!(
            eth(u128::MAX),
            "340,282,366,920,938,463,463.374607431768211455 ETH"
        );
    }

    #[test]
    fn token_amounts_use_the_tokens_decimals() {
        assert_eq!(token(125_500_000, 6, "USDC"), "125.5 USDC");
        assert_eq!(token(2_500_000_000, 6, "USDC"), "2,500 USDC");
        assert_eq!(token(1, 8, "WBTC"), "0.00000001 WBTC");
        assert_eq!(base_units(125_500_000), "125,500,000 units");
    }

    #[test]
    fn counts_get_separators() {
        assert_eq!(grouped(67), "67");
        assert_eq!(grouped(1024), "1,024");
        assert_eq!(grouped(1_000_000), "1,000,000");
    }

    #[test]
    fn key_numbers_split_off_leading_zeros() {
        assert_eq!(key_number(67), ("#00".to_string(), "67".to_string()));
        assert_eq!(key_number(0), ("#000".to_string(), "0".to_string()));
        assert_eq!(key_number(1023), ("#".to_string(), "1023".to_string()));
    }

    #[test]
    fn addresses_show_every_digit_in_groups_of_four() {
        let mut address = [0; 20];
        for (byte, value) in address.iter_mut().zip(0_u8..) {
            *byte = value * 11;
        }
        let (first, second) = address_lines(&address);
        assert_eq!(first, "0x000b 1621 2c37 424d 5863");
        assert_eq!(second, "6e79 848f 9aa5 b0bb c6d1");
    }

    #[test]
    fn account_keys_grey_the_middle() {
        let (start, middle, end) = key_parts(&[0xAB; 32]);
        assert_eq!(start, "0xabab");
        assert_eq!(end, "abab");
        assert!(middle.starts_with("abab"));
        assert_eq!(start.len() + middle.len() + end.len(), 18);
    }

    #[test]
    fn waits_read_naturally() {
        assert_eq!(duration(30_000), "30 s");
        assert_eq!(duration(60_000), "1 min");
        assert_eq!(duration(61_000), "1 min 1 s");
        assert_eq!(duration(3_600_000), "60 min");
        assert_eq!(duration(1), "1 s");
    }
}
