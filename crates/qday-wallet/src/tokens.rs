//! Tokens the device recognises, so a token transfer reads "125.5 USDC" rather than a raw number.
//!
//! The list is built in, like a hardware wallet's token list: the laptop can't add to it, so the
//! symbol on screen always belongs to the contract being called. Transfers of any other token still
//! sign, after a warning that shows the contract and the amount in the token's smallest unit.

use crate::frame::Address;

/// A token the device knows by contract address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    /// The ERC-20 contract.
    pub address: Address,
    /// What the screen calls it.
    pub symbol: &'static str,
    /// Decimal places in one whole token.
    pub decimals: u8,
}

/// The devnet's test tokens. `tools/devnet/deploy-tokens.sh` deploys them through the CREATE2
/// factory, so they have these addresses on every devnet.
pub const KNOWN: [Token; 2] = [
    Token {
        address: address("0xd1108cBf3C5E7b44D7A58d4b2cdA492a0DDF6a72"),
        symbol: "USDC",
        decimals: 6,
    },
    Token {
        address: address("0xb212346871a9a08494e165F5AeFe655f06A5B75F"),
        symbol: "WBTC",
        decimals: 8,
    },
];

/// The known token at `address`, if there is one.
#[must_use]
pub fn known(address: &Address) -> Option<&'static Token> {
    KNOWN.iter().find(|token| token.address == *address)
}

const fn address(text: &str) -> Address {
    let digits = text.as_bytes();
    assert!(digits.len() == 42, "an address is 0x and 40 hex digits");
    let mut out = [0; 20];
    let mut i = 0;
    while i < 20 {
        out[i] = (nibble(digits[2 + 2 * i]) << 4) | nibble(digits[3 + 2 * i]);
        i += 1;
    }
    out
}

const fn nibble(digit: u8) -> u8 {
    match digit {
        b'0'..=b'9' => digit - b'0',
        b'a'..=b'f' => digit - b'a' + 10,
        b'A'..=b'F' => digit - b'A' + 10,
        _ => panic!("not a hex digit"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_test_tokens_are_known_by_address() {
        let usdc =
            known(&address("0xd1108cbf3c5e7b44d7a58d4b2cda492a0ddf6a72")).expect("USDC is known");
        assert_eq!((usdc.symbol, usdc.decimals), ("USDC", 6));
        assert_eq!(usdc.address[0], 0xd1);
        assert_eq!(usdc.address[19], 0x72);
        assert!(known(&[0; 20]).is_none());
    }
}
