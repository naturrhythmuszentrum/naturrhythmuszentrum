// SPDX-License-Identifier: AGPL-3.0-only
//! Outward implicit differentiation and full original covariance contraction retain local scope.

use super::{AT, Error, Heap, Input, Moments};
use crate::{CovarianceRepresentation, HeapError, JointLimits};
use nrz_bindings::{Budget, readings::algebra::limits::BudgetKind};
use nrz_scalar::{ArithmeticContext, DecodedLiteral, Enclosure, Finite, Scalar};

/// All root partials and all four source covariance entries are enclosed before publication.
pub(super) fn evaluate<P: Scalar + 'static>(
    heap: &Heap,
    input: &Input<P>,
    constants: &[DecodedLiteral<P>; 3],
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Moments<P>, Error> {
    let scratch = u64::try_from(
        24 * size_of::<Enclosure<P>>()
            + size_of::<crate::StatisticalView<'_, P>>()
            + 4 * size_of::<crate::RegionView<'_, P>>(),
    )
    .map_err(|_| HeapError::ExtentOverflow)?;
    budget.check(BudgetKind::StagedPayloadBytes, scratch, AT)?;
    let partials = partials(heap, input, constants, context, budget)?;
    let view = heap.statistical(input.companion)?;
    let terms = match view.model().representation() {
        CovarianceRepresentation::Factor(factor) => factor.columns().max(1),
        CovarianceRepresentation::Dense(_) => 1,
    };
    let work = terms
        .checked_mul(32)
        .and_then(|value| value.checked_add(12))
        .ok_or(HeapError::ExtentOverflow)?;
    budget.work(
        u64::try_from(work).map_err(|_| HeapError::ExtentOverflow)?,
        AT,
    )?;
    let mut variance = Enclosure::singleton(Finite::ZERO);
    for (row, &left) in input.columns.iter().enumerate() {
        for (column, &right) in input.columns.iter().enumerate() {
            let covariance =
                view.model()
                    .covariance(left, right, context, JointLimits::new(2, 4, work))?;
            let term = partials[row]
                .mul(covariance, context)?
                .mul(partials[column], context)?;
            variance = variance.add(term, context)?;
        }
    }
    Ok(Moments { partials, variance })
}

/// Differentiate the checked positive characteristic root using the evaluated operator partials.
fn partials<P: Scalar + 'static>(
    heap: &Heap,
    input: &Input<P>,
    constants: &[DecodedLiteral<P>; 3],
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<[Enclosure<P>; 2], Error> {
    let point = input.derivative.operating_point(heap, context, budget)?;
    let values = input.derivative.values(heap, context, budget)?;
    let [inductance, capacitance] = point else {
        return Err(Error::Identity);
    };
    let [_, lower_l, upper_l, _, _, lower_c, upper_c, _] = values else {
        return Err(Error::Identity);
    };
    budget.work(24, AT)?;
    let upper = constants[0]
        .enclosure()
        .div(Enclosure::singleton(Finite::new(*capacitance)?), context)?;
    let lower = constants[1]
        .enclosure()
        .div(Enclosure::singleton(Finite::new(*inductance)?), context)?;
    let magnitude = upper.mul(lower, context)?.sqrt(context)?;
    let twice = Enclosure::singleton(Finite::new(P::ONE + P::ONE)?);
    let denominator = twice
        .mul(magnitude, context)?
        .mul(constants[2].enclosure(), context)?;
    let mut result = [Enclosure::singleton(Finite::ZERO); 2];
    for (slot, (down, up)) in [(lower_l, upper_l), (lower_c, upper_c)]
        .into_iter()
        .enumerate()
    {
        result[slot] = lower
            .mul(*up, context)?
            .sub(upper.mul(*down, context)?, context)?
            .div(denominator, context)?;
    }
    Ok(result)
}
