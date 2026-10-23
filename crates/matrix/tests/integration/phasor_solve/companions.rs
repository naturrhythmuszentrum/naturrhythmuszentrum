// SPDX-License-Identifier: AGPL-3.0-only
//! Restored scalar copies retain the actual value and both separately addressed parameter channels.

use crate::reading_testing::TestResult;
use nrz_bindings::{CompanionKind, readings::CompanionReadingKind};
use nrz_matrix::{Heap, ReadingBinding};
use nrz_scalar::Scalar;

/// Every selected scalar and all eight companion payloads match their complete producer arrays.
pub(super) fn check<P: Scalar + 'static>(
    heap: &Heap,
    readings: &[ReadingBinding<P>],
    centers: &[P; 4],
    totals: &[P; 4],
    errors: &[P; 4],
) -> TestResult {
    for (index, copy) in readings[1..5].iter().enumerate() {
        assert!(heap.reading_entries(copy)?[0].same_bits(centers[index]));
        let mut channels = 0;
        for (position, companion) in copy.companions().iter().enumerate() {
            if companion.targets()[0].reading() != copy.reading() {
                continue;
            }
            let expected = match companion.companion().kind() {
                CompanionReadingKind::Radius(CompanionKind::DeterministicBound) => totals[index],
                CompanionReadingKind::Radius(CompanionKind::NumericalError) => errors[index],
                CompanionReadingKind::Covariance => return Err("unexpected covariance".into()),
            };
            assert!(heap.reading_companion_entries(copy, position)?[0].same_bits(expected));
            channels += 1;
        }
        assert_eq!(channels, 2);
    }
    Ok(())
}
