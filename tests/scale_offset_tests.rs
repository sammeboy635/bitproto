use bitproto::BitPack;

// ─────────────────────────────────────────────────────────────────────────────
// Test structs
// ─────────────────────────────────────────────────────────────────────────────

/// 8-bit bitfield with scale only.
/// Decode: field = raw * 0.5
/// Encode: wire = round(field / 0.5)
#[derive(BitPack, Debug, Default, PartialEq)]
#[bitpack(size = 2, endian = "le")]
struct BitfieldScale {
    #[bitpack(byte = 0, bits = "0..8", scale = 0.5)]
    pub value: f32,
    #[bitpack(byte = 1)]
    pub spare: u8,
}

/// 8-bit bitfield with offset only.
/// Decode: field = raw + 100
/// Encode: wire = field - 100
#[derive(BitPack, Debug, Default, PartialEq)]
#[bitpack(size = 2, endian = "le")]
struct BitfieldOffset {
    #[bitpack(byte = 0, bits = "0..8", offset = 100)]
    pub value: i32,
    #[bitpack(byte = 1)]
    pub spare: u8,
}

/// 8-bit bitfield with scale AND offset.
/// Decode: field = (raw + offset) * scale  →  (raw + 20) * 0.5
/// Encode: wire = round(field / 0.5) - 20
#[derive(BitPack, Debug, Default, PartialEq)]
#[bitpack(size = 2, endian = "le")]
struct BitfieldScaleAndOffset {
    #[bitpack(byte = 0, bits = "0..8", scale = 0.5, offset = 20)]
    pub value: f32,
    #[bitpack(byte = 1)]
    pub spare: u8,
}

/// Scalar u16 with scale only.
/// Decode: field = round(raw * 2.0) as u16
/// Encode: wire = round(field / 2.0) as u16
#[derive(BitPack, Debug, Default, PartialEq)]
#[bitpack(size = 2, endian = "le")]
struct ScalarScale {
    #[bitpack(byte = 0, scale = 2.0)]
    pub value: u16,
}

/// Scalar i16 with offset only.
/// Decode: field = (raw as i64 + 1000) as i16
/// Encode: wire = (field as i64 - 1000) as i16
#[derive(BitPack, Debug, Default, PartialEq)]
#[bitpack(size = 2, endian = "le")]
struct ScalarOffset {
    #[bitpack(byte = 0, offset = 1000)]
    pub value: i16,
}

/// Scalar i16 with scale AND offset.
/// Decode: field = round((raw as i64 + 500) as f64 * 0.5) as i16
/// Encode: wire = (round(field as f64 / 0.5) as i64 - 500) as i16
#[derive(BitPack, Debug, Default, PartialEq)]
#[bitpack(size = 2, endian = "le")]
struct ScalarScaleAndOffset {
    #[bitpack(byte = 0, scale = 0.5, offset = 500)]
    pub value: i16,
}

// ─────────────────────────────────────────────────────────────────────────────
// Bitfield scale tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn bitfield_scale_decode() {
    // wire byte 0 = 20 → raw = 20 → field = 20 * 0.5 = 10.0
    let buf = [20u8, 0];
    let s = BitfieldScale::decode(&buf);
    assert_eq!(s.value, 10.0f32);
}

#[test]
fn bitfield_scale_encode() {
    let s = BitfieldScale { value: 10.0, spare: 0 };
    let buf = s.encode();
    // field = 10.0 → wire = round(10.0 / 0.5) = 20
    assert_eq!(buf[0], 20);
}

#[test]
fn bitfield_scale_roundtrip() {
    for raw in [0u8, 1, 10, 100, 200, 255] {
        let buf = [raw, 0u8];
        let decoded = BitfieldScale::decode(&buf);
        let reencoded = decoded.encode();
        assert_eq!(reencoded[0], raw, "roundtrip failed for raw={raw}");
    }
}

#[test]
fn bitfield_scale_does_not_bleed_into_spare() {
    let s = BitfieldScale { value: 127.5, spare: 0 };
    let d = BitfieldScale::decode(&s.encode());
    assert_eq!(d.spare, 0);
}

#[test]
fn bitfield_scale_spare_does_not_bleed_into_value() {
    let s = BitfieldScale { value: 0.0, spare: 0xFF };
    let d = BitfieldScale::decode(&s.encode());
    assert_eq!(d.value, 0.0f32);
    assert_eq!(d.spare, 0xFF);
}

// ─────────────────────────────────────────────────────────────────────────────
// Bitfield offset tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn bitfield_offset_decode() {
    // wire byte 0 = 5 → raw = 5 → field = 5 + 100 = 105
    let buf = [5u8, 0];
    let s = BitfieldOffset::decode(&buf);
    assert_eq!(s.value, 105);
}

#[test]
fn bitfield_offset_encode() {
    let s = BitfieldOffset { value: 105, spare: 0 };
    let buf = s.encode();
    // field = 105 → wire = 105 - 100 = 5
    assert_eq!(buf[0], 5);
}

#[test]
fn bitfield_offset_roundtrip() {
    for raw in [0u8, 1, 50, 127, 200, 255] {
        let buf = [raw, 0u8];
        let decoded = BitfieldOffset::decode(&buf);
        let reencoded = decoded.encode();
        assert_eq!(reencoded[0], raw, "roundtrip failed for raw={raw}");
    }
}

#[test]
fn bitfield_offset_zero_wire_gives_offset_value() {
    let buf = [0u8, 0];
    let s = BitfieldOffset::decode(&buf);
    assert_eq!(s.value, 100); // 0 + 100
}

// ─────────────────────────────────────────────────────────────────────────────
// Bitfield scale + offset tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn bitfield_scale_offset_decode() {
    // wire = 6 → raw = 6 → (6 + 20) * 0.5 = 13.0
    let buf = [6u8, 0];
    let s = BitfieldScaleAndOffset::decode(&buf);
    assert_eq!(s.value, 13.0f32);
}

#[test]
fn bitfield_scale_offset_encode() {
    let s = BitfieldScaleAndOffset { value: 13.0, spare: 0 };
    let buf = s.encode();
    // wire = round(13.0 / 0.5) - 20 = 26 - 20 = 6
    assert_eq!(buf[0], 6);
}

#[test]
fn bitfield_scale_offset_roundtrip() {
    for raw in [0u8, 6, 50, 100, 200, 235] {
        let buf = [raw, 0u8];
        let decoded = BitfieldScaleAndOffset::decode(&buf);
        let reencoded = decoded.encode();
        assert_eq!(reencoded[0], raw, "roundtrip failed for raw={raw}");
    }
}

#[test]
fn bitfield_offset_only_affects_magnitude_not_wire_position() {
    // Offset shifts the value but doesn't move bits
    let s = BitfieldScaleAndOffset { value: 0.0 * 0.5 + 20.0 * 0.5, spare: 0 };
    let buf = s.encode();
    // value = 10.0 → wire = round(10.0/0.5) - 20 = 20 - 20 = 0
    assert_eq!(buf[0], 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// Scalar scale tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn scalar_scale_decode() {
    // wire u16 LE = 50 → field = round(50 * 2.0) as u16 = 100
    let buf = 50u16.to_le_bytes();
    let s = ScalarScale::decode(&buf);
    assert_eq!(s.value, 100);
}

#[test]
fn scalar_scale_encode() {
    let s = ScalarScale { value: 100 };
    let wire = u16::from_le_bytes(s.encode().try_into().unwrap());
    // field = 100 → wire = round(100 / 2.0) = 50
    assert_eq!(wire, 50);
}

#[test]
fn scalar_scale_roundtrip() {
    for raw in [0u16, 1, 50, 100, 500, 1000, u16::MAX / 2] {
        let buf = raw.to_le_bytes();
        let decoded = ScalarScale::decode(&buf);
        let reencoded = u16::from_le_bytes(decoded.encode().try_into().unwrap());
        assert_eq!(reencoded, raw, "roundtrip failed for raw={raw}");
    }
}

#[test]
fn scalar_scale_zero() {
    let buf = 0u16.to_le_bytes();
    let s = ScalarScale::decode(&buf);
    assert_eq!(s.value, 0);
    assert_eq!(u16::from_le_bytes(s.encode().try_into().unwrap()), 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// Scalar offset tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn scalar_offset_decode() {
    // wire i16 = -500 → field = (-500 + 1000) as i16 = 500
    let buf = (-500i16).to_le_bytes();
    let s = ScalarOffset::decode(&buf);
    assert_eq!(s.value, 500);
}

#[test]
fn scalar_offset_encode() {
    let s = ScalarOffset { value: 500 };
    let wire = i16::from_le_bytes(s.encode().try_into().unwrap());
    // field = 500 → wire = (500 - 1000) as i16 = -500
    assert_eq!(wire, -500);
}

#[test]
fn scalar_offset_roundtrip() {
    for raw in [-1000i16, -500, 0, 100, 500, 1000] {
        let buf = raw.to_le_bytes();
        let decoded = ScalarOffset::decode(&buf);
        let reencoded = i16::from_le_bytes(decoded.encode().try_into().unwrap());
        assert_eq!(reencoded, raw, "roundtrip failed for raw={raw}");
    }
}

#[test]
fn scalar_offset_zero_wire_gives_offset_value() {
    let buf = 0i16.to_le_bytes();
    let s = ScalarOffset::decode(&buf);
    assert_eq!(s.value, 1000); // 0 + 1000
}

// ─────────────────────────────────────────────────────────────────────────────
// Scalar scale + offset tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn scalar_scale_offset_decode() {
    // wire = 100 → (100 + 500) * 0.5 = 300
    let buf = 100i16.to_le_bytes();
    let s = ScalarScaleAndOffset::decode(&buf);
    assert_eq!(s.value, 300);
}

#[test]
fn scalar_scale_offset_encode() {
    let s = ScalarScaleAndOffset { value: 300 };
    let wire = i16::from_le_bytes(s.encode().try_into().unwrap());
    // round(300 / 0.5) - 500 = 600 - 500 = 100
    assert_eq!(wire, 100);
}

#[test]
fn scalar_scale_offset_roundtrip() {
    for raw in [-500i16, -100, 0, 100, 500, 1000] {
        let buf = raw.to_le_bytes();
        let decoded = ScalarScaleAndOffset::decode(&buf);
        let reencoded = i16::from_le_bytes(decoded.encode().try_into().unwrap());
        assert_eq!(reencoded, raw, "roundtrip failed for raw={raw}");
    }
}
