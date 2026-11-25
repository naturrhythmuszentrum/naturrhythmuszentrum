// SPDX-License-Identifier: AGPL-3.0-only
//! Literal canonical records and source tariffs, independent of producer Work counters.

/// Canonical syntax length, record count, payload bytes, reference slots, rational encoding Work.
const SYNTAX: [[u64; 5]; 21] = [
    [374, 5, 299, 5, 0],
    [371, 4, 307, 4, 0],
    [371, 4, 307, 4, 0],
    [375, 4, 311, 4, 0],
    [371, 4, 307, 4, 0],
    [371, 4, 307, 4, 0],
    [686, 11, 545, 23, 64],
    [464, 6, 378, 9, 32],
    [467, 6, 381, 9, 32],
    [686, 11, 545, 23, 64],
    [685, 11, 544, 23, 64],
    [377, 4, 313, 4, 0],
    [377, 4, 313, 4, 0],
    [377, 4, 313, 4, 0],
    [377, 4, 313, 4, 0],
    [377, 4, 313, 4, 0],
    [377, 4, 313, 4, 0],
    [462, 6, 376, 10, 0],
    [560, 8, 452, 16, 32],
    [657, 10, 527, 22, 64],
    [654, 10, 524, 24, 32],
];
/// `validation::encode_draft`: complete byte lengths in the displayed forty-draft order.
const DRAFT: [u64; 40] = [
    217, 15, 23, 28, 28, 32, 28, 34, 34, 34, 34, 34, 34, 34, 34, 34, 33, 34, 41, 32, 41, 32, 41,
    41, 41, 41, 32, 41, 32, 41, 41, 32, 41, 32, 41, 41, 42, 41, 41, 41,
];
/// `vertex_key`: the duplicate draft results retain the earlier identical keys.
const KEY: [u64; 40] = [
    244, 17, 249, 46, 46, 50, 46, 52, 52, 52, 52, 52, 52, 52, 52, 52, 51, 52, 59, 41, 59, 41, 50,
    50, 50, 59, 41, 59, 41, 50, 59, 41, 59, 41, 50, 59, 60, 59, 59, 59,
];
/// Complete graph dependencies include registered Real ambients and both header scalar spaces.
const EDGES: [u64; 40] = [
    2, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 4, 2, 4, 2, 3, 3, 3, 4, 2, 4, 2, 3, 4, 2,
    4, 2, 3, 4, 4, 4, 4, 4,
];

/// Exact complete Work includes all literal bindings events and the registered CAS route.
pub(super) fn work(replay: bool) -> u64 {
    // number.rs: four integer parses at 225, half parse at 291, three formats at 64.
    let rational_pool = 4 * 225 + 291 + 3 * 64;
    // Seven exact matches and three zero/half comparisons use seven one-limb tariffs;
    // twelve remaining unequal-quotient comparisons use five tariffs, each 32 Work.
    let rational_match = (7 * 7 + 3 * 7 + 12 * 5) * 32;
    let syntax = SYNTAX
        .iter()
        .map(|&[key, count, payload, references, arithmetic]| {
            assert_eq!(key, 20 + 11 * count + payload);
            // describe: payload + arithmetic; footprint: 16*payload+256*references;
            // ready: 2*payload+11 per record; Kahn visits: triangular count; envelope: key.
            19 * payload
                + 256 * references
                + 11 * count
                + arithmetic
                + count * (count + 1) / 2
                + key
        })
        .sum::<u64>();
    let scope = 108 + SYNTAX.iter().map(|row| row[0]).sum::<u64>();
    let wire = scope + 195;
    assert_eq!((syntax, scope, wire), (217_160, 9_924, 10_119));
    let registered = if replay {
        // Reader reads Q bytes, allocates twice the 28-byte name and ten 24-byte handles.
        345 + (wire + 296) + syntax + scope + 2 * wire + (wire + 1167) + (wire + 3) + 256 + 5
    } else {
        509 + syntax + scope + wire + (wire + 1191) + (wire + 3) + 256 + 5
    };
    let total = 821 + rational_pool + rational_match + admission() + registered;
    assert_eq!(total, if replay { 343_917 } else { 323_571 });
    total
}

/// Admission is evaluated from record sizes, arities and the existing header/name policy.
fn admission() -> u64 {
    let encoded = DRAFT.iter().sum::<u64>();
    let pending = 3 * encoded + 40 * 256;
    assert_eq!((encoded, pending), (1593, 15019));
    // Field, two environment encodings, declaration sorting, uniqueness, sorts, frozen encoding.
    let header = 1 + 2 * 216 + 10 * (4 * 257 + 12) + 10 * 256 + 10 * 2 + 216;
    // Two scalar spaces; header; region probe, new-region visit and insertion; root context.
    let insert = (2 * 20 + 1)
        + (2 * 23 + 1)
        + (2 * 244 + 1)
        + (2 * (2 * 17 + 1) + 1)
        + (1 + 4 + 2 * 249 + 1);
    let expressions = (3..40)
        .map(|index| {
            let boolean = matches!(index, 19 | 21 | 22 | 23 | 24 | 26 | 28 | 29 | 31 | 33 | 34);
            let arity = EDGES[index] - if boolean { 1 } else { 2 };
            let comparison = if boolean { 7 } else { 8 };
            let sort = if boolean { 1 } else { 2 };
            let arithmetic = if (13..18).contains(&index) { 32 } else { 0 };
            let free = if index < 13 { 5 * 256 + 6 } else { 0 };
            let copies = if matches!(index, 25 | 26) { 1 } else { 2 };
            3 * arity
                + sort
                + comparison
                + 8 * comparison * (arity + 1)
                + free
                + copies * (2 * KEY[index] + 1 + arithmetic)
        })
        .sum::<u64>();
    assert_eq!((header, insert, expressions), (13629, 1152, 25718));
    let local_map = 70 + 80 + 5 + 80;
    let total = encoded + 5 * 32 + pending + 256 + 40 + header + insert + expressions + local_map;
    assert_eq!(total, 57802);
    total
}

/// Forty actual records: two scalar spaces and thirty-eight distinct explicit draft records.
pub(super) fn graph() -> u64 {
    let explicit = (0..40)
        .filter(|index| !matches!(index, 25 | 26))
        .map(|index| 3 * KEY[index] + 256 + 32 * EDGES[index])
        .sum::<u64>();
    let total = explicit + (3 * 20 + 256) + (3 * 23 + 256);
    assert_eq!(total, 20514);
    total
}

/// New retained ownership excludes the caller's existing physical original union.
pub(super) fn retained(replay: bool) -> u64 {
    48 + size_of::<nrz_cas::Cas>() as u64
        + 384
        + 16
        + graph()
        + 10119
        + if replay { 1146 } else { 1170 }
}

/// `syntax_key` gathers one DAG while the earlier canonical blobs remain live.
fn registered_scope() -> u64 {
    let request = 576;
    let headers = 22 * 24;
    let mut prior = 0;
    let mut peak = (0, 0);
    for (slot, &[key, _, payload, references, _]) in SYNTAX.iter().enumerate() {
        // wire_memo::footprint and wire_build::gather, with memo reuse disabled.
        let scratch = 16 * payload + 256 * references;
        let staged = request + headers + prior + scratch;
        if staged > peak.1 {
            peak = (slot, staged);
        }
        prior += if key <= 646 { 646 } else { 1292 };
    }
    // Last slot: reverse equation. Its prior blobs total 15_504 bytes;
    // its ten records contribute payload524 and references24, hence scratch14_528.
    assert_eq!(peak, (20, 576 + 528 + 15_504 + 16 * 524 + 256 * 24));
    let assembled = request + headers + prior + 12_864 + 24;
    assert_eq!(assembled, 30_788);
    let peak = peak.1.max(assembled);
    assert_eq!(peak, 31_136);
    peak
}

/// Maximum simultaneous service phase, before caller P and original baseline U.
pub(super) fn staged(replay: bool) -> u64 {
    let store = size_of::<nrz_cas::Cas>() as u64 + 384;
    let certificate = if replay { 10_119 + 24 } else { 0 };
    let candidate = if replay { 960 } else { 984 };
    let service = store + graph() + 5 * 64 + certificate;
    let scope = registered_scope();
    let candidate_build = 12_864 + 24 + 576 + 984 + if replay { 296 } else { 0 };
    let encoding = 12_864 + 24 + 576 + candidate + 64 + 10_119;
    let checking = 12_864 + 24 + 19_952 + 24 + 576 + candidate + 16;
    let publication = 576 + 19_952 + 24 + 32 + 2 * 10_119 + if replay { 1020 } else { 1044 };
    // Draft: 24+40*328 +10*144 +150 +38*24 +289 rational payload, plus indices104.
    let draft = 24 + 40 * 328 + 10 * 144 + 150 + 38 * 24 + 289 + 104;
    assert_eq!(draft, 16_039);
    // Before dropping pool capacity888, operands are not yet emitted: at most16_015.
    let assembly = draft.max(16_015);
    let batch = 104 + 15_019 + graph();
    let request = graph() + 984 + 104 + 576;
    let freeze = store
        + graph()
        + 10_119
        + if replay { 1146 } else { 1170 }
        + 576
        + 32
        + size_of::<nrz_cas::Cas>() as u64
        + 32
        + certificate;
    let phases = [
        288 + certificate,
        store + assembly + certificate,
        store + batch + certificate,
        store + request + certificate,
        service + scope,
        service + candidate_build,
        service + encoding,
        service + checking,
        service + publication,
        freeze,
    ];
    let maximum = phases.into_iter().max().unwrap_or_default();
    assert_eq!(maximum, if replay { 74_187 } else { 64_068 });
    maximum
}

/// Read the public certificate framing; none of these lengths comes from a Work observation.
pub(super) fn wire(bytes: &[u8]) -> super::TestResult {
    assert_eq!(bytes.len(), 10119);
    assert_eq!(&bytes[..8], b"NRZLCR01");
    let mut offset = 52 + 24;
    let mut lengths = Vec::new();
    for row in SYNTAX {
        let length = u32::from_le_bytes(bytes[offset..offset + 4].try_into()?) as usize;
        assert_eq!(length as u64, row[0]);
        offset += 4;
        let (actual, capacity) = super::keys::inventory(&bytes[offset..offset + length])?;
        assert_eq!(actual, row);
        assert_eq!(capacity, if length <= 646 { 646 } else { 1292 });
        lengths.push(length);
        offset += length;
    }
    assert_eq!(offset, 52 + 9924);
    super::keys::capacities(&lengths)?;
    Ok(())
}
