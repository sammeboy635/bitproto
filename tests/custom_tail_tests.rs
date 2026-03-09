use bitproto::BitPack;

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn unpack_u32s(buf: &[u8]) -> Vec<u32> {
    buf.chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

fn pack_u32s(v: &Vec<u32>) -> Vec<u8> {
    v.iter().flat_map(|n| n.to_le_bytes()).collect()
}

// ─── Structs ──────────────────────────────────────────────────────────────────

/// Fixed 8-byte header + variable Vec<u32> tail.
#[derive(BitPack, Debug, Default, PartialEq)]
#[bitpack(size = 8, endian = "le")]
struct Header {
    #[bitpack(byte = 0)] pub version: u8,
    #[bitpack(byte = 4)] pub count: u32,
    #[bitpack(unpack = "unpack_u32s", pack = "pack_u32s")]
    pub items: Vec<u32>,
}

/// Custom unpack only (read-only tail).
#[derive(BitPack, Debug, Default, PartialEq)]
#[bitpack(size = 2, endian = "le")]
struct ReadOnly {
    #[bitpack(byte = 0)] pub len: u16,
    #[bitpack(unpack = "unpack_u32s")]
    pub values: Vec<u32>,
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[test]
fn custom_tail_pack_produces_header_plus_items() {
    let msg = Header { version: 3, count: 2, items: vec![10, 20] };
    let buf = msg.pack();
    // 8 bytes header + 2×4 bytes items
    assert_eq!(buf.len(), 16);
    assert_eq!(buf[0], 3);                                          // version
    assert_eq!(u32::from_le_bytes(buf[4..8].try_into().unwrap()), 2); // count
    assert_eq!(u32::from_le_bytes(buf[8..12].try_into().unwrap()), 10);
    assert_eq!(u32::from_le_bytes(buf[12..16].try_into().unwrap()), 20);
}

#[test]
fn custom_tail_unpack_reads_items_from_tail() {
    let mut buf = vec![0u8; 16];
    buf[0] = 7;                                        // version
    buf[4..8].copy_from_slice(&5u32.to_le_bytes());    // count
    buf[8..12].copy_from_slice(&100u32.to_le_bytes());
    buf[12..16].copy_from_slice(&200u32.to_le_bytes());

    let msg = Header::unpack(&buf);
    assert_eq!(msg.version, 7);
    assert_eq!(msg.count, 5);
    assert_eq!(msg.items, vec![100, 200]);
}

#[test]
fn custom_tail_roundtrip() {
    let original = Header { version: 1, count: 3, items: vec![1, 2, 3] };
    assert_eq!(Header::unpack(&original.pack()), original);
}

#[test]
fn custom_tail_empty_items_roundtrip() {
    let original = Header { version: 0, count: 0, items: vec![] };
    let buf = original.pack();
    assert_eq!(buf.len(), 8); // header only
    assert_eq!(Header::unpack(&buf), original);
}

#[test]
fn custom_unpack_only_reads_tail() {
    let mut buf = vec![0u8; 10];
    buf[0..2].copy_from_slice(&3u16.to_le_bytes()); // len
    buf[2..6].copy_from_slice(&42u32.to_le_bytes());
    buf[6..10].copy_from_slice(&99u32.to_le_bytes());

    let msg = ReadOnly::unpack(&buf);
    assert_eq!(msg.len, 3);
    assert_eq!(msg.values, vec![42, 99]);
}

#[test]
fn custom_unpack_only_pack_leaves_tail_empty() {
    // custom_pack not provided — pack() only writes the fixed header
    let msg = ReadOnly { len: 5, values: vec![1, 2, 3] };
    let buf = msg.pack();
    assert_eq!(buf.len(), 2); // just the fixed header
}
