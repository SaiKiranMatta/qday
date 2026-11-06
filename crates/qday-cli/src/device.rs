//! Talking to the device over its USB port.

use std::io::{ErrorKind, Read, Write};
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use qday_mldsa::SIGNATURE_BYTES;
use qday_wallet::frame::{Address, FrameTx, MlFrameTx};
use qday_wallet::hex;
use qday_wallet::protocol::{
    NextKey, PREFIX, Request, Response, error_code, parse_response, request_line, scheme_name,
};
use qday_wallet::settings::Scheme;
use qday_wallet::stealth::StealthSpend;
use serde_json::Value;

use crate::json;

/// Opening the port can reset the board, so the first request gets this many tries.
const HELLO_TRIES: u32 = 5;
const HELLO_WAIT: Duration = Duration::from_secs(1);
/// Signing waits for a person, and tree building takes minutes.
const LONG_WAIT: Duration = Duration::from_secs(300);
const BAUD: u32 = 115_200;
const READ_TIMEOUT: Duration = Duration::from_millis(100);

/// The device's keys, from a `PUBKEY` response.
#[derive(Debug, PartialEq, Eq)]
pub struct Status {
    pub root: [u8; 32],
    pub pub_seed: [u8; 32],
    pub next_index: u32,
    pub tree: u32,
    pub next: Option<NextKey>,
}

impl Status {
    /// The fields in `PUBKEY` order, as the scripts read them.
    pub fn line(&self) -> String {
        let mut fields = vec![
            hex::encode(&self.root),
            hex::encode(&self.pub_seed),
            self.next_index.to_string(),
            self.tree.to_string(),
        ];
        if let Some((root, pub_seed)) = &self.next {
            fields.push(hex::encode(root));
            fields.push(hex::encode(pub_seed));
        }
        fields.join(" ")
    }
}

/// A connection to a device, over anything that reads and writes.
pub struct Device<P> {
    port: P,
    pending: Vec<u8>,
}

/// Opens a serial port.
pub fn open(path: &str) -> Result<Device<Box<dyn serialport::SerialPort>>> {
    let port = serialport::new(path, BAUD)
        .timeout(READ_TIMEOUT)
        .open()
        .with_context(|| format!("couldn't open {path}"))?;
    Ok(Device::new(port))
}

impl<P: Read + Write> Device<P> {
    pub fn new(port: P) -> Self {
        Self {
            port,
            pending: Vec::new(),
        }
    }

    /// Reads one line, or `None` if `wait` passes first.
    fn read_line(&mut self, deadline: Instant) -> Result<Option<String>> {
        loop {
            if let Some(end) = self.pending.iter().position(|&byte| byte == b'\n') {
                let line: Vec<u8> = self.pending.drain(..=end).collect();
                return Ok(Some(String::from_utf8_lossy(&line).trim_end().to_string()));
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            let mut chunk = [0; 256];
            match self.port.read(&mut chunk) {
                Ok(0) => sleep(Duration::from_millis(5)),
                Ok(count) => self.pending.extend_from_slice(&chunk[..count]),
                Err(error) if error.kind() == ErrorKind::TimedOut => {}
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) => return Err(error).context("the port failed"),
            }
        }
    }

    /// Sends a request and waits for its response, skipping log output.
    fn exchange(&mut self, request: &Request, wait: Duration) -> Result<Option<Response>> {
        self.port
            .write_all(request_line(request).as_bytes())
            .context("the port failed")?;
        self.port.flush().context("the port failed")?;
        let deadline = Instant::now() + wait;
        while let Some(line) = self.read_line(deadline)? {
            if line.starts_with(PREFIX) {
                let response = parse_response(&line);
                return response
                    .map(Some)
                    .ok_or_else(|| anyhow!("bad response: {line}"));
            }
        }
        Ok(None)
    }

    fn request(&mut self, request: &Request) -> Result<Response> {
        match self.exchange(request, LONG_WAIT)? {
            Some(Response::Err(error)) => bail!("the device said {}", error_code(error)),
            Some(response) => Ok(response),
            None => bail!("the device did not answer"),
        }
    }

    /// Sends a first request, retrying because opening the port can reset the board.
    fn hello(&mut self, request: &Request) -> Result<Response> {
        for _ in 0..HELLO_TRIES {
            match self.exchange(request, HELLO_WAIT)? {
                Some(Response::Err(error)) => bail!("the device said {}", error_code(error)),
                Some(response) => return Ok(response),
                None => {}
            }
        }
        bail!("the device did not answer after {HELLO_TRIES} tries")
    }

    /// Asks for the keys.
    pub fn status(&mut self) -> Result<Status> {
        match self.hello(&Request::PubKey)? {
            Response::PubKey {
                root,
                pub_seed,
                next_index,
                tree,
                next,
            } => Ok(Status {
                root,
                pub_seed,
                next_index,
                tree,
                next,
            }),
            _ => bail!("the device gave the wrong response"),
        }
    }

    /// Asks which scheme is active. This works in either mode, so it's the safe first request.
    pub fn scheme(&mut self) -> Result<Scheme> {
        match self.hello(&Request::Scheme)? {
            Response::Scheme(scheme) => Ok(scheme),
            _ => bail!("the device gave the wrong response"),
        }
    }

    /// Reads the stored ML-DSA public key.
    pub fn ml_pubkey(&mut self) -> Result<Vec<u8>> {
        self.scheme()?;
        match self.request(&Request::MlPubKey)? {
            Response::MlPubKey(key) => Ok(key),
            _ => bail!("the device gave the wrong response"),
        }
    }

    /// Asks the device for its stealth address, which the person has to agree to.
    pub fn stealth_address(&mut self) -> Result<Vec<u8>> {
        self.scheme()?;
        match self.request(&Request::StealthAddress)? {
            Response::StealthAddress(address) => Ok(address),
            _ => bail!("the device gave the wrong response"),
        }
    }

    /// Asks the device for its view seed, which needs the person's passcode.
    pub fn view_key(&mut self) -> Result<[u8; 32]> {
        self.scheme()?;
        match self.request(&Request::ViewKey)? {
            Response::ViewKey(view) => Ok(view),
            _ => bail!("the device gave the wrong response"),
        }
    }

    /// Signs a spend of a stealth note on the device.
    pub fn spend_stealth(&mut self, spend: &StealthSpend) -> Result<Vec<u8>> {
        self.scheme()?;
        match self.request(&Request::SpendStealth(Box::new(*spend)))? {
            Response::Sig { signature, .. } if signature.len() == SIGNATURE_BYTES => Ok(signature),
            _ => bail!("the device gave the wrong response"),
        }
    }

    /// Signs a two-frame transfer on the device and returns the submission JSON.
    pub fn ml_send(
        &mut self,
        sender: Address,
        nonce: u64,
        to: Address,
        value: u128,
    ) -> Result<Value> {
        let scheme = self.scheme()?;
        if scheme != Scheme::MlDsa44 {
            bail!("the device is on {}: switch it first", scheme_name(scheme));
        }
        let tx = MlFrameTx::transfer(sender, nonce, to, value);
        match self.request(&Request::SignFrame(tx.encode(&[])))? {
            Response::Sig { signature, .. } if signature.len() == SIGNATURE_BYTES => {
                Ok(json::ml_submission(&tx, &signature))
            }
            _ => bail!("the device gave the wrong response"),
        }
    }

    fn sign(&mut self, tx: &FrameTx) -> Result<Value> {
        let request = Request::SignFrame(tx.encode(&[]));
        match self.request(&request)? {
            Response::Sig { signature, .. } => Ok(json::submission(tx, &signature)),
            _ => bail!("the device gave the wrong response"),
        }
    }

    /// Signs a transfer on the device and returns the submission JSON.
    pub fn send(&mut self, sender: Address, nonce: u64, to: Address, value: u128) -> Result<Value> {
        let status = self.status()?;
        self.sign(&FrameTx::transfer(
            sender,
            nonce,
            status.next_index,
            to,
            value,
        ))
    }

    /// Signs the account's first transaction, which deploys it with the device's active key, and
    /// returns the submission JSON.
    pub fn create(&mut self, sender: Address, factory: Address) -> Result<Value> {
        let status = self.status()?;
        self.sign(&FrameTx::create(
            sender,
            status.next_index,
            factory,
            status.root,
            status.pub_seed,
        ))
    }

    /// Signs a rotation to the prepared tree and returns the submission JSON.
    pub fn rotate(&mut self, sender: Address, nonce: u64) -> Result<Value> {
        let status = self.status()?;
        let (root, pub_seed) = status
            .next
            .context("the next tree isn't ready: run device-prepare first")?;
        self.sign(&FrameTx::rotation(
            sender,
            nonce,
            status.next_index,
            root,
            pub_seed,
        ))
    }

    /// Builds the next tree, then reports the keys.
    pub fn prepare(&mut self) -> Result<Status> {
        self.status()?;
        self.expect_ok(&Request::Prepare)?;
        self.status()
    }

    /// Switches to a prepared tree.
    pub fn activate(&mut self, tree: u32) -> Result<()> {
        self.status()?;
        self.expect_ok(&Request::Activate(tree))
    }

    /// Replaces the keys on the device.
    pub fn restore(&mut self, seed: [u8; 32], tree: u32, chain_next_index: u32) -> Result<()> {
        self.scheme()?;
        self.expect_ok(&Request::Restore {
            seed,
            tree,
            chain_next_index,
        })
    }

    fn expect_ok(&mut self, request: &Request) -> Result<()> {
        match self.request(request)? {
            Response::Ok => Ok(()),
            _ => bail!("the device gave the wrong response"),
        }
    }

    /// Prints whatever the device logs for `seconds`.
    pub fn log(&mut self, seconds: u64) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        while let Some(line) = self.read_line(deadline)? {
            println!("{line}");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::io::{Error, ErrorKind, Read, Result, Write};

    use qday_wallet::frame::{Action, FrameTx, MlFrameTx};
    use qday_wallet::protocol::{Request, Response, parse_request, response_line};
    use qday_wallet::settings::Scheme;
    use qday_wallet::stealth::StealthSpend;
    use qday_wallet::wallet::WalletError;

    use super::Device;

    const SENDER: [u8; 20] = [0x11; 20];
    const TO: [u8; 20] = [0x22; 20];

    fn status_response(next: Option<([u8; 32], [u8; 32])>) -> Response {
        Response::PubKey {
            root: [1; 32],
            pub_seed: [2; 32],
            next_index: 67,
            tree: 3,
            next,
        }
    }

    /// Answers each request with the next scripted response, after some log noise.
    struct FakeDevice {
        answers: VecDeque<Option<Response>>,
        requests: Vec<Request>,
        incoming: Vec<u8>,
        outgoing: VecDeque<u8>,
    }

    impl FakeDevice {
        fn new(answers: Vec<Option<Response>>) -> Self {
            Self {
                answers: answers.into(),
                requests: Vec::new(),
                incoming: Vec::new(),
                outgoing: VecDeque::new(),
            }
        }
    }

    impl Write for &mut FakeDevice {
        fn write(&mut self, data: &[u8]) -> Result<usize> {
            self.incoming.extend_from_slice(data);
            while let Some(end) = self.incoming.iter().position(|&byte| byte == b'\n') {
                let line: Vec<u8> = self.incoming.drain(..=end).collect();
                let text = String::from_utf8(line).expect("requests are text");
                let request = parse_request(text.trim_end())
                    .expect("this is a protocol line")
                    .expect("this is a valid request");
                self.requests.push(request);
                if let Some(Some(response)) = self.answers.pop_front() {
                    self.outgoing.extend(b"I (40) qday: working\n");
                    self.outgoing.extend(response_line(&response).as_bytes());
                }
            }
            Ok(data.len())
        }

        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
    }

    impl Read for &mut FakeDevice {
        fn read(&mut self, buffer: &mut [u8]) -> Result<usize> {
            if self.outgoing.is_empty() {
                return Err(Error::from(ErrorKind::TimedOut));
            }
            let count = buffer.len().min(self.outgoing.len());
            for slot in &mut buffer[..count] {
                *slot = self.outgoing.pop_front().expect("the count was checked");
            }
            Ok(count)
        }
    }

    #[test]
    fn log_lines_are_skipped() {
        let mut fake = FakeDevice::new(vec![Some(status_response(None))]);
        let status = Device::new(&mut fake).status().expect("the device answers");
        assert_eq!(status.next_index, 67);
        assert_eq!(status.tree, 3);
    }

    #[test]
    fn pubkey_is_retried_while_the_board_resets() {
        let mut fake = FakeDevice::new(vec![None, None, Some(status_response(None))]);
        Device::new(&mut fake)
            .status()
            .expect("the third try is answered");
        assert_eq!(fake.requests.len(), 3);
    }

    #[test]
    fn pubkey_gives_up_after_five_tries() {
        let mut fake = FakeDevice::new(vec![None; 8]);
        assert!(Device::new(&mut fake).status().is_err());
        assert_eq!(fake.requests.len(), 5);
    }

    #[test]
    fn send_builds_the_transaction_from_the_devices_next_index() {
        let mut fake = FakeDevice::new(vec![
            Some(status_response(None)),
            Some(Response::Sig {
                signature: vec![9; 2500],
                next: None,
            }),
        ]);
        let json = Device::new(&mut fake)
            .send(SENDER, 7, TO, 1000)
            .expect("the device signs");
        let Request::SignFrame(raw) = &fake.requests[1] else {
            panic!("the second request signs");
        };
        let tx = FrameTx::decode_unsigned(raw).expect("the request is a valid transaction");
        assert_eq!(tx.index, 67);
        assert_eq!(tx.nonce, 7);
        assert_eq!(
            tx.action,
            Action::Transfer {
                to: TO,
                value: 1000
            }
        );
        assert!(
            json["signatures"][0]["signature"]
                .as_str()
                .expect("text")
                .starts_with("0x0909")
        );
    }

    #[test]
    fn scheme_is_retried_while_the_board_resets() {
        let mut fake = FakeDevice::new(vec![None, None, Some(Response::Scheme(Scheme::MlDsa44))]);
        let scheme = Device::new(&mut fake).scheme().expect("it answers");
        assert_eq!(scheme, Scheme::MlDsa44);
        assert_eq!(fake.requests.len(), 3);
    }

    #[test]
    fn scheme_gives_up_after_five_tries() {
        let mut fake = FakeDevice::new(vec![None; 5]);
        assert!(Device::new(&mut fake).scheme().is_err());
        assert_eq!(fake.requests.len(), 5);
    }

    #[test]
    fn ml_pubkey_says_hello_with_scheme_then_reads_the_key() {
        let mut fake = FakeDevice::new(vec![
            Some(Response::Scheme(Scheme::MlDsa44)),
            Some(Response::MlPubKey(vec![6; 1312])),
        ]);
        let key = Device::new(&mut fake).ml_pubkey().expect("it answers");
        assert_eq!(key, vec![6; 1312]);
        assert_eq!(fake.requests, [Request::Scheme, Request::MlPubKey]);
    }

    #[test]
    fn ml_send_builds_the_two_frame_transfer() {
        let mut fake = FakeDevice::new(vec![
            Some(Response::Scheme(Scheme::MlDsa44)),
            Some(Response::Sig {
                signature: vec![9; 2420],
                next: None,
            }),
        ]);
        let json = Device::new(&mut fake)
            .ml_send(SENDER, 7, TO, 1000)
            .expect("the device signs");
        let want = MlFrameTx::transfer(SENDER, 7, TO, 1000);
        assert_eq!(fake.requests[1], Request::SignFrame(want.encode(&[])));
        assert_eq!(
            json,
            crate::json::ml_submission(&want, &vec![9; 2420]),
            "the submission carries the device's signature"
        );
    }

    #[test]
    fn ml_send_says_so_when_the_device_is_on_xmss() {
        let mut fake = FakeDevice::new(vec![Some(Response::Scheme(Scheme::Xmss))]);
        let error = Device::new(&mut fake)
            .ml_send(SENDER, 7, TO, 1000)
            .expect_err("nothing is signed");
        assert!(error.to_string().contains("xmss"), "{error}");
        assert_eq!(fake.requests, [Request::Scheme]);
    }

    #[test]
    fn ml_send_refuses_an_xmss_length_signature() {
        let mut fake = FakeDevice::new(vec![
            Some(Response::Scheme(Scheme::MlDsa44)),
            Some(Response::Sig {
                signature: vec![9; 2500],
                next: None,
            }),
        ]);
        assert!(Device::new(&mut fake).ml_send(SENDER, 7, TO, 1000).is_err());
    }

    #[test]
    fn restoring_works_in_ml_dsa_mode() {
        let mut fake = FakeDevice::new(vec![
            Some(Response::Scheme(Scheme::MlDsa44)),
            Some(Response::Ok),
        ]);
        Device::new(&mut fake)
            .restore([5; 32], 0, 0)
            .expect("the device restores");
        assert_eq!(fake.requests[0], Request::Scheme);
    }

    fn stealth_spend() -> StealthSpend {
        StealthSpend {
            ciphertext: [3; 1120],
            key_hash: [4; 32],
            pool: SENDER,
            to: TO,
            fee: 1000,
        }
    }

    #[test]
    fn stealth_address_says_hello_with_scheme_then_asks() {
        let mut fake = FakeDevice::new(vec![
            Some(Response::Scheme(Scheme::Xmss)),
            Some(Response::StealthAddress(vec![7; 4160])),
        ]);
        let got = Device::new(&mut fake)
            .stealth_address()
            .expect("it answers");
        assert_eq!(got, vec![7; 4160]);
        assert_eq!(fake.requests, [Request::Scheme, Request::StealthAddress]);
    }

    #[test]
    fn view_key_returns_the_seed() {
        let mut fake = FakeDevice::new(vec![
            Some(Response::Scheme(Scheme::MlDsa44)),
            Some(Response::ViewKey([8; 32])),
        ]);
        assert_eq!(
            Device::new(&mut fake).view_key().expect("it answers"),
            [8; 32]
        );
    }

    #[test]
    fn spend_stealth_returns_the_ml_dsa_signature() {
        let mut fake = FakeDevice::new(vec![
            Some(Response::Scheme(Scheme::Xmss)),
            Some(Response::Sig {
                signature: vec![9; 2420],
                next: None,
            }),
        ]);
        let signature = Device::new(&mut fake)
            .spend_stealth(&stealth_spend())
            .expect("the device signs");
        assert_eq!(signature, vec![9; 2420]);
        assert_eq!(
            fake.requests[1],
            Request::SpendStealth(Box::new(stealth_spend()))
        );
    }

    #[test]
    fn spend_stealth_refuses_another_signature_length() {
        let mut fake = FakeDevice::new(vec![
            Some(Response::Scheme(Scheme::Xmss)),
            Some(Response::Sig {
                signature: vec![9; 2176],
                next: None,
            }),
        ]);
        assert!(
            Device::new(&mut fake)
                .spend_stealth(&stealth_spend())
                .is_err()
        );
    }

    #[test]
    fn a_refusal_is_an_error_with_its_code() {
        let mut fake = FakeDevice::new(vec![
            Some(status_response(None)),
            Some(Response::Err(WalletError::Rejected)),
        ]);
        let error = Device::new(&mut fake)
            .send(SENDER, 7, TO, 1)
            .expect_err("it was refused");
        assert!(error.to_string().contains("rejected"), "{error}");
    }

    #[test]
    fn rotate_uses_the_devices_prepared_key() {
        let next = ([7; 32], [8; 32]);
        let mut fake = FakeDevice::new(vec![
            Some(status_response(Some(next))),
            Some(Response::Sig {
                signature: vec![9; 2500],
                next: Some(next),
            }),
        ]);
        Device::new(&mut fake)
            .rotate(SENDER, 1)
            .expect("the device signs");
        let Request::SignFrame(raw) = &fake.requests[1] else {
            panic!("the second request signs");
        };
        let tx = FrameTx::decode_unsigned(raw).expect("the request is a valid transaction");
        assert_eq!(
            tx.action,
            Action::Rotate {
                next_root: next.0,
                next_pub_seed: next.1
            }
        );
    }

    #[test]
    fn rotate_without_a_prepared_key_says_what_to_do() {
        let mut fake = FakeDevice::new(vec![Some(status_response(None))]);
        let error = Device::new(&mut fake)
            .rotate(SENDER, 1)
            .expect_err("nothing is prepared");
        assert!(error.to_string().contains("device-prepare"), "{error}");
    }

    #[test]
    fn prepare_reports_the_new_key() {
        let next = ([7; 32], [8; 32]);
        let mut fake = FakeDevice::new(vec![
            Some(status_response(None)),
            Some(Response::Ok),
            Some(status_response(Some(next))),
        ]);
        let status = Device::new(&mut fake)
            .prepare()
            .expect("the device prepares");
        assert_eq!(status.next, Some(next));
        assert_eq!(fake.requests[1], Request::Prepare);
    }

    #[test]
    fn activate_and_restore_send_their_fields() {
        let mut fake = FakeDevice::new(vec![
            Some(status_response(None)),
            Some(Response::Ok),
            Some(Response::Scheme(Scheme::Xmss)),
            Some(Response::Ok),
        ]);
        let mut device = Device::new(&mut fake);
        device.activate(4).expect("the device activates");
        device
            .restore([5; 32], 2, 100)
            .expect("the device restores");
        drop(device);
        assert_eq!(fake.requests[1], Request::Activate(4));
        let want = Request::Restore {
            seed: [5; 32],
            tree: 2,
            chain_next_index: 100,
        };
        assert_eq!(fake.requests[3], want);
    }

    #[test]
    fn status_lines_match_the_response_order() {
        let mut fake = FakeDevice::new(vec![Some(status_response(None))]);
        let line = Device::new(&mut fake)
            .status()
            .expect("the device answers")
            .line();
        let fields: Vec<&str> = line.split(' ').collect();
        assert_eq!(fields.len(), 4);
        assert_eq!(fields[2], "67");
        assert_eq!(fields[3], "3");
    }
}
