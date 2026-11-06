//! RLP, with a decoder that accepts only the one canonical encoding of each value.

use alloc::vec::Vec;

/// Why bytes are not valid canonical RLP of the expected shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RlpError;

/// Appends a byte string.
pub fn put_bytes(out: &mut Vec<u8>, data: &[u8]) {
    if let [byte @ 0..=0x7f] = data {
        out.push(*byte);
    } else {
        put_length(out, STRING, data.len());
        out.extend_from_slice(data);
    }
}

/// Appends an integer: big-endian with no leading zeros, so zero is the empty string.
pub fn put_uint(out: &mut Vec<u8>, value: u128) {
    let bytes = value.to_be_bytes();
    let zeros = bytes.iter().take_while(|&&byte| byte == 0).count();
    put_bytes(out, &bytes[zeros..]);
}

/// Appends a list whose items are already encoded in `items`.
pub fn put_list(out: &mut Vec<u8>, items: &[u8]) {
    put_length(out, LIST, items.len());
    out.extend_from_slice(items);
}

/// First prefix byte of each kind of item.
const STRING: u8 = 0x80;
const LIST: u8 = 0xc0;
/// Lengths up to this fit in the prefix byte itself.
const SHORT_MAX: usize = 55;

fn put_length(out: &mut Vec<u8>, kind: u8, length: usize) {
    if length <= SHORT_MAX {
        out.push(kind + u8::try_from(length).unwrap_or(0));
    } else {
        let bytes = length.to_be_bytes();
        let zeros = bytes.iter().take_while(|&&byte| byte == 0).count();
        let size = bytes.len() - zeros;
        out.push(kind + 55 + u8::try_from(size).unwrap_or(0));
        out.extend_from_slice(&bytes[zeros..]);
    }
}

/// Reads items one at a time from a list's contents.
pub struct Reader<'a> {
    rest: &'a [u8],
}

impl<'a> Reader<'a> {
    /// Reads from `data`.
    #[must_use]
    pub fn new(data: &'a [u8]) -> Self {
        Self { rest: data }
    }

    /// Reads a byte string.
    ///
    /// # Errors
    ///
    /// Fails if the next item is a list, truncated or not canonical.
    pub fn bytes(&mut self) -> Result<&'a [u8], RlpError> {
        let (&first, after) = self.rest.split_first().ok_or(RlpError)?;
        if first < STRING {
            let (item, rest) = self.rest.split_at(1);
            self.rest = rest;
            return Ok(item);
        }
        if first >= LIST {
            return Err(RlpError);
        }
        let (payload, rest) = take_payload(first - STRING, after)?;
        if let [0..=0x7f] = payload {
            return Err(RlpError);
        }
        self.rest = rest;
        Ok(payload)
    }

    /// Reads an integer of at most 16 bytes.
    ///
    /// # Errors
    ///
    /// Fails if the item is not a canonical integer, or is too large.
    pub fn uint(&mut self) -> Result<u128, RlpError> {
        let bytes = self.bytes()?;
        if bytes.len() > 16 || bytes.first() == Some(&0) {
            return Err(RlpError);
        }
        Ok(bytes
            .iter()
            .fold(0, |value, &byte| (value << 8) | u128::from(byte)))
    }

    /// Reads a list and returns a reader over its contents.
    ///
    /// # Errors
    ///
    /// Fails if the next item is a byte string, truncated or not canonical.
    pub fn list(&mut self) -> Result<Reader<'a>, RlpError> {
        let (&first, after) = self.rest.split_first().ok_or(RlpError)?;
        if first < LIST {
            return Err(RlpError);
        }
        let (payload, rest) = take_payload(first - LIST, after)?;
        self.rest = rest;
        Ok(Reader::new(payload))
    }

    /// Succeeds only if every byte has been read.
    ///
    /// # Errors
    ///
    /// Fails if anything is left over.
    pub fn finish(self) -> Result<(), RlpError> {
        if self.rest.is_empty() {
            Ok(())
        } else {
            Err(RlpError)
        }
    }
}

/// Splits an item's payload from what follows it. `tag` is the prefix byte minus the item kind.
fn take_payload(tag: u8, after: &[u8]) -> Result<(&[u8], &[u8]), RlpError> {
    let (length, after) = if usize::from(tag) <= SHORT_MAX {
        (usize::from(tag), after)
    } else {
        let size = usize::from(tag) - SHORT_MAX;
        if size > core::mem::size_of::<u32>() || after.len() < size {
            return Err(RlpError);
        }
        let (bytes, after) = after.split_at(size);
        if bytes[0] == 0 {
            return Err(RlpError);
        }
        let length = bytes
            .iter()
            .fold(0, |value, &byte| (value << 8) | usize::from(byte));
        if length <= SHORT_MAX {
            return Err(RlpError);
        }
        (length, after)
    };
    if after.len() < length {
        return Err(RlpError);
    }
    Ok(after.split_at(length))
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;

    use super::{Reader, RlpError, put_bytes, put_list, put_uint};

    fn encoded_uint(value: u128) -> Vec<u8> {
        let mut out = Vec::new();
        put_uint(&mut out, value);
        out
    }

    fn encoded_bytes(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        put_bytes(&mut out, data);
        out
    }

    #[test]
    fn integers_have_no_leading_zeros() {
        assert_eq!(encoded_uint(0), [0x80]);
        assert_eq!(encoded_uint(15), [0x0f]);
        assert_eq!(encoded_uint(127), [0x7f]);
        assert_eq!(encoded_uint(128), [0x81, 0x80]);
        assert_eq!(encoded_uint(1024), [0x82, 0x04, 0x00]);
    }

    #[test]
    fn short_strings_get_one_prefix_byte() {
        assert_eq!(encoded_bytes(b""), [0x80]);
        assert_eq!(encoded_bytes(b"\x00"), [0x00]);
        assert_eq!(encoded_bytes(b"dog"), [0x83, b'd', b'o', b'g']);
    }

    #[test]
    fn long_strings_carry_their_length() {
        let got = encoded_bytes(&[7; 56]);
        assert_eq!(got[..2], [0xb8, 56]);
        assert_eq!(got.len(), 58);
        let got = encoded_bytes(&[7; 2500]);
        assert_eq!(got[..3], [0xb9, 0x09, 0xc4]);
    }

    #[test]
    fn lists_wrap_their_items() {
        let mut items = Vec::new();
        put_bytes(&mut items, b"cat");
        put_bytes(&mut items, b"dog");
        let mut out = Vec::new();
        put_list(&mut out, &items);
        assert_eq!(out, [0xc8, 0x83, b'c', b'a', b't', 0x83, b'd', b'o', b'g']);

        let mut empty = Vec::new();
        put_list(&mut empty, &[]);
        assert_eq!(empty, [0xc0]);

        let mut long = Vec::new();
        put_list(&mut long, &[1; 60]);
        assert_eq!(long[..2], [0xf8, 60]);
    }

    #[test]
    fn everything_written_reads_back() {
        for value in [0, 1, 127, 128, 255, 256, 1337, 2_000_000_000, u128::MAX] {
            let data = encoded_uint(value);
            let mut reader = Reader::new(&data);
            assert_eq!(reader.uint(), Ok(value));
            assert_eq!(reader.finish(), Ok(()));
        }
        for size in [0, 1, 55, 56, 255, 256, 2500] {
            let payload = vec![0xAB; size];
            let data = encoded_bytes(&payload);
            let mut reader = Reader::new(&data);
            assert_eq!(reader.bytes(), Ok(&payload[..]));
            assert_eq!(reader.finish(), Ok(()));
        }
    }

    #[test]
    fn a_list_reads_as_a_nested_reader() {
        let data = [0xc2, 0x01, 0x02, 0x03];
        let mut reader = Reader::new(&data);
        let mut inner = reader.list().expect("the list is valid");
        assert_eq!(inner.uint(), Ok(1));
        assert_eq!(inner.uint(), Ok(2));
        assert_eq!(inner.finish(), Ok(()));
        assert_eq!(reader.uint(), Ok(3));
        assert_eq!(reader.finish(), Ok(()));
    }

    #[test]
    fn a_small_byte_with_a_prefix_is_refused() {
        assert_eq!(Reader::new(&[0x81, 0x05]).bytes(), Err(RlpError));
    }

    #[test]
    fn the_long_form_for_a_short_string_is_refused() {
        let mut data = vec![0xb8, 55];
        data.extend([1; 55]);
        assert_eq!(Reader::new(&data).bytes(), Err(RlpError));
    }

    #[test]
    fn a_length_with_a_leading_zero_is_refused() {
        let mut data = vec![0xb9, 0x00, 60];
        data.extend([1; 60]);
        assert_eq!(Reader::new(&data).bytes(), Err(RlpError));
    }

    #[test]
    fn an_integer_with_a_leading_zero_is_refused() {
        assert_eq!(Reader::new(&[0x82, 0x00, 0x01]).uint(), Err(RlpError));
        assert_eq!(Reader::new(&[0x00]).uint(), Err(RlpError));
    }

    #[test]
    fn an_oversized_integer_is_refused() {
        let mut data = vec![0x80 + 17];
        data.extend([1; 17]);
        assert_eq!(Reader::new(&data).uint(), Err(RlpError));
    }

    #[test]
    fn truncated_input_is_refused() {
        assert_eq!(Reader::new(&[]).bytes(), Err(RlpError));
        assert_eq!(Reader::new(&[0x83, b'd', b'o']).bytes(), Err(RlpError));
        assert_eq!(Reader::new(&[0xb9, 0x09]).bytes(), Err(RlpError));
        assert!(Reader::new(&[0xc3, 0x01]).list().is_err());
        assert_eq!(Reader::new(&[0xb9, 0xff, 0xff, 1]).bytes(), Err(RlpError));
    }

    #[test]
    fn huge_declared_lengths_are_refused_without_overflowing() {
        let data = [0xbf, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
        assert_eq!(Reader::new(&data).bytes(), Err(RlpError));
    }

    #[test]
    fn a_list_where_bytes_belong_is_refused() {
        assert_eq!(Reader::new(&[0xc0]).bytes(), Err(RlpError));
        assert!(Reader::new(&[0x80]).list().is_err());
    }

    #[test]
    fn leftover_bytes_are_refused() {
        let data = [0x01, 0x02];
        let mut reader = Reader::new(&data);
        assert_eq!(reader.uint(), Ok(1));
        assert_eq!(reader.finish(), Err(RlpError));
    }
}
