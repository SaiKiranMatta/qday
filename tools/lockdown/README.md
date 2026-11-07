# Chip lockdown

**Prototype, and disabled. Don't run anything here unless you explicitly want to lock a board.**

Nothing here runs by itself, and `cargo run` and the normal firmware steps never use it. It has
never run on a chip, and burning fuses can't be undone.

**Never flash the bootloader in `bootloader/` by hand.** It's built with secure boot and flash
encryption on, so on an open chip it burns fuses by itself on first boot.

The steps, and what they do and don't protect, are in [`docs/lockdown.md`](../../docs/lockdown.md).
