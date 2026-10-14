// SPDX-License-Identifier: AGPL-3.0-only
//! Independent canonical cancellation bytes and their test-only hex reader.
//!
//! Compiled only in the public test target.

use super::common::TestResult;

/// Independently assembled format-one bytes for x/x = 1 with residual x nonzero.
const CANCELLATION_WIRE_HEX: &str = concat!(
    "4e525a4341533031010000000900000065712e63616e63656c010000000100020000000100000078",
    "01000000780001010100000079010000007900010100000000030000000101000000780001010000",
    "00000000000000080001010000000000020000000000000000000000000001000000010100000001",
    "00010100000000000000000000000100000002000000010000000100000000000000000100000000",
    "00000000010000000200000000000000000000000000000000000000",
);

/// Decode this fixed test vector without using the production witness codec.
pub(crate) fn canonical_fixture() -> TestResult<Vec<u8>> {
    let mut bytes = Vec::new();
    for pair in CANCELLATION_WIRE_HEX.as_bytes().as_chunks::<2>().0 {
        bytes.push(u8::from_str_radix(std::str::from_utf8(pair)?, 16)?);
    }
    Ok(bytes)
}
