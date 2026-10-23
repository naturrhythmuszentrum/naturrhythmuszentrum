// SPDX-License-Identifier: AGPL-3.0-only
//! Formal citations match the independent LC replay checker's actual primitive trace.

use crate::proof::HypothesisKind;
use crate::request::Outcome;
use crate::testing::common::TestResult;

/// Every replayed LC application cites the rule actually checked, retaining strict sign premises.
/// ´test:crate:lc-formal-citations-match-checker´
#[test]
fn lc_formal_citations_match_checker() -> TestResult {
    let (mut source, request) = super::fixture::request([1, 1], true)?;
    let Outcome::Solved { answer, .. } = source.cas.run(&request) else {
        return Err("missing original LC theorem".into());
    };
    let witness = answer.coverage.ok_or("LC coverage")?;
    let bytes = source.cas.export_witness(&witness, &mut super::ledger()?)?;
    let (mut receiver, request) = super::fixture::request([1, 1], true)?;
    let proposed =
        crate::testing::replay_answer::proposal(&source.cas, &answer.set, &mut receiver.cas)?;
    let expected = super::replay::expected(&receiver, &request, &proposed)?;
    let (checked, observations) = crate::testing::replay_answer::measured(|| {
        Ok(receiver
            .cas
            .replay(&bytes, &expected, &mut super::ledger()?)?)
    })?;
    let citations = receiver
        .cas
        .proof_citations(checked.proof(), &mut super::ledger()?)?;
    assert_eq!(citations.len(), 12);
    let actual: Vec<_> = citations.iter().map(|row| row.primitive).collect();
    let executed: Vec<_> = observations.iter().map(|row| row.rule).collect();
    assert_eq!(actual, executed);
    for row in &citations {
        assert_eq!(
            row.formal_schema().lemma.name,
            row.primitive.specification().key
        );
        assert_eq!(
            row.formal_schema().lemma.version,
            row.primitive.specification().version
        );
    }
    assert!(
        citations
            .iter()
            .flat_map(|row| &row.premises)
            .any(|premise| premise.kind == HypothesisKind::StrictSign)
    );
    assert_eq!(
        receiver
            .cas
            .export_witness(&checked, &mut super::ledger()?)?,
        bytes
    );
    Ok(())
}
