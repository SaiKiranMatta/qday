# QDay firmware

Screen test for the YD-ESP32-S3 N16R8 and the MSP2807 2.8" display (ILI9341, 240x320). It starts
the screen in landscape and cycles red, green, blue and a text page.

## Wiring

Use the labels printed on the board. The diagram lists the pins in wire order, not board order.

![Wiring from the screen to the ESP32-S3](wiring.svg)

| Screen pin | Connect to |
|---|---|
| 1 VCC | 3V3 |
| 2 GND | GND |
| 3 CS | GPIO10 |
| 4 RESET | GPIO14 |
| 5 DC/RS | GPIO9 |
| 6 SDI (MOSI) | GPIO11 |
| 7 SCK | GPIO12 |
| 8 LED | 3V3 |
| 9 SDO (MISO) | Not connected |
| 10-14 touch pins | Not connected |

## Build and flash

Connect the board with the port labelled **COM**, then run:

```bash
source ~/export-esp.sh
cargo run --release
```

To build without flashing, use `cargo build --release`.

## Expected result

The backlight comes on when the board has power. The screen then shows red, green and blue for one
second each, followed by white text on black for three seconds. The cycle repeats, and the serial
monitor prints a `screen test:` line for each page.

| Problem | Likely cause |
|---|---|
| White screen | CS, RESET, DC, MOSI or SCK wiring, or no firmware running |
| Dark screen | VCC, GND or LED wiring |
| Red and blue swapped | Change `ColorOrder::Bgr` to `ColorOrder::Rgb` in `src/main.rs` |
| Text mirrored | Add `.flip_horizontal()` after `.rotate(Rotation::Deg90)` in `src/main.rs` |
| Text upside down | Change `Rotation::Deg90` to `Rotation::Deg270` in `src/main.rs` |
