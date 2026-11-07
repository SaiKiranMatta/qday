# Locking the chip

This is the plan for locking the ESP32-S3, and the code that carries it out. **No board has been
locked.** A board with no seal key in its fuses runs exactly as it did before this code existed.

Burning fuses can't be undone. A mistake here can leave a board that never boots again, so read
all of this first and try it on a spare board, not the one you use.

## What it protects

| An attacker with your board tries to | Today | Locked |
|---|---|---|
| Read the seed with a flash reader | Works | Gets a masked value, useless off this chip |
| Test passcodes against a flash copy | Works: only a million codes | Needs the chip's key |
| Erase the passcode and choose a new one | Works | Wipes the keys instead |
| Write in a passcode they know | Works | Refused: it needs the chip's key |
| Flash their own firmware | Works | The chip won't run it |
| Read memory over JTAG | Works | JTAG is off |
| Read flash over USB in download mode | Works | Download mode can only write |

### What it doesn't protect

- **Rolling the flash back.** Someone with a clip on the flash chip can restore an old copy. That
  resets the failure count, so passcode guesses can continue, ten at a time at most. It also rewinds
  the one-way counter, which could reuse a one-time key. Only a counter outside the flash stops
  this, which is the job planned for the ATECC608A.
- **Physical attacks on the chip itself,** such as voltage or clock glitching. The ESP32-S3 isn't a
  secure element.
- **Timing and power analysis.** The signers aren't constant-time.
- **A passcode someone watched you enter,** or a seed phrase someone found.

## Why flash encryption alone isn't enough

Flash encryption covers the bootloader, the partition table and the app. It doesn't cover the
`qday` partition, and it can't. The counter and the passcode log work by clearing bits in place,
which encrypted flash can't do. The encrypted write in `esp-storage` also erases a whole sector and
rewrites it, which would undo every power-cut guarantee.

So the wallet protects its own two secrets with a third key, the **seal key**. It sits in a fuse
that software can use through the chip's HMAC peripheral but can't read:

- **The seed** is stored masked with a hash made from that key and the key's public half. See
  `crates/qday-wallet/src/keystore.rs`.
- **The passcode** is hashed with that key. See `crates/qday-wallet/src/passcode.rs`.
- **The order of first setup changes.** The passcode comes first, then the keys. A sealed wallet
  with keys and no passcode can only mean the passcode was erased, so it wipes the keys.

The firmware finds the seal key by reading the fuses at start. If there isn't one, nothing changes.

## What changes once it's locked

- **`cargo run` stops working.** The board only runs firmware signed with your key.
- **The wallet is erased.** The plan erases the flash, so no plain seed survives. Restore from your
  24 words afterwards.
- **Updates need both laptop keys.** There's no over-the-air update here. To update, sign the new
  image, encrypt it with the flash encryption key, and write it over USB. If you delete that key, as
  Espressif advises, the firmware is frozen for good.
- **Download mode can only write,** so you can't read the flash back to debug it.
- **USB still works.** The plan burns `DIS_USB_JTAG` and never `DIS_USB_SERIAL_JTAG`, which would
  cut off the port the wallet talks through. Espressif's pages don't say outright that the app's
  USB serial survives, so this is the first thing to check on a spare board.
- **Screen redraws may slow down.** PSRAM is encrypted along with flash, and the framebuffer is in
  PSRAM. Measure it on a spare board with the `measure` feature.

## Check a board

This only reads the fuses, and it needs firmware from this branch:

```bash
qday device-lockdown /dev/cu.usbmodem1101
```

A new chip says `chip: open`. It reports the fuses the wallet depends on, not every fuse on the
chip, and the firmware that answers isn't checked by anything until secure boot is on.

## The steps

You need `esptool` 5 or later, which provides `esptool`, `espefuse` and `espsecure`, and ESP-IDF to
build the bootloader. Keep the keys out of this repository: `~/.qday-devnet/lockdown` works.

1. **Back up.** Write down your 24 words and check them with the device's backup check.

2. **Build the firmware and the bootloader.** The stock bootloader doesn't check the app's
   signature, so a locked chip needs this one.

   ```bash
   (cd firmware && cargo build --release)
   (cd tools/lockdown/bootloader && idf.py bootloader)
   espflash partition-table --to-binary -o partitions.bin firmware/partitions.csv
   ```

3. **Read the plan.** This prints every command and runs none of them.

   ```bash
   qday lockdown-plan --port /dev/cu.usbmodem1101 \
     --dir ~/.qday-devnet/lockdown \
     --elf firmware/target/xtensa-esp32s3-none-elf/release/qday-firmware \
     --bootloader tools/lockdown/bootloader/build/bootloader/bootloader.bin \
     --partition-table partitions.bin
   ```

4. **Prepare.** This makes the keys and images and doesn't touch a chip. It never replaces a key
   that already exists. Use `lockdown-prepare` with the same options, without `--port`.

5. **Rehearse.** This runs every fuse command against a simulated chip in a file, so a bad fuse name
   shows up here. Use `lockdown-rehearse` with `--fuses rehearsal.bin` in place of `--port`.

6. **Burn.** Use `lockdown-burn` with `--port` and `--mac`. It refuses any chip but that one, and
   asks you to type a phrase. `espefuse` then asks for `BURN` at every step. The order is fixed and
   tested in `crates/qday-cli/src/lockdown.rs`.

   If a step fails, stop. Don't run it again from the top, because the keys are already burned.
   Check `espefuse summary` and work out where it stopped.

7. **Decide about the flash encryption key.** Keep it offline to allow updates, or delete it to
   freeze the firmware. The burn deletes the seal key on its own. Keep the signing key offline.

8. **Set up again.** Restart the board, choose a passcode, wait for the keys, then restore from your
   words. Check the result with `qday device-lockdown`.

## What hasn't been tested

Everything on the host is tested: the seal, the keyed passcode, the setup order, the report and the
order of the burn. None of this has run on a chip:

- The HMAC peripheral with a real fuse key.
- The bootloader, which hasn't been built here because ESP-IDF isn't installed.
- `esptool elf2image` on this firmware's ELF. Only `espflash` has packaged it so far.
- The fuse commands, including the rehearsal, because `esptool` isn't installed here.
- Whether `burn-key` read-protects an `HMAC_UP` key by itself. Espressif's page contradicts itself
  on this, so confirm it in the rehearsal. In `espefuse summary` the key must not be readable.
- The app's USB serial port on a locked chip.

The commands come from Espressif's ESP32-S3 pages on
[security workflows](https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/security/security-features-enablement-workflows.html),
[flash encryption](https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/security/flash-encryption.html) and
[secure boot](https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/security/secure-boot-v2.html).
