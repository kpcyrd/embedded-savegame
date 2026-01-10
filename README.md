# embedded-savegame

[![Crates.io](https://img.shields.io/crates/v/embedded-savegame.svg)](https://crates.io/crates/embedded-savegame)
[![Documentation](https://docs.rs/embedded-savegame/badge.svg)](https://docs.rs/embedded-savegame)

A `no_std` savegame library for embedded systems with power-fail safety and wear leveling.

**⚠️ Work in progress**: The on-disk format may still change with no migration path between versions.

## Supported Flash Hardware

- **AT24Cxx EEPROM** (via `eeprom24x` feature)
- **W25Q NOR flash** (via `w25q` feature)
- **Custom hardware** (implement the `Flash` trait)

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
embedded-savegame = "0.2"

# Enable support for your flash hardware:
# embedded-savegame = { version = "0.2", features = ["eeprom24x"] }
# embedded-savegame = { version = "0.2", features = ["w25q"] }
```

## Usage Example

```rust
use embedded_savegame::storage::{Storage, Slot};

// Configure storage: 64-byte slots, 8 total slots
const SLOT_SIZE: usize = 64;
const SLOT_COUNT: usize = 8;

// Create storage manager with your flash device
let mut storage = Storage::<_, SLOT_SIZE, SLOT_COUNT>::new(flash_device);

// Scan for existing savegame
if let Ok(Some(slot)) = storage.scan() {
    let mut buf = [0u8; 256];
    if let Ok(Some(data)) = storage.read(slot.idx, &mut buf) {
        // Process loaded savegame
        process_game_state(data);
    }
}

// Write a new savegame
let mut game_data = serialize_game_state();
storage.append(&mut game_data)?;
```

## License

`MIT OR Apache-2.0`
