// SPDX-License-Identifier: AGPL-3.0-only
//! Direct cartesian images preserve the ordered binary law and nominal nearest center.

use super::{
    DomainReason, KernelError,
    arithmetic::{Entry, singleton},
};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// Closed native binary functions supported by the masked application adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryElementwise {
    /// Cartesian product of two represented boxes.
    Product,
    /// Ordered direct division by a box separated from zero.
    Quotient,
}

/// One selected invocation performs no allocation or exact arithmetic.
pub(crate) fn evaluate_selected<P: Scalar>(
    centers: [P; 2],
    radii: [P; 2],
    function: BinaryElementwise,
    entry: usize,
    context: &ArithmeticContext,
) -> Result<Entry<P>, KernelError> {
    #[cfg(test)]
    crate::loan::fault::entered_scalar();
    evaluate(centers, radii, function, entry, context).map_err(|cause| match cause {
        KernelError::Arithmetic(nrz_scalar::ArithmeticFailure::Overflow) => {
            domain(function, entry, DomainReason::UnboundedImage)
        }
        other => other,
    })
}

/// Reject the exact divisor premise before a box-construction overflow can obscure it.
fn evaluate<P: Scalar>(
    [left, right]: [P; 2],
    [left_radius, right_radius]: [P; 2],
    function: BinaryElementwise,
    entry: usize,
    context: &ArithmeticContext,
) -> Result<Entry<P>, KernelError> {
    nrz_accuracy::Nonnegative::new(left_radius)?;
    nrz_accuracy::Nonnegative::new(right_radius)?;
    if function == BinaryElementwise::Quotient && right.abs() <= right_radius {
        return Err(domain(function, entry, DomainReason::ZeroInDivisor));
    }
    let left_nominal = singleton(left)?;
    let right_nominal = singleton(right)?;
    let left_whole = whole(left_nominal, left_radius, context)?;
    let right_whole = whole(right_nominal, right_radius, context)?;
    let touches_zero = right_whole.lower().get() <= P::ZERO && right_whole.upper().get() >= P::ZERO;
    if function == BinaryElementwise::Quotient && touches_zero {
        return Err(domain(function, entry, DomainReason::ZeroInDivisor));
    }
    let image = apply(left_whole, right_whole, function, context)?;
    let nominal = apply(left_nominal, right_nominal, function, context)?;
    let candidate = Finite::new(match function {
        BinaryElementwise::Product => left * right,
        BinaryElementwise::Quotient => left / right,
    })?;
    let error = nominal.displacement_from(candidate, context)?.get();
    let image_radius = image.displacement_from(candidate, context)?.get();
    let total = if image_radius > error {
        image_radius
    } else {
        error
    };
    Ok(Entry {
        center: candidate.get(),
        total,
        error,
    })
}

/// Preserve outward box construction even for an absent total represented by zero.
fn whole<P: Scalar>(
    center: Enclosure<P>,
    radius: P,
    context: &ArithmeticContext,
) -> Result<Enclosure<P>, KernelError> {
    let radius = singleton(radius)?;
    Ok(Enclosure::new(
        center.sub(radius, context)?.lower().get(),
        center.add(radius, context)?.upper().get(),
    )?)
}

/// Division is a direct endpoint-pair operation, never reciprocal followed by multiplication.
fn apply<P: Scalar>(
    left: Enclosure<P>,
    right: Enclosure<P>,
    function: BinaryElementwise,
    context: &ArithmeticContext,
) -> Result<Enclosure<P>, KernelError> {
    Ok(match function {
        BinaryElementwise::Product => left.mul(right, context)?,
        BinaryElementwise::Quotient => left.div(right, context)?,
    })
}

/// Domain failures retain the original law and original flat entry.
const fn domain(function: BinaryElementwise, entry: usize, reason: DomainReason) -> KernelError {
    KernelError::BinaryDomain {
        function,
        entry,
        reason,
    }
}
