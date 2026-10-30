// SPDX-License-Identifier: AGPL-3.0-only
//! Prepared attempts retain complete question identity without granting selected authority.

use super::{decision_fixture::ledger, lattice_selection::prepared};
use crate::testing::declarations::TestResult;
use nrz_bindings::{DecisionError, LatticeDecisionCase, LatticeMetric};
use nrz_cas::lattice::LatticeConvention;

/// Matching evidence admits while foreign mathematical owners and changed conventions refuse.
/// ´test:integration:bindings-lattice-attempt-identity´
/// ´claim:bindings:lattice-attempt-identity´
#[test]
fn bindings_lattice_attempt_identity() -> TestResult {
    let mut source = prepared(2, LatticeConvention::Floor)?;
    let attempt = source.fixture.frame.cas.prepare_lattice_selection(
        &source.cover,
        &source.expected,
        &source.enclosure,
        &mut ledger()?,
    )?;
    source.case.validate_attempt(&attempt, &mut ledger()?)?;
    let foreign = prepared(2, LatticeConvention::Floor)?;
    let Err(fault) = foreign.case.validate_attempt(&attempt, &mut ledger()?) else {
        panic!("foreign mathematical handles must refuse at question matching");
    };
    assert!(matches!(fault, DecisionError::Case));
    let mut changed = source.case.specification().clone();
    changed.convention = LatticeConvention::Nearest;
    changed.metric = Some(LatticeMetric::EuclideanNormalized);
    let changed = LatticeDecisionCase::declare(&source.fixture.frame.cas, changed, &mut ledger()?)?;
    let Err(fault) = changed.validate_attempt(&attempt, &mut ledger()?) else {
        panic!("another convention must refuse at question matching");
    };
    assert!(matches!(fault, DecisionError::Case));
    Ok(())
}

/// An equal-valued target with different original syntax cannot inherit the prepared target identity.
/// ´test:integration:bindings-lattice-attempt-target-identity´
/// ´claim:bindings:lattice-attempt-target-identity´
#[test]
fn bindings_lattice_attempt_target_identity() -> TestResult {
    use super::lattice_fixture::{column_or_matrix, number, term};
    use crate::testing::readings::real;
    use nrz_cas::syntax::Operator;
    let mut source = prepared(2, LatticeConvention::Floor)?;
    let attempt = source.fixture.frame.cas.prepare_lattice_selection(
        &source.cover,
        &source.expected,
        &source.enclosure,
        &mut ledger()?,
    )?;
    let entries = source
        .fixture
        .frame
        .cas
        .operands(&source.fixture.target, &mut ledger()?)?;
    let zero = number(&mut source.fixture.frame, "0")?;
    let mut values = Vec::new();
    for entry in entries {
        values.push(term(
            &mut source.fixture.frame,
            Operator::Add,
            real(),
            &[entry, zero.clone()],
        )?);
    }
    let mut changed = source.case.specification().clone();
    changed.target = column_or_matrix(&mut source.fixture.frame, 2, 1, &values)?;
    assert_ne!(changed.target, source.fixture.target);
    let changed = LatticeDecisionCase::declare(&source.fixture.frame.cas, changed, &mut ledger()?)?;
    let Err(fault) = changed.validate_attempt(&attempt, &mut ledger()?) else {
        panic!("equal-valued original target substitution must refuse at question matching");
    };
    assert!(matches!(fault, DecisionError::Case));
    Ok(())
}
