// SPDX-License-Identifier: AGPL-3.0-only
//! Original scalar sources, joint declarations and one shared evaluated Jacobian.

use crate::reading_testing::TestResult;
use crate::spectral_testing::Fixture;
use nrz_bindings::Budget;
use nrz_matrix::{
    CompanionRef, CompanionSetDraft, CovarianceBlock, Heap, JointLimits, JointModel, LcSpectrum,
    ReadingBinding, ReciprocalDerivativeContract, ReciprocalDerivativeEvaluation, Source,
    SourceFactor, ValueBinding,
};
use nrz_scalar::{ArithmeticContext, Scalar};

/// One native input experiment preserves its complete original identities.
pub(super) struct Case<P: Scalar> {
    pub(super) heap: Heap,
    pub(super) original: [ReadingBinding<P>; 2],
    pub(super) selected: Vec<ValueBinding<P>>,
    pub(super) derivative: ReciprocalDerivativeEvaluation<P>,
    pub(super) companion: CompanionRef<P>,
}

/// Joint declarations precede both operator and root statistical admission.
pub(super) fn prepare<P: Scalar + 'static>(
    fixture: &Fixture,
    contract: &ReciprocalDerivativeContract,
    values: [[P; 3]; 2],
    coefficients: [P; 2],
    known: bool,
    budget: &mut Budget,
) -> TestResult<Case<P>> {
    let (mut heap, original) = fixture.import_with_budget(values, budget)?;
    let selected = original
        .iter()
        .map(|source| heap.select_reading(source, budget))
        .collect::<Result<Vec<_>, _>>()?;
    let handles = super::selected_joint::capture_handles(&mut heap, &selected)?;
    let sources = selected
        .iter()
        .map(|value| Source::new(value.symbol().descriptor(), 1))
        .collect::<Result<Vec<_>, _>>()?;
    let limits = JointLimits::default();
    let cross = if known {
        CovarianceBlock::Known
    } else {
        CovarianceBlock::Unknown
    };
    let model = JointModel::from_factor(
        &sources,
        SourceFactor::new(2, &coefficients, &[P::ONE], limits)?,
        &[
            CovarianceBlock::Known,
            cross.clone(),
            cross,
            CovarianceBlock::Known,
        ],
        limits,
    )?;
    let draft = CompanionSetDraft::new(
        model,
        super::selected_joint::capture_targets(&heap, handles)?,
        [vec![0], vec![1]],
    );
    let companion = heap.publish_joint(draft, limits)?;
    let derivative = heap.evaluate_reciprocal_derivative(
        contract,
        &original,
        &ArithmeticContext::new()?,
        budget,
    )?;
    Ok(Case {
        heap,
        original,
        selected,
        derivative,
        companion,
    })
}

/// Every root plan consumes the same original evaluated identity as the reciprocal binding.
pub(super) fn plan<P: Scalar + 'static>(
    fixture: &Fixture,
    case: &Case<P>,
    budget: &mut Budget,
) -> TestResult<LcSpectrum<P>> {
    Ok(fixture
        .plan(case.original.clone(), budget)?
        .with_reciprocal_statistics(&case.heap, case.derivative.clone(), case.companion, budget)?)
}

/// Canonical dyadic box and common fluctuation for both binary-format resource owners.
pub(super) fn nominal<P: Scalar + 'static>(
    fixture: &Fixture,
    contract: &ReciprocalDerivativeContract,
    budget: &mut Budget,
) -> TestResult<Case<P>> {
    prepare(
        fixture,
        contract,
        [
            [P::ONE / P::from_u8(4), P::ONE / P::from_u8(64), P::ZERO],
            [
                P::ONE / P::from_u8(16),
                P::ONE / (P::from_u8(128) * P::from_u8(2)),
                P::ZERO,
            ],
        ],
        [
            P::ONE / P::from_u8(128),
            P::ONE / (P::from_u8(128) * P::from_u8(4)),
        ],
        true,
        budget,
    )
}
