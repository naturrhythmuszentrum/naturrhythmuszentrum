// SPDX-License-Identifier: AGPL-3.0-only
//! Family fixture citations agree with the independently replayed primitive trace.

use crate::limits::{Budget, Limits};
use crate::request::Outcome;
use crate::testing::common::TestResult;

/// Positive, zero and negative quadratic pieces retain exactly the formal rules replay actually checks.
/// ´test:crate:mixed-family-formal-citations-match-checker´
#[test]
fn mixed_family_formal_citations_match_checker() -> TestResult {
    for sign in [-1, 0, 1] {
        let (mut fixture, mut request) = super::request(2)?;
        super::stratum(&mut fixture, &mut request, sign)?;
        let Outcome::Solved { answer, .. } = fixture.cas.run(&request) else {
            return Err("missing mixed family theorem".into());
        };
        let witness = answer.coverage.ok_or("family coverage")?;
        let bytes = fixture
            .cas
            .export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
        let mut fresh = crate::Cas::new(Limits::blind_cubic())?;
        let (checked, observations) = crate::testing::replay_answer::measured(|| {
            Ok(fresh.inspect(&bytes, &mut Budget::new(Limits::blind_cubic())?)?)
        })?;
        let citations =
            fresh.proof_citations(checked.proof(), &mut Budget::new(Limits::blind_cubic())?)?;
        assert_eq!(
            citations
                .iter()
                .map(|row| row.primitive)
                .collect::<Vec<_>>(),
            observations.iter().map(|row| row.rule).collect::<Vec<_>>()
        );
        for row in &citations {
            assert_eq!(
                row.formal_schema().lemma.name,
                row.primitive.specification().key
            );
            assert_eq!(
                row.formal_schema().lemma.version,
                row.primitive.specification().version
            );
            for premise in &row.premises {
                assert!(premise.kind.formal_schema().label.starts_with("assum:cas:"));
            }
        }
    }
    Ok(())
}
