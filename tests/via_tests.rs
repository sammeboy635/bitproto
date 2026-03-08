use bitproto::{BitPack, FromRepr};

// ─── Test enum ────────────────────────────────────────────────────────────────

/// 2-bit sensor mode used as a via field.
#[derive(FromRepr, Debug, Clone, Copy, PartialEq, Default)]
#[repr(u8)]
enum SensorMode {
    #[default]
    Off    = 0,
    Idle   = 1,
    Active = 2,
    Error  = 3,
}

/// Enum with non-contiguous discriminants to test gap handling.
#[derive(FromRepr, Debug, Clone, Copy, PartialEq, Default)]
#[repr(u8)]
enum Priority {
    #[default]
    Low    = 0,
    Medium = 5,
    High   = 10,
}

// ─── Test struct ──────────────────────────────────────────────────────────────

/// 1-byte config: mode in bits 0-1, gain in bits 2-7.
#[derive(BitPack, Debug, Default, PartialEq)]
#[bitpack(size = 1, endian = "le")]
struct SensorConfig {
    #[bitpack(byte = 0, bits = "0..2", via = "u8")]
    pub mode: SensorMode,
    #[bitpack(byte = 0, bits = "2..8")]
    pub gain: u8,
}

/// 2-byte packet: priority in bits 0-7 of byte 0 (8-bit via field), value in byte 1.
#[derive(BitPack, Debug, Default, PartialEq)]
#[bitpack(size = 2, endian = "le")]
struct PriorityPacket {
    #[bitpack(byte = 0, bits = "0..8", via = "u8")]
    pub priority: Priority,
    #[bitpack(byte = 1)]
    pub value: u8,
}

// ─── FromRepr tests ───────────────────────────────────────────────────────────

#[test]
fn from_repr_known_variants() {
    assert_eq!(SensorMode::from(0u8), SensorMode::Off);
    assert_eq!(SensorMode::from(1u8), SensorMode::Idle);
    assert_eq!(SensorMode::from(2u8), SensorMode::Active);
    assert_eq!(SensorMode::from(3u8), SensorMode::Error);
}

#[test]
fn from_repr_unknown_falls_back_to_default() {
    assert_eq!(SensorMode::from(4u8),   SensorMode::Off);
    assert_eq!(SensorMode::from(255u8), SensorMode::Off);
}

#[test]
fn from_repr_non_contiguous_discriminants() {
    assert_eq!(Priority::from(0u8),   Priority::Low);
    assert_eq!(Priority::from(5u8),   Priority::Medium);
    assert_eq!(Priority::from(10u8),  Priority::High);
    // gaps fall back to default
    assert_eq!(Priority::from(1u8),   Priority::Low);
    assert_eq!(Priority::from(6u8),   Priority::Low);
    assert_eq!(Priority::from(255u8), Priority::Low);
}

// ─── via + BitPack integration tests ─────────────────────────────────────────

#[test]
fn via_roundtrip_all_variants() {
    for mode in [SensorMode::Off, SensorMode::Idle, SensorMode::Active, SensorMode::Error] {
        let s = SensorConfig { mode, gain: 0 };
        assert_eq!(SensorConfig::decode(&s.encode()).mode, mode, "{mode:?} failed roundtrip");
    }
}

#[test]
fn via_wire_value_matches_discriminant() {
    for (mode, disc) in [
        (SensorMode::Off,    0u8),
        (SensorMode::Idle,   1),
        (SensorMode::Active, 2),
        (SensorMode::Error,  3),
    ] {
        let wire = SensorConfig { mode, gain: 0 }.encode()[0] & 0x03;
        assert_eq!(wire, disc, "{mode:?}: expected disc {disc}, got {wire}");
    }
}

#[test]
fn via_does_not_bleed_into_adjacent_field() {
    // mode=Error(3) should not corrupt gain
    let s = SensorConfig { mode: SensorMode::Error, gain: 0b111111 };
    let d = SensorConfig::decode(&s.encode());
    assert_eq!(d.mode, SensorMode::Error);
    assert_eq!(d.gain, 0b111111);
}

#[test]
fn via_adjacent_field_does_not_bleed_into_mode() {
    let s = SensorConfig { mode: SensorMode::Off, gain: 0b111111 };
    let d = SensorConfig::decode(&s.encode());
    assert_eq!(d.mode, SensorMode::Off);
    assert_eq!(d.gain, 0b111111);
}

#[test]
fn via_exact_wire_layout() {
    // mode=Active(2)=0b10, gain=0b101010 → byte = 0b10_10_10_10 = 0xAA
    let s = SensorConfig { mode: SensorMode::Active, gain: 0b101010 };
    assert_eq!(s.encode()[0], 0b1010_1010);
}

#[test]
fn via_non_contiguous_roundtrip() {
    for priority in [Priority::Low, Priority::Medium, Priority::High] {
        let s = PriorityPacket { priority, value: 0xBE };
        let d = PriorityPacket::decode(&s.encode());
        assert_eq!(d.priority, priority, "{priority:?} failed roundtrip");
        assert_eq!(d.value, 0xBE);
    }
}
