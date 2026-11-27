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
    /// Checked scalar affine prelude over native cell incidence.
    ProjectedAffine(&'a Rc<KernelContract>),
    /// Relative endpoint target with its exact retained original correspondence.
    PreparedExpm1(&'a Rc<KernelContract>, &'a PreparedExpm1Operation),
    /// Dyadic schedule with its full captured owner and use binding.
    PreparedDyadicPower(
        &'a Rc<KernelContract>,
        &'a crate::loan::retained_dyadic_power::Binding,
    ),
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
            (Recipe::ProjectedAffine(..), None) => Ok(Self::ProjectedAffine(law)),
            (Recipe::Transform(_), None) => Ok(Self::Transform(law)),
            (Recipe::Elementwise(Elementwise::Square), None) => Ok(Self::Square(law)),
            (Recipe::Elementwise(Elementwise::Reciprocal), None) => Ok(Self::Reciprocal(law)),
            (Recipe::Elementwise(Elementwise::SquareRoot), None) => Ok(Self::SquareRoot(law)),
            (
                Recipe::BinaryElementwise(function) | Recipe::CartesianBinary(_, _, function),
                None,
            ) => Ok(Self::Binary(law, *function)),
            (Recipe::PreparedExpm1, Some(operation)) if law.work == 0 => {
                Ok(Self::PreparedExpm1(law, operation))
            }
            _ => Err(LoanError::UnsupportedContract),
        }
    }
    /// Authenticate a published retained alternative against its closed recipe.
    pub(super) fn check_evidence(
        operation: &'a CheckedOperation,
        expected: KernelKind,
        prepared: Option<&'a super::PreparedCorrespondence>,
    ) -> Result<Self, LoanError> {
        match prepared {
            Some(super::PreparedCorrespondence::Expm1(original)) => {
                Self::check(operation, expected, Some(original))
            }
            Some(super::PreparedCorrespondence::DyadicPower(binding)) => {
                Self::check_dyadic(operation, expected, binding)
            }
            None => Self::check(operation, expected, None),
        }
    }
    /// Authenticate the original dyadic recipe and its exact sealed invocation allowance.
    pub(super) fn check_dyadic(
        operation: &'a CheckedOperation,
        expected: KernelKind,
        binding: &'a crate::loan::retained_dyadic_power::Binding,
    ) -> Result<Self, LoanError> {
        if expected != KernelKind::PreparedDyadicPower
            || operation.inputs().len() != 1
            || operation.outputs().len() != 1
            || operation.revision() != 1
        {
            return Err(LoanError::UnsupportedContract);
        }
        let crate::binding::NativeRecipe::Kernel(law) = &operation.recipe else {
            return Err(LoanError::UnsupportedContract);
        };
        if !matches!(law.recipe, Recipe::PreparedDyadicPower)
            || law.kind() != expected
            || law.inputs.len() != 1
        {
            return Err(LoanError::UnsupportedContract);
        }
        let units = super::dyadic_power::invocation_units(binding.schedule_inventory()?)?;
        let work = law
            .output
            .entries()
            .checked_mul(units)
            .ok_or(LoanError::CountOverflow)?;
        if law.work != work {
            return Err(LoanError::WrongLoan);
        }
        Ok(Self::PreparedDyadicPower(law, binding))
    }
    /// Retain the original pointer seal, not merely a supported name.
    pub(super) const fn law(self) -> &'a Rc<KernelContract> {
        match self {
            Self::Transform(law)
            | Self::ProjectedAffine(law)
            | Self::ProjectedElementwise(law, _)
            | Self::Square(law)
            | Self::Reciprocal(law)
            | Self::SquareRoot(law)
            | Self::PreparedExpm1(law, _)
            | Self::PreparedDyadicPower(law, _)
            | Self::Binary(law, _) => law,
        }
    }
    /// Fixed output roles are inseparable from the law and its reading semantics.
    pub(super) const fn roles(self) -> usize {
        match self {
            Self::Transform(_)
            | Self::ProjectedAffine(_)
            | Self::ProjectedElementwise(..)
            | Self::Square(_)
            | Self::Reciprocal(_)
            | Self::SquareRoot(_)
            | Self::Binary(..) => 3,
            Self::PreparedExpm1(..) | Self::PreparedDyadicPower(..) => 5,
        }
    }
    /// Original cancellation inventory; the shared records never grow for dispatch.
    pub(super) const fn debits(self) -> usize {
        match self {
            Self::Transform(_)
            | Self::ProjectedAffine(_)
            | Self::ProjectedElementwise(..)
            | Self::Square(_)
            | Self::Reciprocal(_)
            | Self::SquareRoot(_) => 17,
            Self::PreparedExpm1(..) | Self::PreparedDyadicPower(..) => 23,
            Self::Binary(..) => 18,
        }
    }
    /// Both retained members have endpoint owners and ledger-aware staged admission.
    pub(super) const fn is_prepared(self) -> bool {
        self.has_retained_law()
    }
    /// Both retained members require the separately reserved common attachment shell.
    pub(super) const fn has_retained_law(self) -> bool {
        matches!(
            self,
            Self::PreparedExpm1(..) | Self::PreparedDyadicPower(..)
        )
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
        matches!(
            self,
            Self::Transform(_) | Self::ProjectedAffine(_) | Self::ProjectedElementwise(..)
        )
    }
    /// Publication records whether this member preserves input error or reports local arithmetic.
    pub(super) const fn error_scope(
        self,
        input: super::NumericalErrorScope,
    ) -> super::NumericalErrorScope {
        if matches!(self, Self::ProjectedAffine(_)) {
            match input {
                super::NumericalErrorScope::HostInput
                | super::NumericalErrorScope::PropagatedInput => {
                    super::NumericalErrorScope::PropagatedInput
                }
                super::NumericalErrorScope::LocalOperation
                | super::NumericalErrorScope::PropagatedLocal => {
                    super::NumericalErrorScope::PropagatedLocal
                }
            }
        } else if self.is_transform() {
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
            Recipe::ProjectedAffine(affine) => Ok(&affine.map),
            _ => Err(LoanError::UnsupportedContract),
        }
    }
    /// Closed arity is a member property, never inferred from a caller's input count.
    pub(super) const fn arity(self) -> usize {
        if self.is_binary() { 2 } else { 1 }
    }
    /// The closed elementwise law retains its original conservative factor.
    pub(super) fn invocation_factor(self) -> Result<usize, LoanError> {
        match self {
            Self::ProjectedElementwise(..)
            | Self::Square(_)
            | Self::Reciprocal(_)
            | Self::SquareRoot(_) => Ok(12),
            Self::ProjectedAffine(_) => Ok(20),
            Self::Transform(_) => Ok(6),
            Self::PreparedExpm1(..) => Ok(0),
            Self::PreparedDyadicPower(_, binding) => {
                super::dyadic_power::invocation_units(binding.schedule_inventory()?)
            }
            Self::Binary(..) => Ok(21),
        }
    }
    /// Cartesian geometry retains both ordered factor axes inside the original law.
    pub(super) fn cartesian(self) -> Option<crate::CellProduct> {
        match &self.law().recipe {
            Recipe::CartesianBinary(product, _, _) => Some(*product),
            _ => None,
        }
    }
    /// Native reuse reports its exact ledger delta independently of prepaid wrapper visits.
    pub(super) fn native_phase(self) -> Option<&'static str> {
        if self.cartesian().is_some() {
            return Some("Cartesian binary traversal");
        }
        match self {
            Self::Square(_) => None,
            Self::ProjectedAffine(_) => Some("projected affine traversal"),
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
            Self::PreparedDyadicPower(..) => Some("dyadic power native traversal"),
            Self::Reciprocal(_) => Some("reciprocal native traversal"),
            Self::SquareRoot(_) => Some("square root native traversal"),
            Self::Binary(law, crate::BinaryElementwise::Product) => {
                Some(if law.inputs.iter().all(|input| *input == law.output) {
                    "product native traversal"
                } else {
                    "product tile scalar traversal"
                })
            }
            Self::Binary(law, crate::BinaryElementwise::Quotient) => {
                Some(if law.inputs.iter().all(|input| *input == law.output) {
                    "quotient native traversal"
                } else {
                    "quotient tile scalar traversal"
                })
            }
        }
    }
    /// Native invocation allowance is checked separately at the original and one-cell shapes.
    pub(super) fn invocation(self, components: usize) -> Result<usize, LoanError> {
        components
            .checked_mul(self.invocation_factor()?)
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
            Self::Transform(_)
            | Self::ProjectedAffine(_)
            | Self::ProjectedElementwise(..)
            | Self::Square(_) => Ok(()),
            Self::Binary(..) => {
                if source.coverage != crate::Coverage::RepresentedInputs
                    || coverage != crate::Coverage::RepresentedInputs
                {
                    return Err(LoanError::CoverageMismatch);
                }
                Ok(())
            }
            Self::Reciprocal(_) | Self::SquareRoot(_) => {
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
            Self::PreparedDyadicPower(..) => {
                if format != crate::storage::Format::Binary64
                    || source.symbols.bound().is_some()
                    || source.symbols.error().is_some()
                {
                    return Err(LoanError::UnsupportedContract);
                }
                if source.coverage != crate::Coverage::RepresentedInputs
                    || coverage != crate::Coverage::RepresentedInputs
                {
                    return Err(LoanError::CoverageMismatch);
                }
                if source.axis != axis {
                    return Err(LoanError::WrongLoan);
                }
                super::prepared::symbols(&source.symbols, destination, source.extent)
            }
        }
    }
    /// Storage failures preserve the member's lossless public outcome family.
    pub(super) const fn storage_failure(self, component: usize) -> CellOutcome {
        match self {
            Self::Transform(_)
            | Self::ProjectedAffine(_)
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
            Self::PreparedDyadicPower(..) => {
                super::dyadic_refusal(component, nrz_scalar::DyadicPowerFailure::MalformedPlan)
            }
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
                if !matches!(self, Self::PreparedExpm1(..)) || *component >= extent.columns() {
                    return Err(LoanError::WrongLoan);
                }
            }
            CellOutcome::DyadicPowerOutsideDomain { component }
            | CellOutcome::DyadicPowerBelowNormalOperand { component }
            | CellOutcome::DyadicPowerNonDyadicExponent { component }
            | CellOutcome::DyadicPowerTargetNotCertifiable { component, .. }
            | CellOutcome::DyadicPowerInventoryLimit { component, .. }
            | CellOutcome::DyadicPowerMalformedPlan { component }
            | CellOutcome::DyadicPowerEnvironment { component }
            | CellOutcome::DyadicPowerEngine { component, .. } => {
                if !matches!(self, Self::PreparedDyadicPower(..)) || *component >= extent.columns()
                {
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

/// LC layout definitions must not widen the common borrowed member dispatch.
/// ´test:unit:lc-origin-common-member-layout´
#[test]
fn lc_origin_common_member_layout() {
    assert_eq!(size_of::<Member<'_>>(), 24);
    assert_eq!(std::mem::align_of::<Member<'_>>(), 8);
}
