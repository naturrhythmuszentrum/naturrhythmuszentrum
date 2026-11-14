// SPDX-License-Identifier: AGPL-3.0-only
//! Probe text uses a fixed fallible byte reservation, never geometric growth.

use std::io::Read;

/// Read through a fixed buffer with one byte reserved solely to detect truncation.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Used by crate siblings through the owning module boundary."
)]
pub(crate) fn bounded(reader: &mut impl Read, cap: u64) -> Result<String, &'static str> {
    text(reader, cap, false)
}

/// Keep a complete first CPU record inside the prefix even if later records exceed it.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Used by crate siblings through the owning module boundary."
)]
pub(crate) fn first_record(reader: &mut impl Read, cap: u64) -> Result<String, &'static str> {
    text(reader, cap, true)
}

/// Share the fixed reservation and byte limit across full-file and first-record probes.
fn text(reader: &mut impl Read, cap: u64, record: bool) -> Result<String, &'static str> {
    let limit = cap.checked_add(1).ok_or("probe cap overflow")?;
    let length = usize::try_from(limit).map_err(|_| "probe cap overflow")?;
    let mut budget = crate::memory::Budget::new(limit);
    let mut bytes = budget
        .reserve(length)
        .map_err(|_| "probe capacity unavailable")?;
    bytes.resize(length, 0_u8);
    let mut used = 0;
    loop {
        let count = reader
            .read(&mut bytes[used..])
            .map_err(|_| "probe read failed")?;
        if count == 0 {
            break;
        }
        used = used.checked_add(count).ok_or("probe cap overflow")?;
        if record {
            let prefix = &bytes[..used.min(length - 1)];
            if let Some(end) = prefix.windows(2).position(|pair| pair == b"\n\n") {
                bytes.truncate(end + 2);
                return String::from_utf8(bytes).map_err(|_| "invalid probe text");
            }
        }
        if used == length {
            return Err("ProbeTruncated");
        }
    }
    bytes.truncate(used);
    String::from_utf8(bytes).map_err(|_| "invalid probe text")
}

#[cfg(test)]
mod tests {
    use super::{bounded, first_record};
    use std::io::Cursor;

    #[test]
    fn first_cpu_record_survives_a_truncated_tail() {
        let text = b"cpu MHz : 1\n\nprocessor : 1\n";
        assert_eq!(
            first_record(&mut Cursor::new(text), 13).as_deref(),
            Ok("cpu MHz : 1\n\n")
        );
        assert_eq!(
            first_record(&mut Cursor::new(text), 12),
            Err("ProbeTruncated")
        );
        assert_eq!(bounded(&mut Cursor::new(text), 13), Err("ProbeTruncated"));
        assert_eq!(
            first_record(&mut Cursor::new(b"a\n\n\xff"), 3).as_deref(),
            Ok("a\n\n")
        );
    }

    #[test]
    fn exact_cap_is_accepted_but_truncation_and_non_utf8_are_refused() {
        assert_eq!(bounded(&mut Cursor::new(b"abc"), 3).as_deref(), Ok("abc"));
        assert_eq!(bounded(&mut Cursor::new(b"abcd"), 3), Err("ProbeTruncated"));
        assert_eq!(
            bounded(&mut Cursor::new([255]), 1),
            Err("invalid probe text")
        );
        assert_eq!(bounded(&mut Cursor::new(b""), 0).as_deref(), Ok(""));
    }
}
