//! Touch calibration and saved settings.

use qday_xmss::keccak256;

use crate::flash::{Flash, layout};
use crate::keystore::StoreError;

/// Screen width in pixels.
pub const SCREEN_WIDTH: i64 = 320;
/// Screen height in pixels.
pub const SCREEN_HEIGHT: i64 = 240;

/// Where the three calibration crosses are drawn: top left, top right, bottom left.
pub const CROSSES: [(i32, i32); 3] = [(20, 20), (300, 20), (20, 220)];

/// Readings measured on the wired panel, used until a calibration has run.
const FACTORY_RAW: [(i32, i32); 3] = [(523, 567), (523, 3673), (3523, 567)];

/// Turns raw touch readings into pixels with an affine map, which also handles swapped axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Calibration {
    x: [i64; 3],
    y: [i64; 3],
    divisor: i64,
}

impl Calibration {
    /// The map that takes each raw reading to its cross. Returns `None` if the three readings lie
    /// on a line, which a real panel can't produce.
    #[must_use]
    pub fn from_points(raw: [(i32, i32); 3]) -> Option<Self> {
        let [p0, p1, p2] = raw.map(|(x, y)| (i64::from(x), i64::from(y)));
        let divisor = (p1.0 - p0.0) * (p2.1 - p0.1) - (p2.0 - p0.0) * (p1.1 - p0.1);
        if divisor == 0 {
            return None;
        }
        let solve = |t0: i64, t1: i64, t2: i64| {
            let a = (t1 - t0) * (p2.1 - p0.1) - (t2 - t0) * (p1.1 - p0.1);
            let b = (t2 - t0) * (p1.0 - p0.0) - (t1 - t0) * (p2.0 - p0.0);
            [a, b, t0 * divisor - a * p0.0 - b * p0.1]
        };
        let [c0, c1, c2] = CROSSES.map(|(x, y)| (i64::from(x), i64::from(y)));
        let x = solve(c0.0, c1.0, c2.0);
        let y = solve(c0.1, c1.1, c2.1);
        // A positive divisor keeps the rounding in `to_screen` simple.
        let sign = divisor.signum();
        Some(Self {
            x: x.map(|term| term * sign),
            y: y.map(|term| term * sign),
            divisor: divisor * sign,
        })
    }

    /// The panel's measured values, for use until a calibration has run.
    #[must_use]
    pub fn factory() -> Self {
        Self::from_points(FACTORY_RAW).unwrap_or(Self {
            x: [0; 3],
            y: [0; 3],
            divisor: 1,
        })
    }

    /// Converts a raw reading to a pixel on the screen.
    #[must_use]
    pub fn to_screen(&self, raw_x: i32, raw_y: i32) -> (i32, i32) {
        let apply = |terms: &[i64; 3], limit: i64| {
            let sum = terms[0] * i64::from(raw_x) + terms[1] * i64::from(raw_y) + terms[2];
            let pixel = (sum + self.divisor / 2).div_euclid(self.divisor);
            i32::try_from(pixel.clamp(0, limit - 1)).unwrap_or(0)
        };
        (apply(&self.x, SCREEN_WIDTH), apply(&self.y, SCREEN_HEIGHT))
    }

    fn to_bytes(self) -> [u8; CALIBRATION_BODY] {
        let mut bytes = [0; CALIBRATION_BODY];
        bytes[..4].copy_from_slice(CALIBRATION_MAGIC);
        let terms = [
            self.x[0],
            self.x[1],
            self.x[2],
            self.y[0],
            self.y[1],
            self.y[2],
            self.divisor,
        ];
        for (slot, term) in bytes[4..].chunks_exact_mut(8).zip(terms) {
            slot.copy_from_slice(&term.to_be_bytes());
        }
        bytes
    }

    fn from_bytes(bytes: &[u8; CALIBRATION_BODY]) -> Option<Self> {
        if !bytes.starts_with(CALIBRATION_MAGIC) {
            return None;
        }
        let mut terms = [0; 7];
        for (term, slot) in terms.iter_mut().zip(bytes[4..].chunks_exact(8)) {
            *term = i64::from_be_bytes(slot.try_into().ok()?);
        }
        let [x0, x1, x2, y0, y1, y2, divisor] = terms;
        (divisor > 0).then_some(Self {
            x: [x0, x1, x2],
            y: [y0, y1, y2],
            divisor,
        })
    }
}

const THEME_MAGIC: &[u8; 4] = b"QSET";
const THEME_BODY: usize = 8;
const CALIBRATION_MAGIC: &[u8; 4] = b"QCAL";
const CALIBRATION_BODY: usize = 60;
/// Bytes of keccak256 that follow each record.
const CHECK: usize = 8;

/// The two themes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeChoice {
    /// Standard Light.
    #[default]
    Light,
    /// Standard Dark.
    Dark,
}

/// Small saved records. A damaged record reads as missing.
pub struct Settings<F> {
    flash: F,
}

impl<F: Flash> Settings<F> {
    /// Settings on `flash`.
    pub fn new(flash: F) -> Self {
        Self { flash }
    }

    /// The saved theme, or the default.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be read.
    pub fn theme(&mut self) -> Result<ThemeChoice, StoreError> {
        let mut body = [0; THEME_BODY];
        if !self.read_record(layout::SETTINGS, &mut body)? || !body.starts_with(THEME_MAGIC) {
            return Ok(ThemeChoice::default());
        }
        Ok(if body[4] == 1 {
            ThemeChoice::Dark
        } else {
            ThemeChoice::Light
        })
    }

    /// Saves the theme.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be written.
    pub fn set_theme(&mut self, theme: ThemeChoice) -> Result<(), StoreError> {
        let mut body = [0; THEME_BODY];
        body[..4].copy_from_slice(THEME_MAGIC);
        body[4] = u8::from(theme == ThemeChoice::Dark);
        self.write_record(layout::SETTINGS, &body)
    }

    /// The saved calibration, if there is one.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be read.
    pub fn calibration(&mut self) -> Result<Option<Calibration>, StoreError> {
        let mut body = [0; CALIBRATION_BODY];
        if !self.read_record(layout::CALIBRATION, &mut body)? {
            return Ok(None);
        }
        Ok(Calibration::from_bytes(&body))
    }

    /// Saves a calibration.
    ///
    /// # Errors
    ///
    /// Fails if the flash can't be written.
    pub fn set_calibration(&mut self, calibration: &Calibration) -> Result<(), StoreError> {
        self.write_record(layout::CALIBRATION, &calibration.to_bytes())
    }

    /// Reads a record into `body`, returning whether its check matched.
    fn read_record(&mut self, offset: u32, body: &mut [u8]) -> Result<bool, StoreError> {
        let mut check = [0; CHECK];
        self.flash.read(offset, body).map_err(|_| StoreError)?;
        let check_at = offset + u32::try_from(body.len()).map_err(|_| StoreError)?;
        self.flash
            .read(check_at, &mut check)
            .map_err(|_| StoreError)?;
        Ok(keccak256(&[body])[..CHECK] == check)
    }

    fn write_record(&mut self, offset: u32, body: &[u8]) -> Result<(), StoreError> {
        self.flash.erase(offset).map_err(|_| StoreError)?;
        self.flash.write(offset, body).map_err(|_| StoreError)?;
        let check_at = offset + u32::try_from(body.len()).map_err(|_| StoreError)?;
        let check = keccak256(&[body]);
        self.flash
            .write(check_at, &check[..CHECK])
            .map_err(|_| StoreError)
    }
}
