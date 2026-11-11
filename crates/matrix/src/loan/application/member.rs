// SPDX-License-Identifier: AGPL-3.0-only
//! Borrowed closed profiles derive from the persistent original recipe and correspondence.

use super::{CellOutcome, PreparedExpm1Operation};
use crate::loan::{LoanError, registry::Declaration};
use crate::{
    CheckedOperation, Elementwise, KernelKind,
    kernels::{KernelContract, contract::Recipe},
};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// No member owns an allocation, authority, receipt or retaining edge.
#[derive(Clone, Copy)]
pub(super) enum Member<'a> {
    /// Exact enclosure of the represented box in either existing scalar format.
    Square(&'a Rc<KernelContract>),
    /// Relative endpoint target with its exact retained original correspondence.
    PreparedExpm1(&'a Rc<KernelContract>, &'a PreparedExpm1Operation),
    /// Binary64 exact enclosure of either sign of a separated represented box.
    Reciprocal(&'a Rc<KernelContract>),
}
impl<'a> Member<'a> {
    /// Expected identity comes from the entry point or the persistent captured law.
    pub(super) fn check(
        operation: &'a CheckedOperation,
        expected: KernelKind,
        prepared: Option<&'a PreparedExpm1Operation>,
    ) -> Result<Self, LoanError> {
        if operation.inputs().len() != 1
            || operation.outputs().len() != 1
            || operation.revision() != 1
        {
            return Err(LoanError::UnsupportedContract);
        }
        let crate::binding::NativeRecipe::Kernel(law) = &operation.recipe else {
            return Err(LoanError::UnsupportedContract);
        };
        if law.kind() != expected {
            return Err(LoanError::UnsupportedContract);
        }
        match (&law.recipe, prepared) {
            (Recipe::Elementwise(Elementwise::Square), None) => Ok(Self::Square(law)),
            (Recipe::Elementwise(Elementwise::Reciprocal), None) => Ok(Self::Reciprocal(law)),
            (Recipe::PreparedExpm1, Some(operation)) if law.work == 0 => {
                Ok(Self::PreparedExpm1(law, operation))
            }
            _ => Err(LoanError::UnsupportedContract),
        }
    }
    /// Retain the original pointer seal, not merely a supported name.
    pub(super) const fn law(self) -> &'a Rc<KernelContract> {
        match self {
            Self::Square(law) | Self::Reciprocal(law) | Self::PreparedExpm1(law, _) => law,
        }
    }
    /// Fixed output roles are inseparable from the law and its reading semantics.
    pub(super) const fn roles(self) -> usize {
        match self {
            Self::Square(_) | Self::Reciprocal(_) => 3,
            Self::PreparedExpm1(..) => 5,
        }
    }
    /// Original cancellation inventory; the shared records never grow for dispatch.
    pub(super) const fn debits(self) -> usize {
        match self {
            Self::Square(_) | Self::Reciprocal(_) => 17,
            Self::PreparedExpm1(..) => 23,
        }
    }
    /// Only the retained member has endpoint owners and ledger-aware staged admission.
    pub(super) const fn is_prepared(self) -> bool {
        matches!(self, Self::PreparedExpm1(..))
    }
    /// The closed elementwise law retains its original conservative factor.
    pub(super) const fn invocation_factor(self) -> usize {
        match self {
            Self::Square(_) | Self::Reciprocal(_) => 12,
            Self::PreparedExpm1(..) => 0,
        }
    }
    /// Native reuse reports its exact ledger delta independently of prepaid wrapper visits.
    pub(super) const fn native_phase(self) -> Option<&'static str> {
        match self {
            Self::Square(_) => None,
            Self::PreparedExpm1(..) => Some("prepared native traversal"),
            Self::Reciprocal(_) => Some("reciprocal native traversal"),
        }
    }
    /// Native invocation allowance is checked separately at the original and one-cell shapes.
    pub(super) fn invocation(self, components: usize) -> Result<usize, LoanError> {
        components
            .checked_mul(self.invocation_factor())
            .ok_or(LoanError::CountOverflow)
    }
    /// The only three legal one-cell recipes require no coefficient copying or construction.
    pub(super) const fn recipe(self) -> Recipe {
        match self {
            Self::Square(_) => Recipe::Elementwise(Elementwise::Square),
            Self::Reciprocal(_) => Recipe::Elementwise(Elementwise::Reciprocal),
            Self::PreparedExpm1(..) => Recipe::PreparedExpm1,
        }
    }
    /// Original and one-cell identity must agree before numerical entry and publication.
    pub(super) fn validate_cell(
        self,
        law: &KernelContract,
        components: usize,
    ) -> Result<(), LoanError> {
        let extent = crate::Extent::new(1, components)?;
        if law.kind() != self.law().kind()
            || law.inputs.as_ref() != [extent]
            || law.output != extent
            || law.work != self.invocation(components)?
        {
            return Err(LoanError::WrongLoan);
        }
        Ok(())
    }
    /// Admission uses the same precision and representation check as the immutable seal.
    pub(super) fn representation<P: Scalar + 'static>(
        self,
        source: &Declaration,
        destination: &nrz_bindings::BindingSymbols,
        axis: crate::CellAxis,
        coverage: crate::Coverage,
    ) -> Result<(), LoanError> {
        self.representation_format(
            crate::storage::Format::of::<P>(),
            source,
            destination,
            axis,
            coverage,
        )
    }
    /// A member cannot inherit another member's coverage, target or input form.
    pub(super) fn representation_format(
        self,
        format: crate::storage::Format,
        source: &Declaration,
        destination: &nrz_bindings::BindingSymbols,
        axis: crate::CellAxis,
        coverage: crate::Coverage,
    ) -> Result<(), LoanError> {
        match self {
            Self::Square(_) => Ok(()),
            Self::Reciprocal(_) => {
                if format != crate::storage::Format::Binary64 {
                    return Err(LoanError::UnsupportedContract);
                }
                if source.coverage != crate::Coverage::RepresentedInputs
                    || coverage != crate::Coverage::RepresentedInputs
                {
                    return Err(LoanError::CoverageMismatch);
                }
                Ok(())
            }
            Self::PreparedExpm1(_, operation) => {
                if source.symbols.bound().is_some()
                    || source.symbols.error().is_some()
                    || format != crate::storage::Format::Binary64
                {
                    return Err(LoanError::UnsupportedContract);
                }
                operation.validate_destination(source, destination, axis, coverage)
            }
        }
    }
    /// Storage failures preserve the member's lossless public outcome family.
    pub(super) const fn storage_failure(self, component: usize) -> CellOutcome {
        match self {
            Self::Square(_) | Self::Reciprocal(_) => CellOutcome::CellRefused {
                component,
                reason: crate::KernelError::Contract,
            },
            Self::PreparedExpm1(..) => CellOutcome::PreparedRefused {
                component,
                reason: nrz_scalar::Expm1Failure::MalformedPlan,
            },
        }
    }
    /// Numerical domain errors must name this member and the original flat entry.
    pub(super) fn validate_outcome(
        self,
        outcome: &CellOutcome,
        cell: usize,
        extent: crate::Extent,
    ) -> Result<(), LoanError> {
        match outcome {
            CellOutcome::PreparedRefused { component, .. } => {
                if !self.is_prepared() || *component >= extent.columns() {
                    return Err(LoanError::WrongLoan);
                }
            }
            CellOutcome::CellRefused { component, reason } => {
                if self.is_prepared() || *component >= extent.columns() {
                    return Err(LoanError::WrongLoan);
                }
                if let crate::KernelError::Domain {
                    function, entry, ..
                } = reason
                    && (self.law().kind() != KernelKind::Elementwise(*function)
                        || *entry != cell + extent.rows() * component)
                {
                    return Err(LoanError::WrongLoan);
                }
            }
            CellOutcome::NotRequested
            | CellOutcome::MissingInput
            | CellOutcome::Pending
            | CellOutcome::Completed => {}
        }
        Ok(())
    }
}
