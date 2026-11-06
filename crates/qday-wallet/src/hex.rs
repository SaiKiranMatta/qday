//! 0x-prefixed hex, as used on the USB line and by the laptop tool.

use alloc::string::String;
use alloc::vec::Vec;

/// Writes bytes as 0x-prefixed lowercase hex.
#[must_use]
pub fn encode(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(2 + bytes.len() * 2);
    out.push_str("0x");
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    out
}

/// Reads 0x-prefixed hex. Returns `None` if it is anything else.
#[must_use]
pub fn decode(text: &str) -> Option<Vec<u8>> {
    let digits = text.strip_prefix("0x")?.as_bytes();
    if digits.len() % 2 != 0 {
        return None;
    }
    digits
        .chunks_exact(2)
        .map(|pair| Some(digit(pair[0])? << 4 | digit(pair[1])?))
        .collect()
}

fn digit(byte: u8) -> Option<u8> {
    char::from(byte)
        .to_digit(16)
        .and_then(|value| u8::try_from(value).ok())
}

/// Reads 0x-prefixed hex of an exact length.
#[must_use]
pub fn decode_array<const N: usize>(text: &str) -> Option<[u8; N]> {
    decode(text)?.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::{decode, decode_array, encode};

    #[test]
    fn bytes_are_written_with_a_prefix() {
        assert_eq!(encode(&[]), "0x");
        assert_eq!(encode(&[0x00, 0xab, 0xff]), "0x00abff");
    }

    #[test]
    fn hex_reads_back_in_either_case() {
        assert_eq!(decode("0x00abFF"), Some(alloc::vec![0x00, 0xab, 0xff]));
        assert_eq!(decode("0x"), Some(alloc::vec![]));
    }

    #[test]
    fn anything_else_is_refused() {
        assert_eq!(decode("00ab"), None);
        assert_eq!(decode("0xabc"), None);
        assert_eq!(decode("0xzz"), None);
        assert_eq!(decode(""), None);
        assert_eq!(decode("0x+1"), None);
    }

    #[test]
    fn arrays_need_the_exact_length() {
        assert_eq!(decode_array::<2>("0x0102"), Some([1, 2]));
        assert_eq!(decode_array::<2>("0x01"), None);
        assert_eq!(decode_array::<2>("0x010203"), None);
    }
}
