// SPDX-License-Identifier: AGPL-3.0-only
//! Member-specific scalar readings feed the common selected traversal and tracked store loop.

use super::Traversal;
use crate::loan::application::{CellOutcome, member::Member};
use crate::loan::{
    HostChannel, LoanError,
    payload::{LoanChannel, set},
};
use nrz_scalar::{Expm1Failure, Expm1Input, Scalar};

/// Stack-only native readings preserve the original storage format without conversion.
pub(in crate::loan::application) enum Values<P: Scalar> {
    /// Center, total and numerical error in the closed primitive format.
    Closed([P; 3]),
    /// Three readings and Lower/Upper in the prepared binary64 format.
    Prepared([f64; 5]),
}
impl Member<'_> {
    /// A refused component still initializes every role with safe placeholders.
    pub(in crate::loan::application) const fn zeros<P: Scalar>(self) -> Values<P> {
        match self {
            Self::Square(_) | Self::Reciprocal(_) | Self::SquareRoot(_) => {
                Values::Closed([P::ZERO; 3])
            }
            Self::PreparedExpm1(..) => Values::Prepared([0.0; 5]),
        }
    }
    /// Exactly the admitted input loads and one native invocation occur at each selected component.
    #[expect(
        clippy::too_many_arguments,
        reason = "original address, reader and retained law are borrowed without a per-cell owner"
    )]
    pub(in crate::loan::application) fn scalar<P: Scalar + 'static>(
        self,
        reader: Option<&crate::TileReader<'_, P>>,
        law: Option<&crate::loan::retained_expm1::Attachment>,
        bounded: bool,
        cell: usize,
        component: usize,
        entry: usize,
        permit: &Traversal,
    ) -> Result<Values<P>, CellOutcome> {
        match self {
            Self::Square(_) | Self::Reciprocal(_) | Self::SquareRoot(_) => {
                let function = if matches!(self, Self::Square(_)) {
                    crate::Elementwise::Square
                } else if matches!(self, Self::Reciprocal(_)) {
                    crate::Elementwise::Reciprocal
                } else {
                    crate::Elementwise::SquareRoot
                };
                let _span = match self {
                    Self::Reciprocal(_) => Some(
                        tracing::trace_span!("nrz.kernel", family = "reciprocal", revision = 1_u64)
                            .entered(),
                    ),
                    Self::SquareRoot(_) => Some(
                        tracing::trace_span!(
                            "nrz.kernel",
                            family = "square root",
                            revision = 1_u64
                        )
                        .entered(),
                    ),
                    Self::Square(_) | Self::PreparedExpm1(..) => None,
                };
                closed(reader, bounded, cell, component, function, entry, permit)
                    .map(|value| Values::Closed([value.center, value.total, value.error]))
                    .map_err(|reason| CellOutcome::CellRefused { component, reason })
            }
            Self::PreparedExpm1(_, operation) => {
                let result = || {
                    let center = reader
                        .ok_or(Expm1Failure::MalformedPlan)?
                        .read_prepaid(HostChannel::Center, cell, component, permit)
                        .map_err(|_| Expm1Failure::MalformedPlan)?
                        .to_f64();
                    let law = law.ok_or(Expm1Failure::MalformedPlan)?;
                    #[cfg(test)]
                    crate::loan::fault::entered_scalar();
                    let value = law
                        .evaluate(
                            Expm1Input {
                                endpoints: [center; 2],
                                center,
                                bits: operation.bits,
                            },
                            &permit.context,
                        )?
                        .reading();
                    Ok(Values::Prepared([
                        value.center,
                        value.total_radius,
                        value.numerical_error,
                        value.endpoints[0],
                        value.endpoints[1],
                    ]))
                };
                result().map_err(|reason| CellOutcome::PreparedRefused { component, reason })
            }
        }
    }
}
/// Closed primitives retain their exact input-radius and original-location overflow contract.
fn closed<P: Scalar + 'static>(
    reader: Option<&crate::TileReader<'_, P>>,
    bounded: bool,
    cell: usize,
    component: usize,
    function: crate::Elementwise,
    entry: usize,
    permit: &Traversal,
) -> Result<crate::kernels::arithmetic::Entry<P>, crate::KernelError> {
    let reader = reader.ok_or(crate::KernelError::Contract)?;
    let center = reader
        .read_prepaid(HostChannel::Center, cell, component, permit)
        .map_err(|_| crate::KernelError::Contract)?;
    let total = if bounded {
        reader
            .read_prepaid(HostChannel::Total, cell, component, permit)
            .map_err(|_| crate::KernelError::Contract)?
    } else {
        P::ZERO
    };
    crate::kernels::nonlinear::evaluate_selected(
        center,
        nrz_accuracy::Nonnegative::new(total)?,
        function,
        entry,
        &permit.context,
    )
}
impl<P: Scalar + 'static> Values<P> {
    /// All member roles share the same write/tracking implementation.
    pub(super) fn store(
        self,
        channels: &mut [Option<LoanChannel>; 3],
        endpoints: &mut Option<[LoanChannel; 2]>,
        entry: usize,
    ) -> bool {
        let owners = channels
            .iter_mut()
            .flatten()
            .chain(endpoints.iter_mut().flatten());
        match self {
            Self::Closed(values) => store(owners, values, entry),
            Self::Prepared(values) => store(owners, values, entry),
        }
    }
}
/// Continue storing after a failure; initialized placeholders never imply completed evidence.
fn store<'a, P: Scalar + 'static>(
    owners: impl Iterator<Item = &'a mut LoanChannel>,
    values: impl IntoIterator<Item = P>,
    entry: usize,
) -> bool {
    let mut failed = false;
    for (channel, value) in owners.zip(values) {
        let result: Result<(), LoanError> = channel.matrix.write(entry, value);
        failed |= result.is_err();
        set(&mut channel.components, entry, true);
    }
    failed
}
