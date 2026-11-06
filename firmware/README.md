# QDay firmware

Screen test for the YD-ESP32-S3 N16R8 and the MSP2807 2.8" display (ILI9341, 240x320). It starts
the screen in landscape and cycles red, green, blue and a text page.

## Wiring

Both boards plug into one 400-hole breadboard, so each signal is a short straight jumper across the
centre gap. Unplug USB while wiring.

![Breadboard wiring for the screen and the ESP32-S3](wiring.svg)

Hold the breadboard with the ESP32's USB-C ports at the top. Columns run a (left) to j (right), and
strips are counted from the top, whatever numbers the breadboard prints.

- **ESP32:** plug the header that starts GND, 5V, GPIO14 into column d, strips 1-22, with GND on
  strip 1. The board hangs off the left, and its other header stays unplugged.
- **Screen:** plug its header into column g, strips 2-15, with VCC on strip 2. The board hangs off
  the right, over columns h-j.

Lay the first two wires before plugging in the screen and the short jumpers:

| Screen pin | Strip | ESP32 pin | Jumper |
|---|---|---|---|
| 1 VCC | 2 | 3V3 (strip 21) | e21 to f2, along the centre gap |
| 2 GND | 3 | GND (strip 1) | h1 to h3 under the screen, then e1 to f1 |
| 3 CS | 4 | GPIO13 | e4 to f4 |
| 4 RESET | 5 | GPIO12 | e5 to f5 |
| 5 DC/RS | 6 | GPIO11 | e6 to f6 |
| 6 SDI (MOSI) | 7 | GPIO10 | e7 to f7 |
| 7 SCK | 8 | GPIO9 | e8 to f8 |
| 8 LED | 9 | GPIO46 | e9 to f9 |
| 9 SDO (MISO) | 10 | Not connected | None |
| 10-14 touch pins | 11-15 | Not connected | None |

Never bridge strip 2 or strip 3 across the gap. Strip 2 on the ESP32 side is 5V, and strip 3 is
GPIO14.

## Build and flash

Connect the board with the port labelled **COM**, then run:

```bash
source ~/export-esp.sh
cargo run --release
```

To build without flashing, use `cargo build --release`.

## Expected result

The firmware turns the backlight on once the screen has started. The screen then shows red, green
and blue for one second each, followed by white text on black for three seconds. The cycle repeats,
and the serial monitor prints a `screen test:` line for each page.

| Problem | Likely cause |
|---|---|
| White screen | CS, RESET, DC, MOSI or SCK wiring |
| Dark screen | VCC, GND or LED wiring, or no firmware running |
| Flashing cannot connect | GPIO46 is a boot pin: unplug the e9 to f9 jumper, flash, then replace it |
| Red and blue swapped | Change `ColorOrder::Bgr` to `ColorOrder::Rgb` in `src/main.rs` |
| Text mirrored | Add `.flip_horizontal()` after `.rotate(Rotation::Deg270)` in `src/main.rs` |
| Text upside down | Change `Rotation::Deg270` to `Rotation::Deg90` in `src/main.rs` |
