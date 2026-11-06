//! The 32-byte address that separates every hash in the scheme.

const CHAIN: u8 = 0;
const LEAF: u8 = 1;
const NODE: u8 = 2;
const SECRET: u8 = 3;

/// A hash address: byte 0 is the type, and `a`, `b` and `c` sit big-endian at bytes 20, 24, 28.
pub struct Adrs;

impl Adrs {
    /// Address of chain `chain` of one-time key `leaf`, at `step`.
    #[must_use]
    pub fn chain(leaf: u32, chain: u32, step: u32) -> [u8; 32] {
        pack(CHAIN, leaf, chain, step)
    }

    /// Address of the leaf hash of one-time key `leaf`.
    #[must_use]
    pub fn leaf(leaf: u32) -> [u8; 32] {
        pack(LEAF, leaf, 0, 0)
    }

    /// Address of tree node `index` at `height`.
    #[must_use]
    pub fn node(height: u32, index: u32) -> [u8; 32] {
        pack(NODE, height, index, 0)
    }

    /// Address of the secret start of chain `chain` of one-time key `leaf`.
    #[must_use]
    pub fn secret(leaf: u32, chain: u32) -> [u8; 32] {
        pack(SECRET, leaf, chain, 0)
    }
}

fn pack(kind: u8, a: u32, b: u32, c: u32) -> [u8; 32] {
    let mut out = [0; 32];
    out[0] = kind;
    out[20..24].copy_from_slice(&a.to_be_bytes());
    out[24..28].copy_from_slice(&b.to_be_bytes());
    out[28..32].copy_from_slice(&c.to_be_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::Adrs;

    fn expected(kind: u8, a: [u8; 4], b: [u8; 4], c: [u8; 4]) -> [u8; 32] {
        let mut out = [0; 32];
        out[0] = kind;
        out[20..24].copy_from_slice(&a);
        out[24..28].copy_from_slice(&b);
        out[28..32].copy_from_slice(&c);
        out
    }

    #[test]
    fn chain_is_type_0_with_leaf_chain_step() {
        let got = Adrs::chain(0x0102_0304, 0x0506_0708, 0x090a_0b0c);
        let want = expected(0, [1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12]);
        assert_eq!(got, want);
    }

    #[test]
    fn leaf_is_type_1_with_leaf_only() {
        assert_eq!(
            Adrs::leaf(0x0102_0304),
            expected(1, [1, 2, 3, 4], [0; 4], [0; 4])
        );
    }

    #[test]
    fn node_is_type_2_with_height_and_index() {
        let want = expected(2, [0, 0, 0, 10], [0, 0, 1, 0], [0; 4]);
        assert_eq!(Adrs::node(10, 256), want);
    }

    #[test]
    fn secret_is_type_3_with_leaf_and_chain() {
        let want = expected(3, [0, 0, 3, 255], [0, 0, 0, 66], [0; 4]);
        assert_eq!(Adrs::secret(1023, 66), want);
    }
}
