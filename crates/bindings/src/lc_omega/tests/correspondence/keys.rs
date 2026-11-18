// SPDX-License-Identifier: AGPL-3.0-only
//! Independent decoder for the closed fixture's canonical syntax inventory.
use super::TestResult;

/// Test-only cursor; every byte is from the independently exported canonical fixture.
struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}
impl<'a> Reader<'a> {
    /// Borrow a framed field and advance the independent decoder.
    fn take(&mut self, count: usize) -> &'a [u8] {
        let start = self.position;
        self.position += count;
        &self.bytes[start..self.position]
    }
    /// Fixed little-endian canonical count.
    fn count(&mut self) -> usize {
        let mut bytes = [0; 4];
        bytes.copy_from_slice(self.take(4));
        u32::from_le_bytes(bytes) as usize
    }
    /// Exact one-byte tag.
    fn byte(&mut self) -> u8 {
        self.take(1)[0]
    }
    /// A bounded spelling has its own canonical count.
    fn name(&mut self) {
        let count = self.count();
        self.take(count);
    }
    /// The fixture contains only Real and Boolean value sorts.
    fn sort(&mut self) {
        assert_eq!(self.byte(), 0);
        let kind = self.byte();
        if kind == 1 {
            assert_eq!(self.byte(), 1);
        } else {
            assert_eq!(kind, 2);
        }
    }
    /// Exact complete header with ten sorted Real declarations.
    fn environment(&mut self) {
        assert_eq!(self.take(2), [1, 0]);
        for _ in 0..self.count() {
            self.name();
            self.name();
            self.sort();
        }
    }
}

/// Reproduce Encoder's public Vec allocation requests, independently of its observed Work.
fn append(buffer: &mut Vec<u8>, count: usize) -> TestResult {
    buffer.try_reserve(count)?;
    buffer.resize(buffer.len() + count, 0);
    Ok(())
}

/// Derive record count, payload, semantic references and rational tariff from one syntax key.
pub(super) fn inventory(bytes: &[u8]) -> TestResult<([u64; 5], usize)> {
    let mut reader = Reader { bytes, position: 0 };
    assert_eq!(reader.take(8), b"NRZSYN04");
    assert_eq!(reader.count(), 0);
    let count = reader.count();
    let mut payload = 0;
    let mut references = 0;
    let mut arithmetic = 0;
    let mut capacity = Vec::new();
    for size in [8, 4, 4] {
        append(&mut capacity, size)?;
    }
    for _ in 0..count {
        let tag = reader.byte();
        let length = reader.count();
        let mut body = Reader {
            bytes: reader.take(length),
            position: 0,
        };
        assert_eq!(body.count(), 1);
        body.byte();
        assert_eq!(body.byte(), 0);
        let data = body.take(length - 6);
        let (edges, work) = record(tag, data);
        payload += data.len() as u64;
        references += edges;
        arithmetic += work;
        append(&mut capacity, 5 + length)?;
    }
    reader.count();
    assert_eq!(reader.position, bytes.len());
    append(&mut capacity, 4)?;
    Ok((
        [
            bytes.len() as u64,
            count as u64,
            payload,
            references,
            arithmetic,
        ],
        capacity.capacity(),
    ))
}

/// Count every supported semantic reference, including registered scalar-space dependencies.
fn record(tag: u8, bytes: &[u8]) -> (u64, u64) {
    let mut reader = Reader { bytes, position: 0 };
    let mut edges = 0;
    let mut arithmetic = 0;
    match tag {
        34 => {
            reader.name();
            assert_eq!(reader.count(), 1);
        }
        0 => {
            assert_eq!(reader.byte(), 0);
            reader.environment();
            let count = reader.count();
            edges += count as u64;
            reader.take(4 * count);
            assert_eq!(reader.count(), 0);
        }
        1 => {
            reader.count();
            edges += 1;
            assert_eq!(reader.count(), 0);
            assert_eq!(reader.take(3), [0; 3]);
        }
        8 => {
            reader.count();
            assert_eq!(reader.byte(), 0);
            reader.count();
            edges += 2;
            assert_eq!(reader.byte(), 0);
            assert_eq!(reader.count(), 0);
        }
        3 => {
            let result = expression(&mut reader);
            edges = result.0;
            arithmetic = result.1;
        }
        _ => panic!("unexpected canonical fixture tag {tag}"),
    }
    assert_eq!(reader.position, bytes.len());
    (edges, arithmetic)
}

/// Decode the exact operator payload before its ordered operands and optional graph metadata.
fn expression(reader: &mut Reader<'_>) -> (u64, u64) {
    reader.count();
    let mut edges = 1;
    assert_eq!(reader.count(), 0);
    if reader.byte() == 1 {
        reader.count();
        edges += 1;
    }
    reader.sort();
    let operator = reader.byte();
    let mut arithmetic = 0;
    match operator {
        0 => {
            reader.byte();
            let numerator = reader.count();
            assert!(numerator <= 1);
            reader.take(numerator);
            let denominator = reader.count();
            assert_eq!(denominator, 1);
            reader.take(denominator);
            arithmetic = 32;
        }
        1 => reader.name(),
        10 | 17 => {
            reader.byte();
        }
        _ => assert!(matches!(operator, 6..=8)),
    }
    assert_eq!(reader.byte(), 0);
    let operands = reader.count();
    edges += operands as u64;
    reader.take(4 * operands);
    assert_eq!(reader.take(2), [0; 2]);
    assert_eq!(reader.count(), 0);
    (edges, arithmetic)
}

/// Scope and wire growth follow their actual raw-write request sequences, separately from length.
pub(super) fn capacities(lengths: &[usize]) -> TestResult {
    let mut scope = Vec::new();
    for count in [8, 4, 4, 4, 4] {
        append(&mut scope, count)?;
    }
    for length in lengths {
        append(&mut scope, 4)?;
        append(&mut scope, *length)?;
    }
    assert_eq!((scope.len(), scope.capacity()), (9924, 12864));
    let mut wire = Vec::new();
    for count in [8, 4, 28, 8, 4, scope.len(), 4] {
        append(&mut wire, count)?;
    }
    for _ in 0..10 {
        append(&mut wire, 4)?;
    }
    for _ in 0..99 {
        append(&mut wire, 1)?;
    }
    assert_eq!((wire.len(), wire.capacity()), (10119, 19952));
    Ok(())
}
