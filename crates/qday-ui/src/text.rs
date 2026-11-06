//! Formatting into a fixed buffer, so screens can build strings without an allocator.

use core::fmt;

pub struct Text<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> Text<N> {
    pub fn new() -> Self {
        Text {
            buf: [0; N],
            len: 0,
        }
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }
}

/// Lets a `Text` go wherever a `&str` does.
impl<const N: usize> core::ops::Deref for Text<N> {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize> Default for Text<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> fmt::Write for Text<N> {
    /// Truncates instead of failing when full.
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &b in s.as_bytes() {
            if self.len == N {
                break;
            }
            self.buf[self.len] = b;
            self.len += 1;
        }
        Ok(())
    }
}

/// `format!` into a `Text<N>`.
#[macro_export]
macro_rules! text {
    ($n:literal, $($arg:tt)*) => {{
        let mut t = $crate::text::Text::<$n>::new();
        let _ = core::fmt::Write::write_fmt(&mut t, format_args!($($arg)*));
        t
    }};
}
