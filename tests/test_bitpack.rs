//! `UBX-ESF-INS` (class 0x10, id 0x15): Vehicle dynamics information.
//!
//! Provides gravity-free compensated angular rate and acceleration in the
//! vehicle frame.  Wire format is 36 bytes, all fields little-endian.
//!
//! ## Wire layout
//!
//! ```text
//! Bytes  0- 3  u32  bitfield (LE)
//!   bits  7..0   version   (0x01)
//!   bit   8      x_ang_rate_valid
//!   bit   9      y_ang_rate_valid
//!   bit  10      z_ang_rate_valid
//!   bit  11      x_accel_valid
//!   bit  12      y_accel_valid
//!   bit  13      z_accel_valid
//! Bytes  4- 7  reserved (preserved round-trip)
//! Bytes  8-11  u32 i_tow         GPS ToW (ms)
//! Bytes 12-15  i32 x_ang_rate    (1e-3 deg/s)
//! Bytes 16-19  i32 y_ang_rate
//! Bytes 20-23  i32 z_ang_rate
//! Bytes 24-27  i32 x_accel       (1e-2 m/s², gravity-free)
//! Bytes 28-31  i32 y_accel
//! Bytes 32-35  i32 z_accel
//! ```

use bitproto::BitPack;

#[derive(BitPack, Debug, Clone, Default, PartialEq)]
#[bitpack(size = 36, endian = "le")]
pub struct UbxEsfIns {
    // ── bytes 0-3: bitfield word (LE u32) ─────────────────────────────────────
    // `word = "u32"` forces the container to 4 bytes even though only bits
    // 0-13 are used — matching the UBX protocol specification.

    /// Message version (should be 0x01).
    #[bitpack(byte = 0, bits = "0..8", word = "u32")]
    pub version: u8,

    /// X-axis angular rate valid flag.
    #[bitpack(byte = 0, bit = 8)]
    pub x_ang_rate_valid: bool,

    /// Y-axis angular rate valid flag.
    #[bitpack(byte = 0, bit = 9)]
    pub y_ang_rate_valid: bool,

    /// Z-axis angular rate valid flag.
    #[bitpack(byte = 0, bit = 10)]
    pub z_ang_rate_valid: bool,

    /// X-axis acceleration valid flag.
    #[bitpack(byte = 0, bit = 11)]
    pub x_accel_valid: bool,

    /// Y-axis acceleration valid flag.
    #[bitpack(byte = 0, bit = 12)]
    pub y_accel_valid: bool,

    /// Z-axis acceleration valid flag.
    #[bitpack(byte = 0, bit = 13)]
    pub z_accel_valid: bool,

    // ── bytes 4-7: reserved ───────────────────────────────────────────────────
    #[allow(dead_code)]
    #[bitpack(byte = 4, raw = 4)]
    pub reserved0: [u8; 4],

    // ── bytes 8-35: scalars ───────────────────────────────────────────────────
    /// GPS time of week (ms).
    #[bitpack(byte = 8)]
    pub i_tow: u32,

    /// Compensated x-axis angular rate (1e-3 deg/s).
    #[bitpack(byte = 12)]
    pub x_ang_rate: i32,

    /// Compensated y-axis angular rate (1e-3 deg/s).
    #[bitpack(byte = 16)]
    pub y_ang_rate: i32,

    /// Compensated z-axis angular rate (1e-3 deg/s).
    #[bitpack(byte = 20)]
    pub z_ang_rate: i32,

    /// Compensated x-axis acceleration (1e-2 m/s², gravity-free).
    #[bitpack(byte = 24)]
    pub x_accel: i32,

    /// Compensated y-axis acceleration (1e-2 m/s², gravity-free).
    #[bitpack(byte = 28)]
    pub y_accel: i32,

    /// Compensated z-axis acceleration (1e-2 m/s², gravity-free).
    #[bitpack(byte = 32)]
    pub z_accel: i32,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── fixtures ─────────────────────────────────────────────────────────────

    fn sample() -> UbxEsfIns {
        UbxEsfIns {
            version:          0x01,
            x_ang_rate_valid: true,
            y_ang_rate_valid: false,
            z_ang_rate_valid: true,
            x_accel_valid:    false,
            y_accel_valid:    true,
            z_accel_valid:    false,
            reserved0:        [0xDE, 0xAD, 0xBE, 0xEF],
            i_tow:            360_000,
            x_ang_rate:        1_500,
            y_ang_rate:         -750,
            z_ang_rate:          200,
            x_accel:             980,
            y_accel:             -50,
            z_accel:               0,
        }
    }

    // ── payload length ────────────────────────────────────────────────────────

    #[test]
    fn encode_is_36_bytes() {
        assert_eq!(sample().encode().len(), 36);
    }

    // ── bitfield word: byte order and bit positions ───────────────────────────

    #[test]
    fn version_lives_in_byte_0() {
        let s = UbxEsfIns { version: 0x01, ..Default::default() };
        let buf = s.encode();
        // bits 0-7 of the LE u32 at byte 0 → byte 0 of the buffer
        assert_eq!(buf[0], 0x01);
        assert_eq!(buf[1], 0x00); // no flags set
    }

    #[test]
    fn flags_at_correct_bit_positions() {
        let s = UbxEsfIns {
            version:          0x00,
            x_ang_rate_valid: true,   // bit 8
            y_ang_rate_valid: false,  // bit 9
            z_ang_rate_valid: true,   // bit 10
            x_accel_valid:    false,  // bit 11
            y_accel_valid:    true,   // bit 12
            z_accel_valid:    false,  // bit 13
            ..Default::default()
        };
        let buf = s.encode();
        let word = u32::from_le_bytes(buf[0..4].try_into().unwrap());

        assert_eq!((word >>  8) & 1, 1, "x_ang_rate_valid @ bit 8");
        assert_eq!((word >>  9) & 1, 0, "y_ang_rate_valid @ bit 9");
        assert_eq!((word >> 10) & 1, 1, "z_ang_rate_valid @ bit 10");
        assert_eq!((word >> 11) & 1, 0, "x_accel_valid    @ bit 11");
        assert_eq!((word >> 12) & 1, 1, "y_accel_valid    @ bit 12");
        assert_eq!((word >> 13) & 1, 0, "z_accel_valid    @ bit 13");
    }

    #[test]
    fn bitfield_word_uses_u32_container() {
        // The word attribute forces 4 bytes even though bits 0-13 could fit u16.
        // Bytes 2-3 of the LE word should be zero (no fields mapped there).
        let s = UbxEsfIns { version: 0x01, x_ang_rate_valid: true, ..Default::default() };
        let buf = s.encode();
        assert_eq!(buf[2], 0x00, "byte 2 of the u32 bitfield should be zero");
        assert_eq!(buf[3], 0x00, "byte 3 of the u32 bitfield should be zero");
    }

    #[test]
    fn version_max_value() {
        let s = UbxEsfIns { version: 0xFF, ..Default::default() };
        assert_eq!(UbxEsfIns::decode(&s.encode()).version, 0xFF);
    }

    #[test]
    fn version_does_not_bleed_into_flags() {
        // version is bits 0-7; flags start at bit 8 — they must be independent.
        let s = UbxEsfIns { version: 0xFF, x_ang_rate_valid: false, ..Default::default() };
        let buf = s.encode();
        let word = u32::from_le_bytes(buf[0..4].try_into().unwrap());
        assert_eq!((word >> 8) & 1, 0, "flag bit 8 should be clear");
    }

    // ── reserved bytes ────────────────────────────────────────────────────────

    #[test]
    fn reserved0_copied_verbatim() {
        let s = sample();
        let buf = s.encode();
        assert_eq!(&buf[4..8], &s.reserved0);
    }

    #[test]
    fn reserved0_roundtrip() {
        let s = sample();
        assert_eq!(UbxEsfIns::decode(&s.encode()).reserved0, s.reserved0);
    }

    // ── scalar field byte order ───────────────────────────────────────────────

    #[test]
    fn i_tow_little_endian_layout() {
        let s = UbxEsfIns { i_tow: 0x1234_5678, ..Default::default() };
        let buf = s.encode();
        assert_eq!(&buf[8..12], &[0x78u8, 0x56, 0x34, 0x12], "LE: LSB first");
    }

    #[test]
    fn i_tow_roundtrip() {
        let s = sample();
        assert_eq!(UbxEsfIns::decode(&s.encode()).i_tow, s.i_tow);
    }

    // ── signed scalar fields ─────────────────────────────────────────────────

    #[test]
    fn negative_x_ang_rate_roundtrip() {
        let s = UbxEsfIns { x_ang_rate: -999_999, ..Default::default() };
        assert_eq!(UbxEsfIns::decode(&s.encode()).x_ang_rate, -999_999);
    }

    #[test]
    fn i32_min_roundtrip() {
        let s = UbxEsfIns { z_accel: i32::MIN, ..Default::default() };
        assert_eq!(UbxEsfIns::decode(&s.encode()).z_accel, i32::MIN);
    }

    #[test]
    fn i32_max_roundtrip() {
        let s = UbxEsfIns { y_accel: i32::MAX, ..Default::default() };
        assert_eq!(UbxEsfIns::decode(&s.encode()).y_accel, i32::MAX);
    }

    // ── full roundtrips ───────────────────────────────────────────────────────

    #[test]
    fn full_roundtrip() {
        let s = sample();
        assert_eq!(UbxEsfIns::decode(&s.encode()), s);
    }

    #[test]
    fn all_zeros_default_roundtrip() {
        let s = UbxEsfIns::default();
        let buf = s.encode();
        assert!(buf.iter().all(|&b| b == 0), "default should be all-zero bytes");
        assert_eq!(UbxEsfIns::decode(&buf), s);
    }

    #[test]
    fn all_flags_set_roundtrip() {
        let s = UbxEsfIns {
            version:          0xFF,
            x_ang_rate_valid: true,
            y_ang_rate_valid: true,
            z_ang_rate_valid: true,
            x_accel_valid:    true,
            y_accel_valid:    true,
            z_accel_valid:    true,
            ..Default::default()
        };
        assert_eq!(UbxEsfIns::decode(&s.encode()), s);
    }

    #[test]
    fn max_values_roundtrip() {
        let s = UbxEsfIns {
            version:    0xFF,
            i_tow:      u32::MAX,
            x_ang_rate: i32::MAX,
            y_ang_rate: i32::MIN,
            z_ang_rate: i32::MAX,
            x_accel:    i32::MIN,
            y_accel:    i32::MAX,
            z_accel:    i32::MIN,
            ..Default::default()
        };
        assert_eq!(UbxEsfIns::decode(&s.encode()), s);
    }

    // ── error conditions ──────────────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "buffer too short")]
    fn decode_short_buffer_panics() {
        UbxEsfIns::decode(&[0u8; 10]);
    }

    /// decode must accept buffers *longer* than 36 bytes.
    #[test]
    fn decode_accepts_longer_buffer() {
        let _ = UbxEsfIns::decode(&vec![0u8; 100]);
    }

    // ── independence of fields ────────────────────────────────────────────────

    #[test]
    fn each_flag_independent() {
        let flags: &[(&str, fn(&mut UbxEsfIns))] = &[
            ("x_ang_rate_valid", |s| s.x_ang_rate_valid = true),
            ("y_ang_rate_valid", |s| s.y_ang_rate_valid = true),
            ("z_ang_rate_valid", |s| s.z_ang_rate_valid = true),
            ("x_accel_valid",    |s| s.x_accel_valid    = true),
            ("y_accel_valid",    |s| s.y_accel_valid     = true),
            ("z_accel_valid",    |s| s.z_accel_valid     = true),
        ];

        for (name, setter) in flags {
            let mut s = UbxEsfIns::default();
            setter(&mut s);
            let d = UbxEsfIns::decode(&s.encode());
            // The flag we set should be true
            let flag_val = match *name {
                "x_ang_rate_valid" => d.x_ang_rate_valid,
                "y_ang_rate_valid" => d.y_ang_rate_valid,
                "z_ang_rate_valid" => d.z_ang_rate_valid,
                "x_accel_valid"    => d.x_accel_valid,
                "y_accel_valid"    => d.y_accel_valid,
                "z_accel_valid"    => d.z_accel_valid,
                _ => unreachable!(),
            };
            assert!(flag_val, "flag {name} should survive encode/decode");
            // All others should still be false
            let others_clear =
                (d.x_ang_rate_valid as u8
                + d.y_ang_rate_valid as u8
                + d.z_ang_rate_valid as u8
                + d.x_accel_valid    as u8
                + d.y_accel_valid    as u8
                + d.z_accel_valid    as u8) == 1;
            assert!(others_clear, "only {name} should be set");
        }
    }

    #[test]
    fn scalars_do_not_bleed_into_each_other() {
        // Set one scalar at a time, all others should remain zero after roundtrip.
        let s = UbxEsfIns { x_ang_rate: 42, ..Default::default() };
        let d = UbxEsfIns::decode(&s.encode());
        assert_eq!(d.x_ang_rate, 42);
        assert_eq!(d.y_ang_rate, 0);
        assert_eq!(d.z_ang_rate, 0);
        assert_eq!(d.x_accel,    0);
        assert_eq!(d.y_accel,    0);
        assert_eq!(d.z_accel,    0);
        assert_eq!(d.i_tow,      0);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Demonstrate reversed-bit storage on a different (hypothetical) struct
// ─────────────────────────────────────────────────────────────────────────────

/// A hypothetical IMU register that stores a 4-bit sensor ID in bits 3..0
/// using MSB-first (reversed) bit order, followed by a 4-bit status in bits 4..7,
/// all in byte 0. Byte 1 holds an independent threshold scalar.
///
/// This exercises the `bits = "HI..LO"` reversed syntax alongside a normal field
/// in the same byte group, with no overlap with the byte-1 scalar.
#[derive(BitPack, Debug, Default, PartialEq)]
#[bitpack(size = 2)]
pub struct ReversedExample {
    /// 4-bit sensor ID stored MSB-first (bit-reversed) in the low nibble of byte 0.
    /// `bits = "3..0"` → lo=3, hi=0, len=4, positions 0-3, reversed.
    #[bitpack(byte = 0, bits = "3..0")]
    pub sensor_id: u8,

    /// 4-bit status in the upper nibble of byte 0, normal order.
    #[bitpack(byte = 0, bits = "4..8")]
    pub status: u8,

    /// Threshold value in byte 1.
    #[bitpack(byte = 1)]
    pub threshold: u8,
}

#[cfg(test)]
mod reversed_tests {
    use super::*;

    #[test]
    fn reversed_bits_roundtrip() {
        let s = ReversedExample { sensor_id: 0b1010, status: 0xA, threshold: 0x42 };
        let d = ReversedExample::decode(&s.encode());
        assert_eq!(d, s);
    }

    #[test]
    fn reversed_bits_are_actually_reversed_on_wire() {
        // sensor_id = 0b1010 (4 bits), reversed = 0b0101
        // stored in bits [0..3] of the word → wire byte 0 low nibble
        let s = ReversedExample { sensor_id: 0b1010, status: 0, threshold: 0 };
        let buf = s.encode();
        let wire_id = buf[0] & 0b0000_1111; // low 4 bits
        let expected_reversed = (0b1010_u8).reverse_bits() >> 4; // reverse 8 bits, shift to 4-bit
        assert_eq!(wire_id, expected_reversed, "sensor_id should be bit-reversed on wire");
    }

    #[test]
    fn normal_and_reversed_fields_independent() {
        let s = ReversedExample { sensor_id: 0, status: 0xF, threshold: 0 };
        let d = ReversedExample::decode(&s.encode());
        assert_eq!(d.status, 0xF);
        assert_eq!(d.sensor_id, 0);
    }
}