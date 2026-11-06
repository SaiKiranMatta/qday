//! The text protocol on the USB port. One request, one response.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use qday_mldsa::{PUBLIC_KEY_BYTES, SIGNATURE_BYTES};
use qday_xmss::{Hash, SIG_BYTES};

use crate::hex;
use crate::settings::Scheme;
use crate::wallet::WalletError;

/// Every protocol line starts with this. Anything else is log output.
pub const PREFIX: &str = "QDAY1 ";
/// The longest line, including the newline.
pub const MAX_LINE: usize = 8192;

/// A request from the laptop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// Report the keys.
    PubKey,
    /// Report the active scheme.
    Scheme,
    /// Report the stored ML-DSA public key.
    MlPubKey,
    /// Review and sign a raw unsigned frame transaction.
    SignFrame(Vec<u8>),
    /// Build the next tree.
    Prepare,
    /// Switch to a prepared tree.
    Activate(u32),
    /// Replace the keys.
    Restore {
        /// The seed.
        seed: Hash,
        /// The tree the chain is on.
        tree: u32,
        /// The chain's `nextIndex`.
        chain_next_index: u32,
    },
}

/// A next key, as (root, public seed).
pub type NextKey = (Hash, Hash);

/// A response from the device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Response {
    /// The keys.
    PubKey {
        /// The active root.
        root: Hash,
        /// The active public seed.
        pub_seed: Hash,
        /// The next one-time key.
        next_index: u32,
        /// The active tree number.
        tree: u32,
        /// The prepared key, if there is one.
        next: Option<NextKey>,
    },
    /// The active scheme.
    Scheme(Scheme),
    /// The 1,312-byte ML-DSA public key.
    MlPubKey(Vec<u8>),
    /// A signature.
    Sig {
        /// The signature: 2,500 bytes for XMSS, 2,420 for ML-DSA.
        signature: Vec<u8>,
        /// The prepared key, if there is one.
        next: Option<NextKey>,
    },
    /// Done.
    Ok,
    /// Failed.
    Err(WalletError),
}

const ERROR_CODES: [(WalletError, &str); 9] = [
    (WalletError::BadRequest, "bad-request"),
    (WalletError::Rejected, "rejected"),
    (WalletError::KeysExhausted, "keys-exhausted"),
    (WalletError::CounterFailure, "counter-failure"),
    (WalletError::RotationNeeded, "rotation-needed"),
    (WalletError::NothingPending, "nothing-pending"),
    (WalletError::StorageFailure, "storage-failure"),
    (WalletError::WrongScheme, "wrong-scheme"),
    (WalletError::SigningFailed, "signing-failed"),
];

/// The word for an error on the USB line.
#[must_use]
pub fn error_code(error: WalletError) -> &'static str {
    ERROR_CODES
        .iter()
        .find(|(known, _)| *known == error)
        .map_or("storage-failure", |(_, code)| code)
}

/// The word for a scheme on the USB line.
#[must_use]
pub fn scheme_name(scheme: Scheme) -> &'static str {
    match scheme {
        Scheme::Xmss => "xmss",
        Scheme::MlDsa44 => "ml-dsa-44",
    }
}

/// Reads a line. `None` means it isn't a protocol line, so it's log output.
#[must_use]
pub fn parse_request(line: &str) -> Option<Result<Request, WalletError>> {
    let body = line.trim_end_matches('\r').strip_prefix(PREFIX)?;
    Some(parse_body(body).ok_or(WalletError::BadRequest))
}

fn parse_body(body: &str) -> Option<Request> {
    let mut words = body.split(' ');
    let request = match words.next()? {
        "PUBKEY" => Request::PubKey,
        "SCHEME" => Request::Scheme,
        "MLPUBKEY" => Request::MlPubKey,
        "PREPARE" => Request::Prepare,
        "SIGNFRAME" => Request::SignFrame(hex::decode(words.next()?)?),
        "ACTIVATE" => Request::Activate(number(words.next()?)?),
        "RESTORE" => Request::Restore {
            seed: hex::decode_array(words.next()?)?,
            tree: number(words.next()?)?,
            chain_next_index: number(words.next()?)?,
        },
        _ => return None,
    };
    words.next().is_none().then_some(request)
}

fn number(text: &str) -> Option<u32> {
    text.bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| text.parse().ok())?
}

/// Writes a request as a line, ending in a newline.
#[must_use]
pub fn request_line(request: &Request) -> String {
    match request {
        Request::PubKey => format!("{PREFIX}PUBKEY\n"),
        Request::Scheme => format!("{PREFIX}SCHEME\n"),
        Request::MlPubKey => format!("{PREFIX}MLPUBKEY\n"),
        Request::Prepare => format!("{PREFIX}PREPARE\n"),
        Request::SignFrame(raw) => format!("{PREFIX}SIGNFRAME {}\n", hex::encode(raw)),
        Request::Activate(tree) => format!("{PREFIX}ACTIVATE {tree}\n"),
        Request::Restore {
            seed,
            tree,
            chain_next_index,
        } => {
            format!(
                "{PREFIX}RESTORE {} {tree} {chain_next_index}\n",
                hex::encode(seed)
            )
        }
    }
}

fn next_key_text(next: Option<&NextKey>) -> String {
    next.map_or_else(String::new, |(root, pub_seed)| {
        format!(" {} {}", hex::encode(root), hex::encode(pub_seed))
    })
}

/// Writes a response as a line, ending in a newline.
#[must_use]
pub fn response_line(response: &Response) -> String {
    match response {
        Response::PubKey {
            root,
            pub_seed,
            next_index,
            tree,
            next,
        } => format!(
            "{PREFIX}PUBKEY {} {} {next_index} {tree}{}\n",
            hex::encode(root),
            hex::encode(pub_seed),
            next_key_text(next.as_ref()),
        ),
        Response::Sig { signature, next } => format!(
            "{PREFIX}SIG {}{}\n",
            hex::encode(signature),
            next_key_text(next.as_ref())
        ),
        Response::Scheme(scheme) => format!("{PREFIX}SCHEME {}\n", scheme_name(*scheme)),
        Response::MlPubKey(key) => format!("{PREFIX}MLPUBKEY {}\n", hex::encode(key)),
        Response::Ok => format!("{PREFIX}OK\n"),
        Response::Err(error) => format!("{PREFIX}ERR {}\n", error_code(*error)),
    }
}

/// Reads an optional next key from the words left on a line.
fn parse_next<'a>(mut words: impl Iterator<Item = &'a str>) -> Result<Option<NextKey>, ()> {
    let Some(root) = words.next() else {
        return Ok(None);
    };
    let root = hex::decode_array(root).ok_or(())?;
    let pub_seed = hex::decode_array(words.next().ok_or(())?).ok_or(())?;
    if words.next().is_some() {
        return Err(());
    }
    Ok(Some((root, pub_seed)))
}

/// Reads a response. `None` means it isn't a valid protocol response.
#[must_use]
pub fn parse_response(line: &str) -> Option<Response> {
    let body = line.trim_end_matches('\r').strip_prefix(PREFIX)?;
    let mut words = body.split(' ');
    match words.next()? {
        "PUBKEY" => Some(Response::PubKey {
            root: hex::decode_array(words.next()?)?,
            pub_seed: hex::decode_array(words.next()?)?,
            next_index: number(words.next()?)?,
            tree: number(words.next()?)?,
            next: parse_next(words).ok()?,
        }),
        "SCHEME" => {
            let scheme = match words.next()? {
                "xmss" => Scheme::Xmss,
                "ml-dsa-44" => Scheme::MlDsa44,
                _ => return None,
            };
            words.next().is_none().then_some(Response::Scheme(scheme))
        }
        "MLPUBKEY" => {
            let key = hex::decode(words.next()?)?;
            (key.len() == PUBLIC_KEY_BYTES && words.next().is_none())
                .then_some(Response::MlPubKey(key))
        }
        "SIG" => {
            let signature = hex::decode(words.next()?)?;
            let next = parse_next(words).ok()?;
            // Only XMSS has a next key to report.
            let fits = signature.len() == SIG_BYTES
                || (signature.len() == SIGNATURE_BYTES && next.is_none());
            fits.then_some(Response::Sig { signature, next })
        }
        "OK" => words.next().is_none().then_some(Response::Ok),
        "ERR" => {
            let code = words.next()?;
            let (error, _) = ERROR_CODES.iter().find(|(_, known)| *known == code)?;
            words.next().is_none().then_some(Response::Err(*error))
        }
        _ => None,
    }
}

/// Collects bytes into lines, refusing any that are too long.
#[derive(Default)]
pub struct LineBuffer {
    line: Vec<u8>,
    overflowed: bool,
    finished: bool,
}

/// What a byte finished.
#[derive(Debug, PartialEq, Eq)]
pub enum Line<'a> {
    /// A whole line, without its newline.
    Text(&'a str),
    /// A line that was too long or wasn't text.
    Bad,
}

impl LineBuffer {
    /// Adds a byte, returning a line if that finished one.
    pub fn push(&mut self, byte: u8) -> Option<Line<'_>> {
        if self.finished {
            self.line.clear();
            self.overflowed = false;
            self.finished = false;
        }
        if byte == b'\n' {
            self.finished = true;
            if self.overflowed {
                return Some(Line::Bad);
            }
            return Some(core::str::from_utf8(&self.line).map_or(Line::Bad, Line::Text));
        }
        if self.line.len() + 1 < MAX_LINE {
            self.line.push(byte);
        } else {
            self.overflowed = true;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::settings::Scheme;
    use qday_mldsa::PUBLIC_KEY_BYTES;

    use alloc::format;
    use alloc::string::ToString;
    use alloc::vec;
    use alloc::vec::Vec;

    use super::{
        Line, LineBuffer, MAX_LINE, Request, Response, error_code, parse_request, parse_response,
        request_line, response_line,
    };
    use crate::hex;
    use crate::wallet::WalletError;

    const A: [u8; 32] = [0xAA; 32];
    const B: [u8; 32] = [0xBB; 32];

    fn a() -> alloc::string::String {
        hex::encode(&A)
    }

    fn b() -> alloc::string::String {
        hex::encode(&B)
    }

    #[test]
    fn error_codes_match_the_protocol() {
        let all = [
            (WalletError::BadRequest, "bad-request"),
            (WalletError::Rejected, "rejected"),
            (WalletError::KeysExhausted, "keys-exhausted"),
            (WalletError::CounterFailure, "counter-failure"),
            (WalletError::RotationNeeded, "rotation-needed"),
            (WalletError::NothingPending, "nothing-pending"),
            (WalletError::StorageFailure, "storage-failure"),
        ];
        for (error, code) in all {
            assert_eq!(error_code(error), code);
        }
    }

    #[test]
    fn log_output_is_ignored() {
        assert_eq!(parse_request("I (312) boot: ESP-IDF"), None);
        assert_eq!(parse_request(""), None);
        assert_eq!(parse_request("QDAY1"), None);
        assert_eq!(parse_request("QDAY2 PUBKEY"), None);
    }

    #[test]
    fn requests_parse() {
        assert_eq!(parse_request("QDAY1 PUBKEY"), Some(Ok(Request::PubKey)));
        assert_eq!(parse_request("QDAY1 PREPARE"), Some(Ok(Request::Prepare)));
        assert_eq!(
            parse_request("QDAY1 ACTIVATE 3"),
            Some(Ok(Request::Activate(3)))
        );
        assert_eq!(
            parse_request("QDAY1 SIGNFRAME 0x06c0"),
            Some(Ok(Request::SignFrame(vec![0x06, 0xc0])))
        );
        let line = format!("QDAY1 RESTORE {} 2 100", a());
        let want = Request::Restore {
            seed: A,
            tree: 2,
            chain_next_index: 100,
        };
        assert_eq!(parse_request(&line), Some(Ok(want)));
    }

    #[test]
    fn a_carriage_return_is_tolerated() {
        assert_eq!(parse_request("QDAY1 PUBKEY\r"), Some(Ok(Request::PubKey)));
    }

    #[test]
    fn malformed_requests_are_bad_requests() {
        let bad = Some(Err(WalletError::BadRequest));
        for line in [
            "QDAY1 ",
            "QDAY1 NOPE",
            "QDAY1 PUBKEY extra",
            "QDAY1 SIGNFRAME",
            "QDAY1 SIGNFRAME 06c0",
            "QDAY1 SIGNFRAME 0x06c",
            "QDAY1 SIGNFRAME 0x06 0x07",
            "QDAY1 ACTIVATE",
            "QDAY1 ACTIVATE x",
            "QDAY1 ACTIVATE -1",
            "QDAY1 RESTORE 0x11 2 100",
            "QDAY1 SIGNHASH 0xaa",
        ] {
            assert_eq!(parse_request(line), bad, "{line}");
        }
    }

    #[test]
    fn there_is_no_way_to_ask_for_a_bare_hash_signature() {
        let line = format!("QDAY1 SIGN {}", a());
        assert_eq!(parse_request(&line), Some(Err(WalletError::BadRequest)));
    }

    #[test]
    fn requests_round_trip() {
        for request in [
            Request::PubKey,
            Request::Prepare,
            Request::Activate(7),
            Request::SignFrame(vec![6, 1, 2]),
            Request::Restore {
                seed: B,
                tree: 1,
                chain_next_index: 9,
            },
        ] {
            let line = request_line(&request);
            assert!(line.starts_with("QDAY1 ") && line.ends_with('\n'), "{line}");
            assert_eq!(parse_request(line.trim_end()), Some(Ok(request)));
        }
    }

    #[test]
    fn pubkey_lines_match_the_protocol() {
        let mut response = Response::PubKey {
            root: A,
            pub_seed: B,
            next_index: 67,
            tree: 1,
            next: None,
        };
        let want = format!("QDAY1 PUBKEY {} {} 67 1\n", a(), b());
        assert_eq!(response_line(&response), want);

        if let Response::PubKey { next, .. } = &mut response {
            *next = Some((B, A));
        }
        let want = format!("QDAY1 PUBKEY {} {} 67 1 {} {}\n", a(), b(), b(), a());
        assert_eq!(response_line(&response), want);
    }

    #[test]
    fn sig_lines_match_the_protocol() {
        let signature = vec![0xCD; 2500];
        let response = Response::Sig {
            signature: signature.clone(),
            next: None,
        };
        assert_eq!(
            response_line(&response),
            format!("QDAY1 SIG {}\n", hex::encode(&signature))
        );
    }

    #[test]
    fn scheme_and_ml_key_requests_match_the_protocol() {
        assert_eq!(request_line(&Request::Scheme), "QDAY1 SCHEME\n");
        assert_eq!(request_line(&Request::MlPubKey), "QDAY1 MLPUBKEY\n");
        for request in [Request::Scheme, Request::MlPubKey] {
            let line = request_line(&request);
            assert_eq!(parse_request(line.trim_end()), Some(Ok(request)));
        }
    }

    #[test]
    fn scheme_and_ml_key_requests_take_no_arguments() {
        for line in ["QDAY1 SCHEME xmss", "QDAY1 MLPUBKEY 0"] {
            assert_eq!(parse_request(line), Some(Err(WalletError::BadRequest)));
        }
    }

    #[test]
    fn an_ml_key_with_extra_words_is_refused() {
        let line = format!("QDAY1 MLPUBKEY {} extra", hex::encode(&vec![4; 1312]));
        assert_eq!(parse_response(&line), None);
    }

    #[test]
    fn scheme_responses_match_the_protocol() {
        assert_eq!(
            response_line(&Response::Scheme(Scheme::Xmss)),
            "QDAY1 SCHEME xmss\n"
        );
        assert_eq!(
            response_line(&Response::Scheme(Scheme::MlDsa44)),
            "QDAY1 SCHEME ml-dsa-44\n"
        );
    }

    #[test]
    fn ml_key_responses_match_the_protocol() {
        let key = vec![0xAB; 1312];
        assert_eq!(
            response_line(&Response::MlPubKey(key.clone())),
            format!("QDAY1 MLPUBKEY {}\n", hex::encode(&key))
        );
    }

    #[test]
    fn the_new_errors_have_their_codes() {
        assert_eq!(
            response_line(&Response::Err(WalletError::WrongScheme)),
            "QDAY1 ERR wrong-scheme\n"
        );
        assert_eq!(
            response_line(&Response::Err(WalletError::SigningFailed)),
            "QDAY1 ERR signing-failed\n"
        );
    }

    #[test]
    fn an_ml_dsa_signature_is_accepted_at_its_own_length() {
        let response = Response::Sig {
            signature: vec![3; 2420],
            next: None,
        };
        let line = response_line(&response);
        assert_eq!(parse_response(line.trim_end()), Some(response));
    }

    #[test]
    fn other_signature_lengths_are_refused() {
        for length in [2419, 2421, 2499, 2501] {
            let line = format!("QDAY1 SIG {}", hex::encode(&vec![3; length]));
            assert_eq!(parse_response(&line), None, "{length}");
        }
    }

    #[test]
    fn an_ml_dsa_signature_carries_no_next_key() {
        let line = format!("QDAY1 SIG {} {} {}", hex::encode(&vec![3; 2420]), a(), b());
        assert_eq!(parse_response(&line), None);
    }

    #[test]
    fn ok_and_errors_match_the_protocol() {
        assert_eq!(response_line(&Response::Ok), "QDAY1 OK\n");
        assert_eq!(
            response_line(&Response::Err(WalletError::Rejected)),
            "QDAY1 ERR rejected\n"
        );
    }

    #[test]
    fn responses_round_trip() {
        for response in [
            Response::Ok,
            Response::Err(WalletError::RotationNeeded),
            Response::Err(WalletError::WrongScheme),
            Response::Err(WalletError::SigningFailed),
            Response::Scheme(Scheme::Xmss),
            Response::Scheme(Scheme::MlDsa44),
            Response::MlPubKey(vec![4; 1312]),
            Response::PubKey {
                root: A,
                pub_seed: B,
                next_index: 0,
                tree: 0,
                next: Some((B, A)),
            },
            Response::PubKey {
                root: A,
                pub_seed: B,
                next_index: 1023,
                tree: 9,
                next: None,
            },
            Response::Sig {
                signature: vec![1; 2500],
                next: Some((A, B)),
            },
            Response::Sig {
                signature: vec![2; 2500],
                next: None,
            },
        ] {
            let line = response_line(&response);
            assert_eq!(parse_response(line.trim_end()), Some(response));
        }
    }

    #[test]
    fn an_ml_key_response_fits_in_a_line() {
        let response = Response::MlPubKey(vec![0; PUBLIC_KEY_BYTES]);
        assert!(response_line(&response).len() <= MAX_LINE);
    }

    #[test]
    fn a_signed_response_fits_in_a_line() {
        let response = Response::Sig {
            signature: vec![0; 2500],
            next: Some((A, B)),
        };
        assert!(response_line(&response).len() <= MAX_LINE);
    }

    #[test]
    fn bad_responses_are_refused() {
        for line in [
            "hello",
            "QDAY1 WHAT",
            "QDAY1 ERR nonsense",
            "QDAY1 SIG 0x00",
            "QDAY1 SCHEME",
            "QDAY1 SCHEME sphincs",
            "QDAY1 SCHEME xmss extra",
            "QDAY1 MLPUBKEY",
            "QDAY1 MLPUBKEY 0x00",
            "QDAY1 MLPUBKEY 0x00 extra",
            "QDAY1 PUBKEY 0x00 0x00 1 1",
        ] {
            assert_eq!(parse_response(line), None, "{line}");
        }
    }

    fn feed(buffer: &mut LineBuffer, bytes: &[u8]) -> Vec<alloc::string::String> {
        let mut lines = Vec::new();
        for &byte in bytes {
            match buffer.push(byte) {
                Some(Line::Text(text)) => lines.push(text.to_string()),
                Some(Line::Bad) => lines.push("<bad>".to_string()),
                None => {}
            }
        }
        lines
    }

    #[test]
    fn lines_split_on_newlines() {
        let mut buffer = LineBuffer::default();
        let got = feed(&mut buffer, b"one\ntwo\nthr");
        assert_eq!(got, ["one", "two"]);
        assert_eq!(feed(&mut buffer, b"ee\n"), ["three"]);
    }

    #[test]
    fn the_longest_line_fits() {
        let mut buffer = LineBuffer::default();
        let mut bytes = vec![b'a'; MAX_LINE - 1];
        bytes.push(b'\n');
        assert_eq!(feed(&mut buffer, &bytes).len(), 1);
        assert_ne!(feed(&mut LineBuffer::default(), &bytes), ["<bad>"]);
    }

    #[test]
    fn a_long_line_is_bad_and_the_next_one_works() {
        let mut buffer = LineBuffer::default();
        let mut bytes = vec![b'a'; MAX_LINE];
        bytes.push(b'\n');
        bytes.extend_from_slice(b"QDAY1 PUBKEY\n");
        assert_eq!(feed(&mut buffer, &bytes), ["<bad>", "QDAY1 PUBKEY"]);
    }

    #[test]
    fn bytes_that_are_not_text_are_bad() {
        let mut buffer = LineBuffer::default();
        assert_eq!(feed(&mut buffer, &[0xFF, 0xFE, b'\n']), ["<bad>"]);
    }
}
