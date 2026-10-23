// SPDX-License-Identifier: AGPL-3.0-only
//! Current scalar/family syntax survives independently reconstructed expected replay.

use crate::testing::common::TestResult;
use crate::testing::sorts::family_witness;
use nrz_cas::limits::{Budget, Limits};

/// Family dependencies round-trip while a supplied assumption remains explicitly assumed.
/// ´test:integration:scalar-family-expected-fresh-replay´
#[test]
fn scalar_family_expected_fresh_replay() -> TestResult {
    let (origin, witness) = family_witness()?;
    let bytes = origin.export_witness(&witness, &mut Budget::new(Limits::default())?)?;
    assert_eq!(bytes.get(..8), Some(b"NRZCAS05".as_slice()));
    let (mut receiver, expected) = family_witness()?;
    let replayed = receiver.replay(
        &bytes,
        expected.judgment(),
        &mut Budget::new(Limits::default())?,
    )?;
    assert_eq!(replayed.judgment(), expected.judgment());
    assert_eq!(replayed.hypotheses().len(), 1);
    assert_eq!(
        receiver.export_witness(&replayed, &mut Budget::new(Limits::default())?)?,
        bytes
    );
    Ok(())
}
