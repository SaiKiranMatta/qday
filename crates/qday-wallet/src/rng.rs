//! Randomness from the hardware.

/// A source of random bytes.
pub trait Rng {
    /// Fills `buffer` with random bytes.
    fn fill(&mut self, buffer: &mut [u8]);

    /// A random number below `limit`, with no bias.
    fn below(&mut self, limit: u32) -> u32 {
        let zone = u32::MAX - (u32::MAX % limit);
        loop {
            let mut bytes = [0; 4];
            self.fill(&mut bytes);
            let value = u32::from_be_bytes(bytes);
            if value < zone {
                return value % limit;
            }
        }
    }
}
