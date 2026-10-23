// SPDX-License-Identifier: AGPL-3.0-only
//! Original parameter symbols and both covariance targets are fixed before numerical access.

use super::{AT, CompanionExecutionError as Error, Heap, HeapError, ReadingBinding};
use nrz_bindings::readings::algebra::limits::BudgetKind;
use nrz_bindings::{Budget, readings::ParameterErrorReading};
use nrz_scalar::{LiteralLimits, Scalar};
use std::rc::Rc;

/// Predeclared companion execution with explicit source parameters and source numerical errors.
#[derive(Clone, Debug)]
pub struct CompanionExecution<P: Scalar>(Rc<Plan<P>>);

/// Shared immutable metadata; no output-parameter adapter is interpreted as a source.
#[derive(Debug)]
struct Plan<P: Scalar> {
    /// Complete output companion definition and source-error transport law.
    exact: Rc<ParameterErrorReading>,
    /// Original parameter and parameter-error numerical bindings.
    sources: [ReadingBinding<P>; 2],
    /// One radius target or two ordered covariance targets.
    targets: Vec<ReadingBinding<P>>,
    /// Explicit bounded rational coefficient import policy.
    literals: LiteralLimits,
    /// Conservative retained metadata and exact evidence allowance.
    bytes: u64,
}

impl<P: Scalar> CompanionExecution<P> {
    /// Match original source symbols, complete identity readings, units and ordered output targets.
    /// # Errors
    /// Refuses substituted output parameters, selected-copy/order changes, wrong units and limits.
    pub fn new(
        exact: Rc<ParameterErrorReading>,
        sources: [ReadingBinding<P>; 2],
        targets: Vec<ReadingBinding<P>>,
        literals: LiteralLimits,
        budget: &mut Budget,
    ) -> Result<Self, Error> {
        let bytes = storage(&exact, &sources, &targets, budget)?;
        budget.with_retained_payload(bytes, AT, |budget| {
            exact.check_sources(sources[0].reading(), sources[1].reading(), budget)?;
            if targets.len() != exact.target().targets().len() {
                return Err(Error::Identity);
            }
            for (actual, expected) in targets.iter().zip(exact.target().targets()) {
                budget.work(1, AT)?;
                if actual.reading() != expected {
                    return Err(Error::Identity);
                }
            }
            if let Some(law) = exact.target().phasor_law() {
                for (target, equation) in targets.iter().zip(law.equations()) {
                    budget.work(1, AT)?;
                    let actual = target.phasor_solve().ok_or(Error::Identity)?;
                    let operation =
                        actual.correspondence().original() == equation.correspondence().original();
                    let voltage = actual.voltage().reading() == equation.voltage().reading();
                    if !operation || !voltage {
                        return Err(Error::Identity);
                    }
                }
            }
            Ok::<_, Error>(())
        })?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        Ok(Self(Rc::new(Plan {
            exact,
            sources,
            targets,
            literals,
            bytes,
        })))
    }
    /// Output parameter and parameter-error symbols, with their original sources and full maps.
    #[must_use]
    pub fn exact(&self) -> &ParameterErrorReading {
        &self.0.exact
    }
    /// Original parameter then original parameter-error bindings, never output coordinates.
    #[must_use]
    pub fn sources(&self) -> &[ReadingBinding<P>; 2] {
        &self.0.sources
    }
    /// Ordered target bindings, preserving shared family and selected-copy identities.
    #[must_use]
    pub fn targets(&self) -> &[ReadingBinding<P>] {
        &self.0.targets
    }
    /// Retained metadata and exact evidence charged beside numerical execution scratch.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
    /// Internal coefficient import policy.
    pub(super) fn literals(&self) -> LiteralLimits {
        self.0.literals
    }
    /// Internal shared metadata allowance.
    pub(super) fn bytes(&self) -> u64 {
        self.0.bytes
    }
}

impl<P: Scalar + 'static> CompanionExecution<P> {
    /// Every source and target is validated, including the second side of a cross block.
    pub(super) fn validate(&self, heap: &Heap) -> Result<(), Error> {
        for binding in self.sources().iter().chain(self.targets()) {
            heap.validate_reading(binding)?;
        }
        Ok(())
    }
}

/// Reserve retained exact maps and all source/target representation certificates before cloning.
fn storage<P: Scalar>(
    exact: &ParameterErrorReading,
    sources: &[ReadingBinding<P>; 2],
    targets: &[ReadingBinding<P>],
    budget: &mut Budget,
) -> Result<u64, Error> {
    let mut bytes = 1024_u64;
    if let Some(law) = exact.target().phasor_law() {
        bytes = bytes
            .checked_add(law.retained_payload_bytes())
            .ok_or(HeapError::ExtentOverflow)?;
    }
    for map in [exact.target().map(), exact.map()] {
        for value in map.coefficients().iter().chain(map.offsets()) {
            budget.work(1, AT)?;
            let (top, bottom) = value.component_bits();
            let size = top
                .div_ceil(64)
                .saturating_add(bottom.div_ceil(64))
                .saturating_mul(16);
            bytes = bytes
                .checked_add(size)
                .and_then(|n| n.checked_add(256))
                .ok_or(HeapError::ExtentOverflow)?;
        }
    }
    for binding in sources.iter().chain(targets) {
        crate::binding::adapters::reading_work(binding.reading(), budget)
            .map_err(super::LiveBindingError::from)?;
        let size = u64::try_from(
            binding
                .adapter()
                .representation()
                .witness()
                .exported_byte_len(),
        )
        .map_err(|_| HeapError::ExtentOverflow)?;
        let retained = if exact.target().phasor_law().is_some() {
            size
        } else {
            size.checked_mul(8).ok_or(HeapError::ExtentOverflow)?
        };
        bytes = bytes
            .checked_add(retained)
            .and_then(|n| n.checked_add(512))
            .ok_or(HeapError::ExtentOverflow)?;
    }
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    Ok(bytes)
}
