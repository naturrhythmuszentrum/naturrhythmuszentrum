// SPDX-License-Identifier: AGPL-3.0-only
//! Physical question identity is checked before interpreting a lattice tuple.

use super::{
    decision_fixture::ledger,
    lattice_fixture::lattice,
    lattice_physical::{source, specification},
};
use crate::testing::declarations::TestResult;
use nrz_bindings::{DecisionError, LatticeDecisionCase, LatticeMetric};
use nrz_cas::lattice::LatticeConvention;

/// A full turn cannot become a half turn under the same original mathematical basis.
/// ´claim:bindings:lattice-question-half-turn´
/// ´test:integration:bindings-lattice-question-half-turn´
#[test]
fn bindings_lattice_question_half_turn() -> TestResult {
    let mut fixture = lattice(0)?;
    let owner = source(0, "1/2", "original physical source")?;
    let spec = specification(&mut fixture, 0, &owner)?;
    let before = fixture.frame.cas.counts();
    assert!(matches!(
        LatticeDecisionCase::declare(&fixture.frame.cas, spec, &mut ledger()?),
        Err(DecisionError::LatticeMeaning)
    ));
    assert_eq!(fixture.frame.cas.counts(), before);
    Ok(())
}

/// Equal values from another physical owner and equal tuples under another convention keep distinct identities.
/// ´claim:bindings:lattice-question-identity´
/// ´test:integration:bindings-lattice-question-identity´
#[test]
fn bindings_lattice_question_identity() -> TestResult {
    let mut fixture = lattice(2)?;
    let owner = source(2, "1", "original physical source")?;
    let spec = specification(&mut fixture, 2, &owner)?;
    let original = LatticeDecisionCase::declare(&fixture.frame.cas, spec.clone(), &mut ledger()?)?;
    original.validate_attachment(&original.attachment(), &mut ledger()?)?;
    let mut nearest = spec;
    nearest.convention = LatticeConvention::Nearest;
    nearest.metric = Some(LatticeMetric::EuclideanNormalized);
    let nearest = LatticeDecisionCase::declare(&fixture.frame.cas, nearest, &mut ledger()?)?;
    assert!(matches!(
        nearest.validate_attachment(&original.attachment(), &mut ledger()?),
        Err(DecisionError::Attachment)
    ));
    let other = source(2, "1", "changed physical source")?;
    let other_spec = specification(&mut fixture, 2, &other)?;
    let other = LatticeDecisionCase::declare(&fixture.frame.cas, other_spec, &mut ledger()?)?;
    assert_eq!(
        other.specification().axes[0].target.exact(),
        original.specification().axes[0].target.exact()
    );
    assert!(matches!(
        other.validate_attachment(&original.attachment(), &mut ledger()?),
        Err(DecisionError::Attachment)
    ));
    Ok(())
}
