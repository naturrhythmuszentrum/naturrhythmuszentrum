// SPDX-License-Identifier: AGPL-3.0-only
//! Independently specified complete-answer replay and portable output lifecycle.

use crate::testing::answer::{budget, encoded, prepared};
use crate::testing::common::TestResult;

/// One import yields both original expected theorems, each with byte-stable standalone export.
/// ´test:integration:complete-answer-portable-pair´
#[test]
fn complete_answer_portable_pair() -> TestResult {
    let bytes = encoded()?;
    let mut receiver = prepared()?;
    let mut ledger = budget()?;
    let (coverage, presentation) = receiver.base.cas.replay_answer(
        &bytes,
        &receiver.coverage,
        &receiver.presentation,
        &mut ledger,
    )?;
    assert_eq!(coverage.judgment(), &receiver.coverage);
    assert_eq!(presentation.judgment(), &receiver.presentation);
    assert!(coverage.is_discharged());
    assert!(presentation.is_discharged());
    assert_eq!(
        coverage.hypotheses(),
        [] as [nrz_cas::domain::HypothesisRef; 0]
    );
    assert_eq!(
        presentation.hypotheses(),
        [] as [nrz_cas::domain::HypothesisRef; 0]
    );
    assert!(ledger.usage().work > 0);
    assert_eq!(
        receiver
            .base
            .cas
            .export_witness(&presentation, &mut budget()?)?,
        bytes
    );
    let coverage_bytes = receiver
        .base
        .cas
        .export_witness(&coverage, &mut budget()?)?;
    let mut independent = prepared()?;
    let standalone =
        independent
            .base
            .cas
            .replay(&coverage_bytes, &independent.coverage, &mut budget()?)?;
    assert_eq!(
        independent
            .base
            .cas
            .export_witness(&standalone, &mut budget()?)?,
        coverage_bytes
    );
    Ok(())
}
