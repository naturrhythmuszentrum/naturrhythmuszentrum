// SPDX-License-Identifier: AGPL-3.0-only
//! Independent verifier interactions that accept deliberately supplied private candidates.

use crate::kernel_oracle::Rational;
use crate::solve_testing::{TestResult, shapes};
use crate::solve_verification::{SolveWork, center, inverse};
use crate::{Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationDraft, SolveError};
use nrz_bindings::Descriptor;
use nrz_scalar::ArithmeticContext;

/// A nonzero residual is certified from heap views independently of the supplied inverse.
/// ´test:crate:solve-verifier-encloses-independent-residual-and-inverse-norms´
#[test]
fn solve_verifier_encloses_independent_residual_and_inverse_norms() -> TestResult {
    let [operator, rhs, _] = shapes(2, 1)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let first = heap.import_represented(
        Extent::new(2, 2)?,
        &Descriptor::ratio(),
        &[2.0_f64, 0.0, 0.0, 4.0],
    )?;
    let second =
        heap.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[2.0_f64, 8.0])?;
    let limits = ExecutionLimits::default();
    let operation = OperationDraft::copy(vec![operator, rhs]).check(limits)?;
    let bound =
        Binding::new(operation, vec![first.value(), second.value()], limits).validate(&mut heap)?;
    let inputs = bound.normalized_inputs()?;
    let operator_view = inputs.views().first().ok_or("operator")?;
    let rhs_view = inputs.views().get(1).ok_or("rhs")?;
    let context = ArithmeticContext::new()?;
    let witness = inverse(
        operator_view,
        &[0.5, 0.0, 0.0, 0.25],
        &mut SolveWork::new(8),
        &context,
    )?;
    let square =
        Rational::binary(witness.magnitude)?.multiply(Rational::binary(witness.magnitude)?)?;
    assert!(Rational::new(5, 16)?.at_most(square)?);
    let certificate = center(
        operator_view,
        &crate::solve::RightHandSide::Physical(rhs_view),
        &[1.125, 2.0],
        &witness,
        &mut SolveWork::new(4),
        &context,
    )?;
    assert!(Rational::new(1, 4)?.at_most(Rational::binary(certificate.residual)?)?);
    assert!(Rational::new(1, 1)?.covered(1.125, certificate.radius)?);
    assert!(Rational::new(2, 1)?.covered(2.0, certificate.radius)?);
    Ok(())
}

/// A valid-looking candidate cannot bypass the independently evaluated strict inverse margin.
/// ´test:crate:inverse-witness-defect-below-at-and-above-one´
#[test]
fn inverse_witness_defect_below_at_and_above_one() -> TestResult {
    let [operator, _, _] = shapes(1, 1)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[1.0_f64])?;
    let limits = ExecutionLimits::default();
    let bound = Binding::new(
        OperationDraft::copy(vec![operator]).check(limits)?,
        vec![source.value()],
        limits,
    )
    .validate(&mut heap)?;
    let inputs = bound.normalized_inputs()?;
    let view = inputs.views().first().ok_or("operator")?;
    let context = ArithmeticContext::new()?;
    let witness = inverse(view, &[0.5], &mut SolveWork::new(1), &context)?;
    assert!(Rational::new(1, 1)?.at_most(Rational::binary(witness.inverse)?)?);
    for candidate in [0.0, -0.5] {
        assert!(matches!(
            inverse(view, &[candidate], &mut SolveWork::new(1), &context),
            Err(SolveError::CannotCertifyInverse)
        ));
    }
    assert!(matches!(
        inverse(view, &[1.0], &mut SolveWork::new(0), &context),
        Err(SolveError::ResourceLimit)
    ));
    Ok(())
}

/// The design's conservative constants reproduce its exact displayed 27/74 theorem bound.
/// ´test:crate:solve-theorem-reproduces-the-exact-worked-bound´
#[test]
fn solve_theorem_reproduces_the_exact_worked_bound() -> TestResult {
    let context = ArithmeticContext::new()?;
    let certificate = crate::solve_verification::CenterCertificate {
        residual: 0.0_f64,
        magnitude: 2.5,
        radius: 0.0,
    };
    let family = crate::solve_verification::perturbation(0.75, &certificate, 0.1, 0.2, &context)?;
    assert!(Rational::new(27, 74)?.at_most(Rational::binary(family.radius)?)?);
    assert!(Rational::binary(family.margin)?.at_most(Rational::new(37, 40)?)?);
    assert!(Rational::new(22, 19)?.covered(1.0, family.radius)?);
    let upper = nrz_accuracy::Nonnegative::new(family.radius)?;
    for (tolerance, decision) in [
        (0.4, nrz_accuracy::BudgetDecision::Met),
        (1.0 / 3.0, nrz_accuracy::BudgetDecision::Exceeded),
    ] {
        let budget = nrz_accuracy::Budget::new(tolerance, 0.0, 1.0)?;
        assert_eq!(
            nrz_accuracy::assess_budget(Some(&budget), upper, &context)?.decision(),
            decision
        );
    }
    Ok(())
}
