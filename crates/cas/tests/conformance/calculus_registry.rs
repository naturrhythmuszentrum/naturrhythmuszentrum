// SPDX-License-Identifier: AGPL-3.0-only
//! Reserved analytic keys never acquire authority from an otherwise valid wire manifest.

use crate::testing::budgets::derivation_budget as budget;
use crate::testing::common::TestResult;
use nrz_cas::{Cas, FaultKind, InvalidCode};

/// Later schemas and recipe keys are refused before any record can be imported.
/// ´test:integration:calculus-reserved-key-replay´
#[test]
fn calculus_reserved_key_replay() -> TestResult {
    for key in [
        "limit.case-restrict",
        "series.case-restrict",
        "dist.affine-pullback",
        "dist.simple-zero-pullback",
        "dist.primitive-unique",
        "jet.jump",
    ] {
        let mut bytes = b"NRZCAS02".to_vec();
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&u32::try_from(key.len())?.to_le_bytes());
        bytes.extend_from_slice(key.as_bytes());
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&[0; 16]);
        let mut cas = Cas::new(budget()?.limits().clone())?;
        let before = cas.canonical_snapshot(&mut budget()?)?;
        let error = cas
            .inspect(&bytes, &mut budget()?)
            .err()
            .ok_or("reserved key accepted")?;
        assert_eq!(
            error.kind,
            FaultKind::Invalid(InvalidCode::UnsupportedVersion)
        );
        assert_eq!(cas.canonical_snapshot(&mut budget()?)?, before);
    }
    Ok(())
}
