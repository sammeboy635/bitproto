# bitproto

A Rust procedural macro library for binary protocol encoding and decoding. Derive `encode`/`decode` methods on structs with bit-level field control — endianness, bitfields, scaling, offsets, two's complement, and enum mapping.

Designed for parsing wire protocols like UBX (u-blox GNSS), but general enough for any fixed-size binary format.

## Features

- Bit-level field extraction (LSB-first and MSB-first)
- Single-bit boolean/integer fields
- Per-field or struct-wide endianness (little-endian / big-endian)
- Linear scaling (`scale`) and offset (`offset`) transformations
- Two's complement sign extension for bitfields (`twos_comp`)
- Raw byte array fields (`raw`)
- Enum mapping via intermediate types (`via`)
- Forced word size for bitfield containers (`word`)
- Skipped fields (`skip`)

## Usage

Add to `Cargo.toml`:

```toml
[dependencies]
bitproto = { path = "..." }
```

### Basic struct

```rust
use bitproto::BitPack;

#[derive(BitPack, Debug, Default, PartialEq)]
#[bitpack(size = 4, endian = "le")]
struct MyMessage {
    #[bitpack(byte = 0)]
    pub sequence: u8,

    #[bitpack(byte = 1)]
    pub flags: u16,

    #[bitpack(byte = 3)]
    pub checksum: u8,
}

let msg = MyMessage { sequence: 1, flags: 0x0102, checksum: 0xFF };
let bytes = msg.encode();
let decoded = MyMessage::decode(&bytes);
assert_eq!(msg, decoded);
```

### Bitfields

Multiple fields can share the same byte offset. The macro groups them into the smallest word type that spans all used bits.

```rust
#[derive(BitPack, Debug, Default, PartialEq)]
#[bitpack(size = 1)]
struct StatusByte {
    /// Bits 0-3: 4-bit version field
    #[bitpack(byte = 0, bits = "0..4")]
    pub version: u8,

    /// Bit 4: single boolean flag
    #[bitpack(byte = 0, bit = 4)]
    pub active: bool,

    /// Bits 5-7: 3-bit error code
    #[bitpack(byte = 0, bits = "5..8")]
    pub error: u8,
}
```

Use `word = "u32"` to force a 4-byte container even if fewer bits are used — useful when the protocol spec requires it:

```rust
#[bitpack(byte = 0, bits = "0..8", word = "u32")]
pub version: u8,
```

### Reversed bit order (MSB-first)

Swap `lo` and `hi` in `bits` to store bits MSB-first on the wire:

```rust
/// 4-bit ID stored bit-reversed (MSB-first) in bits 0-3
#[bitpack(byte = 0, bits = "3..0")]
pub sensor_id: u8,
```

### Scale and offset

Apply linear transformations during encode/decode. Decode: `(raw + offset) * scale`. Encode: `round(field / scale) - offset`.

```rust
/// Stored as integer LSBs of 0.01 m/s²; read back as float m/s²
#[bitpack(byte = 0, bits = "0..16", scale = 0.01)]
pub acceleration: f32,

/// Temperature stored with +40 offset to avoid negative wire values
#[bitpack(byte = 2, offset = 40)]
pub temperature: i8,
```

### Two's complement sign extension

For signed bitfields narrower than the field type:

```rust
/// 12-bit signed value in bits 0-12
#[bitpack(byte = 0, bits = "0..12", twos_comp)]
pub delta: i32,
```

### Raw byte arrays

Copy bytes verbatim without any transformation:

```rust
/// Reserved 4-byte region, preserved across encode/decode
#[bitpack(byte = 4, raw = 4)]
pub reserved: [u8; 4],
```

### Skipped fields

Fields with `skip` are ignored during encode/decode and left at their default value:

```rust
#[bitpack(skip)]
pub computed: u32,
```

### Enums with `FromRepr`

Derive `From<ReprType>` for enums. Unknown values fall back to the `#[default]` variant (or the first variant).

```rust
use bitproto::FromRepr;

#[derive(FromRepr, Debug, Clone, Copy, PartialEq, Default)]
#[repr(u8)]
enum SensorMode {
    #[default]
    Off    = 0,
    Idle   = 1,
    Active = 2,
    Error  = 3,
}

assert_eq!(SensorMode::from(2u8), SensorMode::Active);
assert_eq!(SensorMode::from(99u8), SensorMode::Off); // unknown → default
```

Use `via` to decode enum fields from a bitfield:

```rust
#[bitpack(byte = 0, bits = "0..2", via = "u8")]
pub mode: SensorMode,
```

## Real-world example: UBX-ESF-INS

A 36-byte u-blox IMU packet with bitfield flags and signed scalar fields:

```rust
use bitproto::BitPack;

#[derive(BitPack, Debug, Clone, Default, PartialEq)]
#[bitpack(size = 36, endian = "le")]
pub struct UbxEsfIns {
    #[bitpack(byte = 0, bits = "0..8", word = "u32")]
    pub version: u8,

    #[bitpack(byte = 0, bit = 8)]
    pub x_ang_rate_valid: bool,

    #[bitpack(byte = 0, bit = 9)]
    pub y_ang_rate_valid: bool,

    #[bitpack(byte = 0, bit = 10)]
    pub z_ang_rate_valid: bool,

    #[bitpack(byte = 4, raw = 4)]
    pub reserved0: [u8; 4],

    #[bitpack(byte = 8)]
    pub i_tow: u32,

    #[bitpack(byte = 12)]
    pub x_ang_rate: i32, // units: 1e-3 deg/s

    #[bitpack(byte = 16)]
    pub y_ang_rate: i32,

    #[bitpack(byte = 20)]
    pub z_ang_rate: i32,

    #[bitpack(byte = 24)]
    pub x_accel: i32, // units: 1e-2 m/s², gravity-free

    #[bitpack(byte = 28)]
    pub y_accel: i32,

    #[bitpack(byte = 32)]
    pub z_accel: i32,
}

let msg = UbxEsfIns {
    version: 1,
    x_ang_rate_valid: true,
    i_tow: 360_000,
    x_ang_rate: 1_500,
    ..Default::default()
};
let bytes = msg.encode();
assert_eq!(bytes.len(), 36);
assert_eq!(UbxEsfIns::decode(&bytes), msg);
```

## Attribute reference

### Struct-level `#[bitpack(...)]`

| Attribute | Required | Description |
|-----------|----------|-------------|
| `size = N` | Yes | Total encoded size in bytes |
| `endian = "le"\|"be"` | No | Default byte order (default: `"le"`) |

### Field-level `#[bitpack(...)]`

| Attribute | Description |
|-----------|-------------|
| `skip` | Exclude field from encode/decode |
| `byte = N` | Starting byte offset in the buffer |
| `bits = "lo..hi"` | Bitfield: extract bits `lo` to `hi` (LSB-first if `lo < hi`, MSB-first/reversed if `lo > hi`) |
| `bit = N` | Single bit at position N |
| `raw = N` | Raw byte array of N bytes, copied verbatim |
| `word = "u8"\|"u16"\|"u32"\|"u64"` | Force bitfield container word size |
| `endian = "le"\|"be"` | Override endianness for this scalar field |
| `scale = F` | Multiply by F on decode, divide by F on encode |
| `offset = N` | Add N on decode, subtract N on encode |
| `twos_comp` | Sign-extend bitfield using two's complement |
| `via = "TYPE"` | Intermediate type for enum conversion (bitfields only) |

### Transformation order

- **Decode**: `(raw + offset) * scale`
- **Encode**: `round(field / scale) - offset`

## Generated API

`#[derive(BitPack)]` generates:

```rust
impl MyStruct {
    pub fn encode(&self) -> Vec<u8>;
    pub fn decode(buf: &[u8]) -> Self;  // requires Default
}
```

`decode` panics if `buf.len() < size`. Longer buffers are accepted.

## Dependencies

- [syn](https://crates.io/crates/syn) — parse Rust token streams
- [quote](https://crates.io/crates/quote) — generate Rust code
- [proc-macro2](https://crates.io/crates/proc-macro2) — proc-macro bridge
