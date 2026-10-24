// SPDX-License-Identifier: AGPL-3.0-only
//! Selected affine values and all explicit companion source maps share one live closure.

mod numeric;
mod plan;
pub use plan::ReadingSelection;

use super::{CompanionEvaluation, CompanionExecutionError as Error, ReadingBinding};
use crate::{Coverage, Heap, HeapError};
use nrz_bindings::{Budget, CompanionKind, readings::CompanionReadingKind};
use nrz_scalar::{ArithmeticContext, Scalar};
use std::rc::Rc;

/// A complete read-only affine selection, retaining source parameters and both covariance targets.
#[derive(Clone, Debug)]
pub struct MappedReading<P: Scalar>(Rc<Snapshot<P>>);

/// The value and every selected companion become available together.
#[derive(Debug)]
struct Snapshot<P: Scalar> {
    /// Checked original value and explicit companion execution inventories.
    selection: ReadingSelection<P>,
    /// Numerical value coordinates, total radii and numerical-only radii.
    payload: numeric::Payload<P>,
    /// Original companion maps, separately evaluated from their original parameters.
    companions: Vec<CompanionEvaluation<P>>,
    /// Actual scalar enclosure theorem revision.
    revision: u32,
    /// Physical family assurance stays separate from represented parameter covariance.
    coverage: Coverage,
    /// Complete algorithmic Work charged on the caller's cumulative ledger.
    work: u64,
    /// Owned result arrays and metadata; shared source declarations are excluded.
    bytes: u64,
}

impl<P: Scalar + 'static> MappedReading<P> {
    /// Exact value coordinates in the original reading's declared order.
    /// # Errors
    /// Refuses changed source/companion versions, either covariance target or arithmetic revision.
    pub fn values<'a>(
        &'a self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<&'a [P], Error> {
        self.validate(heap, context)?;
        Ok(&self.0.payload.centers)
    }
    /// Complete deterministic radii, including the selected numerical errors once.
    /// # Errors
    /// Refuses a stale selection or arithmetic revision.
    pub fn totals<'a>(
        &'a self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<&'a [P], Error> {
        self.validate(heap, context)?;
        Ok(&self.0.payload.totals)
    }
    /// Numerical-only radii, including value-map rounding and parameter-map error.
    /// # Errors
    /// Refuses a stale selection or arithmetic revision.
    pub fn errors<'a>(
        &'a self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<&'a [P], Error> {
        self.validate(heap, context)?;
        Ok(&self.0.payload.errors)
    }
    /// Original selected storage, with every repeated copy and discrepancy retained.
    #[must_use]
    pub fn source(&self) -> &ReadingBinding<P> {
        self.0.selection.source()
    }
    /// Original bound/error/covariance definitions and separately enclosed parameter values.
    #[must_use]
    pub fn companions(&self) -> &[CompanionEvaluation<P>] {
        &self.0.companions
    }
    /// Declared deterministic family or represented-input assurance.
    #[must_use]
    pub fn coverage(&self) -> Coverage {
        self.0.coverage
    }
    /// Work added by this complete selection, excluding prior caller work.
    #[must_use]
    pub fn work(&self) -> u64 {
        self.0.work
    }
    /// Owned arrays and metadata, excluding shared exact/source inventories and allocator overhead.
    #[must_use]
    pub fn owned_bytes(&self) -> u64 {
        self.0.bytes
    }
    /// Validate the full selection before any one channel becomes accessible.
    fn validate(&self, heap: &Heap, context: &ArithmeticContext) -> Result<(), Error> {
        if self.0.revision != context.revision() {
            return Err(Error::Identity);
        }
        self.0.selection.validate(heap)?;
        for companion in &self.0.companions {
            companion.values(heap, context)?;
        }
        Ok(())
    }
}

impl Heap {
    /// Execute a selected affine value together with its explicit original companion sources.
    ///
    /// Radius laws use absolute coefficients; covariance retains both exact target maps.
    /// Output-parameter storage is never substituted for original source parameters. The result
    /// is read-only and claims no PSD certificate, independence or nonlinear statistical model.
    /// # Errors
    /// Refuses incomplete inventories, native companions, stale targets, negative radii,
    /// nonfinite arithmetic and cumulative resource limits before returning any result.
    pub fn select_mapped_reading<P: Scalar + 'static>(
        &self,
        selection: &ReadingSelection<P>,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<MappedReading<P>, Error> {
        selection.validate(self)?;
        let start = budget.usage().work;
        let bytes = numeric::owned::<P>(selection)?;
        let retained = selection
            .retained_payload_bytes()
            .checked_add(bytes)
            .ok_or(HeapError::ExtentOverflow)?;
        let (mut payload, companions) =
            budget.with_retained_payload(retained, numeric::AT, |budget| {
                numeric::reserve::<P>(selection, 0, budget)?;
                let payload = numeric::evaluate(self, selection, context, budget)?;
                let mut companions = crate::binding::buffer(selection.companions().len())?;
                for execution in selection.companions() {
                    companions.push(self.evaluate_companion(execution, context, budget)?);
                }
                Ok::<_, Error>((payload, companions))
            })?;
        budget.with_retained_payload(retained, numeric::AT, |budget| {
            numeric::declared(self, selection, &companions, context, budget)?;
            numeric::radii(self, &companions, &mut payload, context, budget)
        })?;
        budget.check(
            nrz_bindings::readings::algebra::limits::BudgetKind::OutputBytes,
            bytes,
            numeric::AT,
        )?;
        let declared = companions.iter().any(|companion| {
            companion.exact().target().kind()
                == CompanionReadingKind::Radius(CompanionKind::DeterministicBound)
        });
        let work = budget
            .usage()
            .work
            .checked_sub(start)
            .ok_or(HeapError::ExtentOverflow)?;
        Ok(MappedReading(Rc::new(Snapshot {
            selection: selection.clone(),
            payload,
            companions,
            revision: context.revision(),
            coverage: selection.source().coverage(declared),
            work,
            bytes,
        })))
    }
}
