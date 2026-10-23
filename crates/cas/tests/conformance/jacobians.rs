// SPDX-License-Identifier: AGPL-3.0-only
//! Source contracts reject invented directions and invalid original operands.

use crate::testing::common::{TestResult, real};
use crate::testing::jacobians::{coordinates, integer, product};
use crate::testing::map_inverses::ledger;
use nrz_cas::syntax::Operator;
use nrz_cas::{Fault, FaultKind, InvalidCode};

/// A declared fixed reference refuses as a source, and duplicate directions refuse.
/// ´test:integration:source-jacobian-fixed-and-duplicate-refusals´
#[test]
fn source_jacobian_fixed_and_duplicate_refusals() -> TestResult {
    let (mut case, original, _) = product()?;
    for (directions, code) in [
        (
            vec!["x".into(), "r".into()],
            InvalidCode::JacobianFixedSource,
        ),
        (
            vec!["x".into(), "x".into()],
            InvalidCode::JacobianDuplicateSource,
        ),
    ] {
        let mut contract = coordinates();
        contract.sources = directions;
        let before = case.cas.counts();
        assert!(
            matches!(case.cas.source_jacobian_key(&case.context, &original, &contract, &mut ledger()?),
            Err(Fault { kind: FaultKind::Invalid(actual), .. }) if actual == code)
        );
        assert_eq!(case.cas.counts(), before);
    }
    Ok(())
}

/// Multiplication by zero cannot erase an invalid original reciprocal.
/// ´test:integration:source-jacobian-invalid-under-zero-refuses´
#[test]
fn source_jacobian_invalid_under_zero_refuses() -> TestResult {
    let (mut case, _, _) = product()?;
    let zero = integer(&mut case, 0)?;
    let one = integer(&mut case, 1)?;
    let reciprocal = case.term(Operator::Div, real(), &[one, zero.clone()])?;
    let original = case.term(Operator::Mul, real(), &[zero, reciprocal])?;
    let before = case.cas.counts();
    assert!(
        case.cas
            .source_jacobian_key(&case.context, &original, &coordinates(), &mut ledger()?)
            .is_err()
    );
    assert_eq!(case.cas.counts(), before);
    Ok(())
}

/// Unproved denominator requirements refuse even if no derivative columns are requested.
/// ´test:integration:source-jacobian-empty-directions-retain-domains´
#[test]
fn source_jacobian_empty_directions_retain_domains() -> TestResult {
    let (mut case, _, _) = product()?;
    let one = integer(&mut case, 1)?;
    let x = case.symbol("x")?;
    let original = case.term(Operator::Div, real(), &[one, x])?;
    let mut contract = coordinates();
    contract.sources.clear();
    let key = case
        .cas
        .source_jacobian_key(&case.context, &original, &contract, &mut ledger()?)?;
    let before = case.cas.counts();
    assert!(matches!(
        case.cas.source_jacobian(&key, &mut ledger()?),
        Err(Fault {
            kind: FaultKind::Invalid(InvalidCode::JacobianDomain),
            ..
        })
    ));
    assert_eq!(case.cas.counts(), before);
    Ok(())
}

/// Every declaration and fixed-role change participates in portable identity.
/// ´test:integration:source-jacobian-declaration-and-fixed-keys´
#[test]
fn source_jacobian_declaration_and_fixed_keys() -> TestResult {
    let (mut first, original, _) = product()?;
    let key =
        first
            .cas
            .source_jacobian_key(&first.context, &original, &coordinates(), &mut ledger()?)?;
    let mut changed = coordinates();
    changed.fixed.clear();
    let alternate =
        first
            .cas
            .source_jacobian_key(&first.context, &original, &changed, &mut ledger()?)?;
    assert_ne!(key, alternate);
    let mut receiver = crate::testing::map_inverses::fixture(&["x", "y", "r", "extra"])?;
    let x = receiver.symbol("x")?;
    let y = receiver.symbol("y")?;
    let original = receiver.term(Operator::Mul, real(), &[x, y])?;
    let alternate = receiver.cas.source_jacobian_key(
        &receiver.context,
        &original,
        &coordinates(),
        &mut ledger()?,
    )?;
    assert_ne!(key, alternate);
    assert!(matches!(
        receiver.cas.source_jacobian(&key, &mut ledger()?),
        Err(Fault {
            kind: FaultKind::Invalid(InvalidCode::ForeignHandle),
            ..
        })
    ));
    Ok(())
}
