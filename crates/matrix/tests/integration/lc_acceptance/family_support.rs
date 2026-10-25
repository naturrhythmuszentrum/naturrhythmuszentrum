// SPDX-License-Identifier: AGPL-3.0-only
//! Both publications retain one original experiment and one exact derivative preparation.

use super::root_support as inputs;
use crate::reading_testing::TestResult;
use crate::spectral_testing::Fixture;
use nrz_bindings::Budget;
use nrz_matrix::{Binding, ExecutionLimits, MomentRef, PublishedBundle, PublishedLcSpectrum};
use nrz_scalar::{ArithmeticContext, Scalar};

pub(super) struct Outputs<P: Scalar> {
    pub(super) operator: PublishedBundle<P>,
    pub(super) moments: MomentRef<P>,
    pub(super) roots: PublishedLcSpectrum<P>,
}

/// The operator and root producers consume exactly the same evaluated original partials.
pub(super) fn publish<P: Scalar + 'static>(
    fixture: &Fixture,
    case: &mut inputs::Case<P>,
    budget: &mut Budget,
) -> TestResult<Outputs<P>> {
    let operator = operator(fixture, case, budget)?;
    let moments = case.heap.moment_companion(operator.value())?;
    let plan = inputs::plan(fixture, case, budget)?;
    let publication = fixture.publication(plan, budget)?;
    let roots = case
        .heap
        .publish_lc_spectrum(&publication, &ArithmeticContext::new()?, budget)?;
    Ok(Outputs {
        operator,
        moments,
        roots,
    })
}

/// Native publication preserves the existing operation and statistical inventory contracts.
pub(super) fn operator<P: Scalar + 'static>(
    fixture: &Fixture,
    case: &mut inputs::Case<P>,
    budget: &mut Budget,
) -> TestResult<PublishedBundle<P>> {
    let bound = Binding::from_symbols(
        super::operator(fixture)?,
        case.selected.clone(),
        ExecutionLimits::default(),
    )
    .with_reciprocal_statistics(case.derivative.clone(), budget)?
    .validate_with_resource_budget(&mut case.heap, budget)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    crate::kernel_testing::output(&outcome)
}
