// SPDX-License-Identifier: AGPL-3.0-only
//! Fresh same-sort finite evaluation replay against independent expected syntax.

use crate::testing::coefficient_evaluation::{family, family_image, finite, selected_root, verify};
use crate::testing::common::TestResult;
use nrz_cas::limits::{Budget, Limits};

/// Scalar, selected-Real-root and mixed-family conclusions replay against freshly built expectations.
/// ´test:integration:scalar-finite-evaluation-expected-fresh-replay´
#[test]
fn scalar_finite_evaluation_expected_fresh_replay() -> TestResult {
    for choice in 0..4 {
        let build = || match choice {
            0 => finite(true, false, false),
            1 => selected_root(),
            2 => family(),
            _ => family_image(),
        };
        let (origin, proof) = verify(build()?)?;
        let bytes = origin.export_witness(&proof, &mut Budget::new(Limits::blind_cubic())?)?;
        let (mut receiver, expected) = verify(build()?)?;
        let replayed = receiver.replay(
            &bytes,
            expected.judgment(),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        assert_eq!(replayed.judgment(), expected.judgment());
        assert_eq!(
            receiver.export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
            bytes
        );
    }
    Ok(())
}
