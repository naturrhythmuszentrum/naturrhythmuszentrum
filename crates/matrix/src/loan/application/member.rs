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

/// Tile-local invocation inventory; no loan-only case enters the crate-wide recipe enum.
#[derive(Clone)]
pub(in crate::loan) struct CellContract {
    /// Independently reserved one-cell input extents.
    pub(super) inputs: Rc<[crate::Extent]>,
    /// Complete one-cell result extent.
    pub(super) output: crate::Extent,
    /// Closed member identity, authenticated against the captured original law.
    pub(super) kind: KernelKind,
    /// Prepaid native invocation allowance.
    pub(super) work: usize,
}

/// No member owns an allocation, authority, receipt or retaining edge.
#[derive(Clone, Copy)]
pub(super) enum Member<'a> {
    /// Exact mapped movement of all existing value and error channels.
    Transform(&'a Rc<KernelContract>),
    /// Exact enclosure of the represented box in either existing scalar format.
    Square(&'a Rc<KernelContract>),
    /// Native projected unary laws read directly through a tile-bound map.
    ProjectedElementwise(&'a Rc<KernelContract>, crate::Elementwise),
    /// Relative endpoint target with its exact retained original correspondence.
    PreparedExpm1(&'a Rc<KernelContract>, &'a PreparedExpm1Operation),
    /// Binary64 exact enclosure of either sign of a separated represented box.
    Reciprocal(&'a Rc<KernelContract>),
    /// Binary64 exact enclosure of the nonnegative branch, including zero.
    SquareRoot(&'a Rc<KernelContract>),
    /// Ordered product or quotient uses the common three-reading lifecycle.
    Binary(&'a Rc<KernelContract>, crate::BinaryElementwise),
}
impl<'a> Member<'a> {
    /// Expected identity comes from the entry point or the persistent captured law.
    pub(super) fn check(
        operation: &'a CheckedOperation,
        expected: KernelKind,
        prepared: Option<&'a PreparedExpm1Operation>,
    ) -> Result<Self, LoanError> {
        if operation.inputs().len()
            != if matches!(expected, KernelKind::BinaryElementwise(_)) {
                2
            } else {
                1
            }
            || operation.outputs().len() != 1
            || operation.revision() != 1
        {
            return Err(LoanError::UnsupportedContract);
        }
        let crate::binding::NativeRecipe::Kernel(law) = &operation.recipe else {
            return Err(LoanError::UnsupportedContract);
        };
        if law.kind() != expected || law.inputs.len() != operation.inputs().len() {
            return Err(LoanError::UnsupportedContract);
        }
        match (&law.recipe, prepared) {
            (Recipe::ProjectedElementwise(_, function), None) => {
                Ok(Self::ProjectedElementwise(law, *function))
            }
            (Recipe::Transform(_), None) => Ok(Self::Transform(law)),
            (Recipe::Elementwise(Elementwise::Square), None) => Ok(Self::Square(law)),
            (Recipe::Elementwise(Elementwise::Reciprocal), None) => Ok(Self::Reciprocal(law)),
            (Recipe::Elementwise(Elementwise::SquareRoot), None) => Ok(Self::SquareRoot(law)),
            (Recipe::BinaryElementwise(function), None) => Ok(Self::Binary(law, *function)),
            (Recipe::PreparedExpm1, Some(operation)) if law.work == 0 => {
                Ok(Self::PreparedExpm1(law, operation))
            }
            _ => Err(LoanError::UnsupportedContract),
        }
    }
    /// Retain the original pointer seal, not merely a supported name.
    pub(super) const fn law(self) -> &'a Rc<KernelContract> {
        match self {
            Self::Transform(law)
            | Self::ProjectedElementwise(law, _)
            | Self::Square(law)
            | Self::Reciprocal(law)
            | Self::SquareRoot(law)
            | Self::PreparedExpm1(law, _)
            | Self::Binary(law, _) => law,
        }
    }
    /// Fixed output roles are inseparable from the law and its reading semantics.
    pub(super) const fn roles(self) -> usize {
        match self {
            Self::Transform(_)
            | Self::ProjectedElementwise(..)
            | Self::Square(_)
            | Self::Reciprocal(_)
            | Self::SquareRoot(_)
            | Self::Binary(..) => 3,
            Self::PreparedExpm1(..) => 5,
        }
    }
    /// Original cancellation inventory; the shared records never grow for dispatch.
    pub(super) const fn debits(self) -> usize {
        match self {
            Self::Transform(_)
            | Self::ProjectedElementwise(..)
            | Self::Square(_)
            | Self::Reciprocal(_)
            | Self::SquareRoot(_) => 17,
            Self::PreparedExpm1(..) => 23,
            Self::Binary(..) => 18,
        }
    }
    /// Only the retained member has endpoint owners and ledger-aware staged admission.
    pub(super) const fn is_prepared(self) -> bool {
        matches!(self, Self::PreparedExpm1(..))
    }
    /// Binary members require both original slots and the inventoried side owner.
    pub(super) const fn is_binary(self) -> bool {
        matches!(self, Self::Binary(..))
    }
    /// Mapped movement has no arithmetic and preserves the original error channel.
    pub(super) const fn is_transform(self) -> bool {
        matches!(self, Self::Transform(_))
    }
    /// Native maps change cell incidence for either movement or direct arithmetic.
    pub(super) const fn is_projected(self) -> bool {
        matches!(self, Self::Transform(_) | Self::ProjectedElementwise(..))
    }
    /// Publication records whether this member preserves input error or reports local arithmetic.
    pub(super) const fn error_scope(
        self,
        input: super::NumericalErrorScope,
    ) -> super::NumericalErrorScope {
        if self.is_transform() {
            input
        } else {
            super::NumericalErrorScope::LocalOperation
        }
    }
    /// The checked full map remains shared with the original operation throughout execution.
    pub(super) fn map(self) -> Result<&'a crate::EntryMap, LoanError> {
        let law = self.law();
        match &law.recipe {
            Recipe::Transform(map) | Recipe::ProjectedElementwise(map, _) => Ok(map),
            _ => Err(LoanError::UnsupportedContract),
        }
    }
    /// Closed arity is a member property, never inferred from a caller's input count.
    pub(super) const fn arity(self) -> usize {
        if self.is_binary() { 2 } else { 1 }
    }
    /// The closed elementwise law retains its original conservative factor.
    pub(super) const fn invocation_factor(self) -> usize {
        match self {
            Self::ProjectedElementwise(..)
            | Self::Square(_)
            | Self::Reciprocal(_)
            | Self::SquareRoot(_) => 12,
            Self::Transform(_) => 6,
            Self::PreparedExpm1(..) => 0,
            Self::Binary(..) => 21,
        }
    }
    /// Native reuse reports its exact ledger delta independently of prepaid wrapper visits.
    pub(super) fn native_phase(self) -> Option<&'static str> {
        match self {
            Self::Square(_) => None,
            Self::ProjectedElementwise(_, crate::Elementwise::Square) => {
                Some("projected square traversal")
            }
            Self::ProjectedElementwise(_, crate::Elementwise::Reciprocal) => {
                Some("projected reciprocal traversal")
            }
            Self::ProjectedElementwise(_, crate::Elementwise::SquareRoot) => {
                Some("projected square root traversal")
            }
            Self::Transform(_) => Some("mapped companion movement"),
            Self::PreparedExpm1(..) => Some("prepared native traversal"),
            Self::Reciprocal(_) => Some("reciprocal native traversal"),
            Self::SquareRoot(_) => Some("square root native traversal"),
            Self::Binary(law, crate::BinaryElementwise::Product) => {
                Some(if law.inputs[1] == law.output {
                    "product native traversal"
                } else {
                    "product right scalar traversal"
                })
            }
            Self::Binary(law, crate::BinaryElementwise::Quotient) => {
                Some(if law.inputs[1] == law.output {
                    "quotient native traversal"
                } else {
                    "quotient right scalar traversal"
                })
            }
        }
    }
    /// Native invocation allowance is checked separately at the original and one-cell shapes.
    pub(super) fn invocation(self, components: usize) -> Result<usize, LoanError> {
        components
            .checked_mul(self.invocation_factor())
            .ok_or(LoanError::CountOverflow)
    }
    /// Original and one-cell identity must agree before numerical entry and publication.
    pub(super) fn validate_cell(
        self,
        law: &CellContract,
        components: usize,
    ) -> Result<(), LoanError> {
        let extent = crate::Extent::new(1, components)?;
        if law.kind != self.law().kind()
            || law.inputs.len() != self.arity()
            || law.inputs.iter().enumerate().any(|(index, input)| {
                let components = if self.is_binary() {
                    self.law().inputs[index].columns()
                } else {
                    components
                };
                input.rows() != 1 || input.columns() != components
            })
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
            Self::Transform(_) | Self::ProjectedElementwise(..) | Self::Square(_) => Ok(()),
            Self::Reciprocal(_) | Self::SquareRoot(_) | Self::Binary(..) => {
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
            Self::Transform(_)
            | Self::ProjectedElementwise(..)
            | Self::Square(_)
            | Self::Reciprocal(_)
            | Self::SquareRoot(_)
            | Self::Binary(..) => CellOutcome::CellRefused {
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
                if let crate::KernelError::BinaryDomain {
                    function, entry, ..
                } = reason
                    && (self.law().kind() != KernelKind::BinaryElementwise(*function)
                        || *entry != cell + extent.rows() * component)
                {
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
