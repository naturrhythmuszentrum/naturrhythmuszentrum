// SPDX-License-Identifier: AGPL-3.0-only
//! Descriptor-backed quantities reuse the heap's selected bindings and complete transactions.

mod arithmetic;
mod conversion;
mod counts;
mod execution;
mod imports;
mod lineage;
mod roles;
mod shapes;
pub use counts::{CountArray, CountError, CountHistory, CountQuantity, ExactCountRatio};

pub use conversion::NormalizedQuantity;
pub use imports::QuantityInput;
pub use roles::{DifferenceRole, DynamicRole, PointRole, QuantityRole};

use crate::{ExecutionLimits, Heap, HeapError, ShapeLimits, ValueBinding, ValueView};
use nrz_bindings::{Budget, CheckedDeclarations, Descriptor, QuantitySymbol};
use nrz_scalar::Scalar;
use std::{marker::PhantomData, rc::Rc};

/// A selected quantity, retaining its existing descriptor, versions and companion bindings.
///
/// No numerical payload is copied. Access always revalidates the selected binding.
#[derive(Clone, Debug)]
pub struct Quantity<P: Scalar, R: QuantityRole = DynamicRole> {
    /// One shared instance of the existing complete selected binding.
    binding: Rc<ValueBinding<P>>,
    /// A sealed role can be obtained only after descriptor validation.
    role: PhantomData<R>,
    /// Original exact count ratios contributing to this numerical quantity.
    counts: Option<Rc<[ExactCountRatio]>>,
}

impl<P: Scalar, R: QuantityRole> Quantity<P, R> {
    /// Original checked descriptor, borrowed from bindings rather than duplicated.
    #[must_use]
    pub fn descriptor(&self) -> &Descriptor {
        self.binding.symbol().descriptor()
    }

    /// Original exact count ratios, retained through arithmetic and coordinate conversions.
    ///
    /// Entries are source evidence, not claims that a transformed result equals each ratio.
    #[must_use]
    pub fn count_ratio_sources(&self) -> &[ExactCountRatio] {
        self.counts.as_deref().unwrap_or(&[])
    }

    /// Original checked symbol, including its immutable graph identity.
    #[must_use]
    pub fn symbol(&self) -> &QuantitySymbol {
        self.binding.symbol()
    }

    /// Existing selected binding with its independently declared companions.
    #[must_use]
    pub fn binding(&self) -> &ValueBinding<P> {
        &self.binding
    }

    /// Erase only the optional static role marker, retaining the same binding.
    #[must_use]
    pub fn dynamic(&self) -> Quantity<P> {
        Quantity {
            binding: Rc::clone(&self.binding),
            counts: self.counts.clone(),
            role: PhantomData,
        }
    }
}

impl<P: Scalar + 'static, R: QuantityRole> Quantity<P, R> {
    /// Revalidate the exact symbol and every captured numerical version before access.
    /// # Errors
    /// Refuses foreign, stale, pending or incomplete value/companion bindings.
    pub fn view<'h>(&self, heap: &'h Heap) -> Result<ValueView<'h, P>, HeapError> {
        heap.value_by_symbol(self.symbol(), &self.binding)
    }
}

/// Frozen descriptor authority and existing shape/execution policies for quantity operations.
#[derive(Clone, Debug)]
pub struct QuantityContext {
    /// Exact immutable declarations and registry; no independent unit algebra is stored.
    declarations: Rc<CheckedDeclarations>,
    /// Existing descriptor and logical-map resource ceilings.
    shapes: ShapeLimits,
    /// Existing numerical scan, kernel and scratch ceilings.
    execution: ExecutionLimits,
}

impl QuantityContext {
    /// Retain a checked declaration owner and explicit existing numerical policies.
    #[must_use]
    pub const fn new(
        declarations: Rc<CheckedDeclarations>,
        shapes: ShapeLimits,
        execution: ExecutionLimits,
    ) -> Self {
        Self {
            declarations,
            shapes,
            execution,
        }
    }

    /// The retained exact rule snapshot used for each operation's admission.
    #[must_use]
    pub fn declarations(&self) -> &CheckedDeclarations {
        &self.declarations
    }

    /// Adopt an already checked selected binding without copying any numerical payload.
    /// # Errors
    /// Refuses foreign/stale bindings, counts and exhausted caller Work or metadata ceilings.
    pub fn capture<P: Scalar + 'static>(
        &self,
        heap: &Heap,
        binding: ValueBinding<P>,
        budget: &mut Budget,
    ) -> Result<Quantity<P>, QuantityError<P>> {
        charge(budget, 1)?;
        self.check_descriptor(binding.symbol().descriptor())?;
        heap.value_by_symbol(binding.symbol(), &binding)?;
        allocation::<P>(budget)?;
        Ok(Quantity {
            binding: Rc::new(binding),
            counts: None,
            role: PhantomData,
        })
    }

    /// Delegate registry admission and reject discrete values before numerical storage access.
    fn check_descriptor(&self, descriptor: &Descriptor) -> Result<(), crate::OperationError> {
        if descriptor.kind().count_domain().is_some() {
            return Err(HeapError::CountStorage.into());
        }
        self.declarations
            .conversion_map(descriptor, descriptor)
            .map_err(crate::ConversionError::from)?;
        Ok(())
    }
}

/// Quantity admission or execution refusal; numerical failures retain their original record.
#[derive(Debug, thiserror::Error)]
pub enum QuantityError<P: Scalar> {
    /// Exact dimensions, kinds, roles or count domain refused.
    #[error("quantity descriptor: {0}")]
    Units(#[from] nrz_bindings::UnitError),
    /// The selected heap binding no longer satisfies its checked contract.
    #[error("quantity storage: {0}")]
    Heap(#[from] HeapError),
    /// Caller Work or payload policy refused before publication.
    #[error("quantity resources: {0}")]
    Resources(#[from] nrz_bindings::readings::algebra::Fault),
    /// An existing matrix operation refused its semantic or numerical contract.
    #[error("quantity operation: {0}")]
    Operation(Box<crate::OperationError>),
    /// An evaluated or published candidate failed without exposing any result prefix.
    #[error("quantity execution: {0}")]
    Execution(#[from] crate::Failure<P>),
    /// The existing scalar literal authority refused numerical representation.
    #[error("quantity numerical representation: {0}")]
    Numerical(#[from] nrz_scalar::ArithmeticFailure),
    /// Arithmetic requires explicit conversion to common coordinate scales and origins.
    #[error("quantity arithmetic requires a common measure")]
    CommonMeasure,
}

impl<P: Scalar> From<crate::OperationError> for QuantityError<P> {
    fn from(fault: crate::OperationError) -> Self {
        Self::Operation(Box::new(fault))
    }
}

/// Charge the caller's monotone ledger before performing adapter work.
fn charge(budget: &mut Budget, work: usize) -> Result<(), nrz_bindings::readings::algebra::Fault> {
    use nrz_bindings::readings::algebra::{Location, limits::BudgetKind};
    let at = Location::new("quantity adapter");
    let work = u64::try_from(work).map_err(|_| {
        nrz_bindings::readings::algebra::Fault::exhausted(
            BudgetKind::Work,
            budget.limits().work,
            u64::MAX,
            at,
        )
    })?;
    budget.work(work, at)
}

/// Admit only the adapter's one shared binding owner; numerical storage has its own limits.
fn allocation<P: Scalar>(budget: &Budget) -> Result<(), nrz_bindings::readings::algebra::Fault> {
    use nrz_bindings::readings::algebra::{Location, limits::BudgetKind};
    let bytes =
        u64::try_from(size_of::<ValueBinding<P>>() + 2 * size_of::<usize>()).unwrap_or(u64::MAX);
    for kind in [BudgetKind::StagedPayloadBytes, BudgetKind::OutputBytes] {
        budget.check(kind, bytes, Location::new("quantity binding owner"))?;
    }
    Ok(())
}
