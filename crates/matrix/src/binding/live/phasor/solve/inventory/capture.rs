// SPDX-License-Identifier: AGPL-3.0-only
//! Source-copy vectors and complete input blocks retain exact own-target companion bindings.

use super::super::super::{AT, numeric::Values};
use super::PhasorAllocationChannel as Channel;
use super::{Owner, PhasorError, PhasorSolve, Run, Stage};
use crate::{Coverage, Heap, ReadingBinding};
use nrz_bindings::{Budget, CompanionKind, readings::CompanionReadingKind};
use nrz_scalar::Scalar;

/// Determine optional channels from the original checked bindings before any numerical read.
pub(super) fn present<P: Scalar>(solve: &PhasorSolve<P>, stage: Stage) -> bool {
    let Stage::InputCopy {
        operand,
        copy,
        channel,
    } = stage
    else {
        return true;
    };
    if channel == Channel::Center {
        return true;
    }
    let kind = if channel == Channel::Total {
        CompanionKind::DeterministicBound
    } else {
        CompanionKind::NumericalError
    };
    let binding = &solve.operands()[operand].copies()[copy];
    binding.companions().iter().any(|companion| {
        let target = companion
            .targets()
            .iter()
            .any(|target| target.reading() == binding.reading());
        target && companion.companion().kind() == CompanionReadingKind::Radius(kind)
    })
}
impl<P: Scalar + 'static> Run<P> {
    /// Read every actual stored copy with its original selected parameter order and channel.
    pub(crate) fn capture(
        &self,
        heap: &Heap,
        operand: usize,
        budget: &mut Budget,
    ) -> Result<(Owner<P, Values<P>>, Coverage), PhasorError> {
        let source = self
            .0
            .solve
            .operands()
            .get(operand)
            .ok_or(crate::PrivateAllocationError::UnplannedMeaning)?;
        let stage = if operand == 0 {
            Stage::Impedance
        } else {
            Stage::Voltage
        };
        let mut result = self.inline(stage, || {
            Ok(Values {
                centers: [P::ZERO; 4],
                totals: [P::ZERO; 4],
                errors: [P::ZERO; 4],
            })
        })?;
        let mut covered = true;
        for (copy, binding) in source.copies().iter().enumerate() {
            budget.work(4, AT)?;
            let values = self.vector(
                Stage::InputCopy {
                    operand,
                    copy,
                    channel: Channel::Center,
                },
                || Ok(heap.reading_entries(binding)?),
            )?;
            let total = self.parameter(
                heap,
                [operand, copy],
                CompanionKind::DeterministicBound,
                budget,
            )?;
            let error = self
                .parameter(heap, [operand, copy], CompanionKind::NumericalError, budget)?
                .unwrap_or(P::ZERO);
            covered &= total.is_some();
            result.centers[copy] = values[0];
            result.errors[copy] = error;
            let total = total.unwrap_or(P::ZERO);
            result.totals[copy] = if total < error { error } else { total };
        }
        Ok((result, source.whole().coverage(covered)))
    }
    /// The selected parameter vector is ticketed before the existing numerical reader allocates it.
    fn parameter(
        &self,
        heap: &Heap,
        position: [usize; 2],
        kind: CompanionKind,
        budget: &mut Budget,
    ) -> Result<Option<P>, PhasorError> {
        let [operand, copy] = position;
        let binding: &ReadingBinding<P> = &self.0.solve.operands()[operand].copies()[copy];
        for (index, companion) in binding.companions().iter().enumerate() {
            budget.work(1, AT)?;
            let target = companion
                .targets()
                .iter()
                .any(|target| target.reading() == binding.reading());
            if target && companion.companion().kind() == CompanionReadingKind::Radius(kind) {
                let channel = if kind == CompanionKind::DeterministicBound {
                    Channel::Total
                } else {
                    Channel::Error
                };
                let values = self.vector(
                    Stage::InputCopy {
                        operand,
                        copy,
                        channel,
                    },
                    || Ok(heap.reading_companion_entries(binding, index)?),
                )?;
                if values[0] < P::ZERO {
                    return Err(PhasorError::Identity);
                }
                return Ok(Some(values[0]));
            }
        }
        Ok(None)
    }
}
