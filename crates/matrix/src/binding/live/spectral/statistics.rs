// SPDX-License-Identifier: AGPL-3.0-only
//! One native joint declaration and one evaluated original Jacobian supply root covariance.

mod numeric;
use super::{AT, Heap, HeapError, LcSpectrum, SpectralError as Error};
use crate::{CompanionRef, JointLimits, ModelId, ReciprocalDerivativeEvaluation, SourceId};
use nrz_bindings::{
    Budget,
    readings::{
        LcRootJacobian,
        algebra::limits::{BudgetKind, BudgetUsage},
    },
};
use nrz_scalar::{ArithmeticContext, DecodedLiteral, Enclosure, Scalar};
use std::rc::Rc;

/// Statistical state occupies a separate allocation, absent from deterministic plans and results.
#[derive(Debug)]
pub(super) struct Extension<P: Scalar> {
    /// Original deterministic plan, retained directly without extension chains.
    pub(super) base: LcSpectrum<P>,
    /// Immutable original statistical input shared by all compatible evaluations.
    pub(super) input: Rc<Input<P>>,
    /// Evaluated root parameters exist only on a successfully completed statistical result.
    pub(super) moments: Option<Moments<P>>,
}

/// Immutable statistical input retains the original derivative, selected targets and model identity.
#[derive(Debug)]
pub(super) struct Input<P: Scalar> {
    /// Complete checked root derivative with the shared original operator proof.
    pub(super) exact: LcRootJacobian,
    /// Evaluated original coefficient partials and every original value/companion version.
    derivative: ReciprocalDerivativeEvaluation<P>,
    /// Original heap-owned joint declaration, revalidated on every result access.
    companion: CompanionRef<P>,
    /// Original model identity, never inferred from equal numerical covariance.
    pub(super) model: ModelId,
    /// Original source identities in physical L/C order.
    pub(super) sources: [SourceId; 2],
    /// Selected covariance coordinates in the original joint model.
    columns: [usize; 2],
    /// Complete new and retained derivative allowance for this statistical input.
    pub(super) bytes: u64,
    /// Complete caller snapshots around statistical input admission.
    pub(super) usage: [BudgetUsage; 2],
}

/// Compact positive-root derivative and variance; signed blocks are exact views of this owner.
#[derive(Clone, Copy, Debug)]
pub(super) struct Moments<P: Scalar> {
    /// Physical positive-root partials in original L/C order.
    pub(super) partials: [Enclosure<P>; 2],
    /// Covariance of the local linear model with its numerical parameter enclosure.
    pub(super) variance: Enclosure<P>,
}

impl<P: Scalar + 'static> LcSpectrum<P> {
    /// Attach the original evaluated Jacobian and its complete native L/C joint declaration.
    ///
    /// Both root signs retain `LinearizedMoments` assurance. Statistical parameter enclosures
    /// never change the deterministic complete-root endpoints or their coverage scope.
    /// # Errors
    /// Refuses foreign models, substituted selections or references, missing covariance blocks,
    /// omitted derivative directions, stale original versions and exhausted caller resources.
    pub fn with_reciprocal_statistics(
        self,
        heap: &Heap,
        derivative: ReciprocalDerivativeEvaluation<P>,
        companion: CompanionRef<P>,
        budget: &mut Budget,
    ) -> Result<Self, Error> {
        let before = budget.usage();
        self.validate(heap)?;
        if self.extension().is_some() {
            return Err(Error::Identity);
        }
        let context = ArithmeticContext::new()?;
        derivative.values(heap, &context, budget)?;
        selected(self.inputs(), derivative.inputs())?;
        let (model, sources, columns) = declaration(heap, self.inputs(), companion, budget)?;
        let exact = LcRootJacobian::new(
            self.law().clone(),
            derivative.contract().exact().clone(),
            budget,
        )?;
        let bytes = storage(&derivative, &exact, self.retained_payload_bytes())?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        let input = Input {
            exact,
            derivative,
            companion,
            model,
            sources,
            columns,
            bytes,
            usage: [before, budget.usage()],
        };
        Ok(Self::extended(Extension {
            base: self,
            input: Rc::new(input),
            moments: None,
        }))
    }
    /// Shared exact root adapter, absent for a deterministic-only spectrum.
    #[must_use]
    pub fn root_jacobian(&self) -> Option<&LcRootJacobian> {
        self.extension().map(|extension| &extension.input.exact)
    }
    /// Retained statistical model and ordered original L/C source identities.
    #[must_use]
    pub fn statistical_identity(&self) -> Option<(ModelId, [SourceId; 2])> {
        self.extension()
            .map(|extension| (extension.input.model, extension.input.sources))
    }
    /// Cumulative statistical admission snapshots, independent of deterministic plan preparation.
    #[must_use]
    pub fn statistical_usage(&self) -> Option<[BudgetUsage; 2]> {
        self.extension().map(|extension| extension.input.usage)
    }
    /// Complete statistical input allowance, including its shared original derivative owner.
    #[must_use]
    pub fn statistical_payload_bytes(&self) -> Option<u64> {
        self.extension().map(|extension| extension.input.bytes)
    }
}

/// Selected identity includes the original group allocation and exact logical reading index.
fn selected<P: Scalar>(
    expected: &[crate::ReadingBinding<P>; 2],
    actual: &[crate::ReadingBinding<P>],
) -> Result<(), Error> {
    if actual.len() != expected.len() {
        return Err(Error::Identity);
    }
    for (left, right) in expected.iter().zip(actual) {
        if !Rc::ptr_eq(&left.group, &right.group) || left.index != right.index {
            return Err(Error::Identity);
        }
    }
    Ok(())
}

/// Exactly two native scalar target sides retain their original covariance source selections.
fn declaration<P: Scalar + 'static>(
    heap: &Heap,
    inputs: &[crate::ReadingBinding<P>; 2],
    companion: CompanionRef<P>,
    budget: &mut Budget,
) -> Result<(ModelId, [SourceId; 2], [usize; 2]), Error> {
    let view = heap.statistical(companion)?;
    let record = heap
        .joint
        .records
        .get(companion.index)
        .ok_or(Error::Identity)?;
    let mut columns = [0; 2];
    for (slot, (input, target)) in inputs.iter().zip(&record.targets).enumerate() {
        budget.work(1, AT)?;
        let actual = heap.symbols.selection(target.key).ok_or(Error::Identity)?;
        if !Rc::ptr_eq(&input.group, &actual.group) || input.index != actual.index {
            return Err(Error::Identity);
        }
        let [column] = view.source_maps()[slot].as_slice() else {
            return Err(Error::Identity);
        };
        columns[slot] = *column;
    }
    budget.work(4, AT)?;
    view.model()
        .require_blocks(&columns, &columns, JointLimits::new(2, 4, 4))?;
    let sources = columns.map(|column| view.model().sources()[column].id());
    Ok((view.model().id(), sources, columns))
}

/// Existing exact derivative retention and the new input/plan records coexist during admission.
fn storage<P: Scalar + 'static>(
    derivative: &ReciprocalDerivativeEvaluation<P>,
    exact: &LcRootJacobian,
    plan: u64,
) -> Result<u64, Error> {
    let metadata =
        u64::try_from(size_of::<Input<P>>() + 2 * size_of::<usize>() + extension_bytes::<P>())
            .map_err(|_| HeapError::ExtentOverflow)?;
    derivative
        .native_payload_bytes()?
        .checked_add(exact.retained_payload_bytes())
        .and_then(|bytes| bytes.checked_add(metadata))
        .and_then(|bytes| bytes.checked_add(plan))
        .ok_or_else(|| HeapError::ExtentOverflow.into())
}

/// The immutable companion and original selected source versions are validated together.
pub(super) fn validate<P: Scalar + 'static>(heap: &Heap, input: &Input<P>) -> Result<(), Error> {
    let view = heap.statistical(input.companion)?;
    if view.model().id() != input.model {
        return Err(Error::Identity);
    }
    Ok(())
}

/// No statistical input produces no statistical result; every numerical owner shares one ledger.
pub(super) fn evaluate<P: Scalar + 'static>(
    heap: &Heap,
    plan: &LcSpectrum<P>,
    constants: &[DecodedLiteral<P>; 3],
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Option<Moments<P>>, Error> {
    plan.extension()
        .map(|extension| {
            let input = &extension.input;
            budget.with_retained_payload(input.bytes, AT, |budget| {
                numeric::evaluate(heap, input, constants, context, budget)
            })
        })
        .transpose()
}

/// New statistical plan record and boxed extension coexist; deterministic requests own neither.
const fn extension_bytes<P: Scalar>() -> usize {
    size_of::<super::plan::Plan<P>>() + 2 * size_of::<usize>() + size_of::<Extension<P>>()
}

/// Statistical output alone owns another plan record and boxed parameter extension.
pub(super) fn output_bytes<P: Scalar>(plan: &LcSpectrum<P>) -> Result<u64, Error> {
    let extra = if plan.extension().is_some() {
        extension_bytes::<P>()
    } else {
        0
    };
    u64::try_from(extra).map_err(|_| HeapError::ExtentOverflow.into())
}

/// Completed deterministic evaluations retain their original exact staged/output byte count.
pub(super) fn evaluation_bytes<P: Scalar>(plan: &LcSpectrum<P>) -> Result<u64, Error> {
    let direct = u64::try_from(size_of::<super::LcSpectralEvaluation<P>>())
        .map_err(|_| HeapError::ExtentOverflow)?;
    direct
        .checked_add(output_bytes(plan)?)
        .ok_or_else(|| HeapError::ExtentOverflow.into())
}

/// Only statistical evaluations allocate a new owner for their immutable root parameters.
pub(super) fn evaluated<P: Scalar>(
    plan: &LcSpectrum<P>,
    moments: Option<Moments<P>>,
) -> LcSpectrum<P> {
    plan.extension().map_or_else(
        || plan.clone(),
        |extension| {
            LcSpectrum::extended(Extension {
                base: extension.base.clone(),
                input: Rc::clone(&extension.input),
                moments,
            })
        },
    )
}
