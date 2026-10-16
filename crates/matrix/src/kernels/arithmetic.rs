// SPDX-License-Identifier: AGPL-3.0-only
//! Outward scalar steps collect into distinct center, total and evaluation regions.

use super::KernelError;
use crate::{NormalizedView, OperationError};
use nrz_accuracy::Nonnegative;
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// A temporary entry whose fields are collected into separate regions.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Entry<P: Scalar> {
    /// Finite represented result.
    pub(crate) center: P,
    /// Complete propagated deterministic radius.
    pub(crate) total: P,
    /// New evaluation radius, already covered by the total.
    pub(crate) error: P,
}

/// Admit a represented operand as an exact singleton for independent verification.
pub(crate) fn singleton<P: Scalar>(value: P) -> Result<Enclosure<P>, KernelError> {
    Ok(Enclosure::singleton(Finite::new(value)?))
}

/// Read one logical entry and the selected total exactly once.
pub(crate) fn operand<P: Scalar + 'static>(
    view: &NormalizedView<'_, P>,
    index: usize,
) -> Result<(P, Nonnegative<P>), OperationError> {
    if index >= view.extent().entries() {
        return Err(KernelError::Contract.into());
    }
    let row = index % view.extent().rows();
    let column = index / view.extent().rows();
    Ok((
        view.get(row, column)?,
        Nonnegative::new(view.bound(row, column)?.unwrap_or(P::ZERO)).map_err(KernelError::from)?,
    ))
}

/// Compare the candidate to an independently enclosed nominal mathematical image.
pub(crate) fn finish<P: Scalar>(
    center: P,
    nominal: Enclosure<P>,
    propagated: Nonnegative<P>,
    context: &ArithmeticContext,
) -> Result<Entry<P>, KernelError> {
    let center = Finite::new(center)?;
    let error = Nonnegative::new(nominal.displacement_from(center, context)?.get())?;
    Ok(Entry {
        center: center.get(),
        total: propagated.add(error, context)?.get(),
        error: error.get(),
    })
}

/// Sum bounds even for repeated aliases; no cancellation premise is inferred.
pub(crate) fn sum<P: Scalar + 'static>(
    left: (P, Nonnegative<P>),
    right: (P, Nonnegative<P>),
    subtract: bool,
    context: &ArithmeticContext,
) -> Result<Entry<P>, KernelError> {
    let first = singleton(left.0)?;
    let second = singleton(right.0)?;
    let (center, nominal) = if subtract {
        (left.0 - right.0, first.sub(second, context)?)
    } else {
        (left.0 + right.0, first.add(second, context)?)
    };
    finish(center, nominal, left.1.add(right.1, context)?, context)
}
