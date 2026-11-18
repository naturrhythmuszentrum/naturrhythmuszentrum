// SPDX-License-Identifier: AGPL-3.0-only
//! Borrowed schema authority accompanies every owning numerical continuation.

use super::{LcInputShape, LcShapeAxes};
use crate::{
    ApplicationError, ApplicationEvaluation, ApplicationPublication, ApplicationRequest,
    ExecutionAccount, Heap, LoanError, OwnedApplication, PointwiseRequest, PublicationReceipt,
    PublishedTile, TileReader,
};
use nrz_scalar::Scalar;

/// Admission preserves the entire original request on refusal without allocating an error box.
#[derive(Debug)]
pub enum LcShapeRequest<'a, P: Scalar> {
    /// Original question and native continuation are retained together.
    Ready(LcShapeApplication<'a, P>),
    /// No application was admitted; the caller retains its original pin and request.
    Refused(ApplicationError, PointwiseRequest<P>),
}

/// A continuation retains the immutable original schema without an additional heap allocation.
#[derive(Debug)]
pub struct LcShapeApplication<'a, P: Scalar> {
    /// Borrowing prevents the original question and symbol inventory from expiring.
    schema: &'a LcInputShape<P>,
    /// Original native role identities and both ordered products survive this continuation.
    axes: LcShapeAxes,
    /// Absent only after successful publication; failed operations restore the same owner.
    owner: Option<OwnedApplication<P>>,
}

/// A mapped publication and the original question remain one read capability.
#[derive(Debug)]
pub struct LcShapeTile<'a, P: Scalar> {
    /// The same immutable schema used for admission, never reconstructed from dimensions.
    schema: &'a LcInputShape<P>,
    /// Original native role identities and both ordered products survive this continuation.
    axes: LcShapeAxes,
    /// Native pin protects the complete original numerical/mask/map closure.
    tile: PublishedTile<P>,
    /// Qualified filled attachments retain the original mapped publication identity.
    receipt: PublicationReceipt,
}

impl<P: Scalar + 'static> LcInputShape<P> {
    /// Bind the original inventories and complete tuple before native request allocation.
    pub fn request<'a>(
        &'a self,
        heap: &mut Heap,
        axes: &LcShapeAxes,
        request: PointwiseRequest<P>,
        account: &mut ExecutionAccount,
    ) -> LcShapeRequest<'a, P> {
        if let Err(error) = self.check(heap, axes, &request, account) {
            return LcShapeRequest::Refused(error.into(), request);
        }
        match heap.request_mapped_copy(request, account) {
            ApplicationRequest::ApplicationReady(owner) => {
                LcShapeRequest::Ready(LcShapeApplication {
                    schema: self,
                    axes: *axes,
                    owner: Some(owner),
                })
            }
            ApplicationRequest::ApplicationRefused(error, request) => {
                LcShapeRequest::Refused(error, request)
            }
        }
    }
    /// Fixed inventory and role checks precede the existing complete current-occupation gate.
    pub(super) fn check(
        &self,
        heap: &Heap,
        axes: &LcShapeAxes,
        request: &PointwiseRequest<P>,
        account: &mut ExecutionAccount,
    ) -> Result<(), LoanError> {
        let projection = self.axes(axes, account)?;
        account
            .ledger
            .with_work_phase(crate::loan::FAMILY, "LC shape application", |budget| {
                crate::loan::charge(budget, 9)?;
                let source = request
                    .input
                    .record
                    .payload
                    .as_ref()
                    .and_then(|pin| pin.bundle.payload.as_ref())
                    .ok_or(LoanError::WrongLoan)?;
                // These scope checks belong to the original fixed bundle visits.
                if source.declaration.coverage != crate::Coverage::RepresentedInputs
                    || request.destination.coverage != crate::Coverage::RepresentedInputs
                {
                    return Err(LoanError::CoverageMismatch);
                }
                if source.origin.evidence.is_some()
                    || source.endpoints.is_some()
                    || source.retained_law.is_some()
                {
                    return Err(LoanError::UnsupportedContract);
                }
                for (actual, expected) in [
                    (&source.declaration.symbols, &self.symbols[0]),
                    (&request.destination.symbols, &self.symbols[1]),
                ] {
                    if actual.value() != expected.value()
                        || actual.bound() != expected.bound()
                        || actual.error() != expected.error()
                    {
                        return Err(LoanError::TargetMismatch);
                    }
                }
                if source.declaration.axis != axes.instances
                    || request.destination.axis != axes.combined.axis()
                {
                    return Err(LoanError::LayoutMismatch);
                }
                let demand = heap.mask(request.demand, request.demand.layout())?;
                if demand.axis() != axes.combined.axis() || demand.gather().is_some() {
                    return Err(LoanError::LayoutMismatch);
                }
                let crate::binding::NativeRecipe::Kernel(law) = &request.operation.recipe else {
                    return Err(LoanError::UnsupportedContract);
                };
                let crate::kernels::contract::Recipe::Transform(map) = &law.recipe else {
                    return Err(LoanError::UnsupportedContract);
                };
                if map.product() != Some((projection, crate::CellFactor::Outer)) {
                    return Err(LoanError::LayoutMismatch);
                }
                Ok(())
            })
    }
}
impl<'a, P: Scalar + 'static> LcShapeApplication<'a, P> {
    /// Save the original producer locator before abandonment or cancellation.
    /// # Errors
    /// Refuses an already published or cancelled continuation.
    pub const fn ticket(&self) -> Result<crate::LoanTicket, LoanError> {
        match &self.owner {
            Some(owner) => Ok(owner.ticket()),
            None => Err(LoanError::WrongLoan),
        }
    }
    /// Deliver cancellation using the original prepaid entitlement, even at exhausted Work.
    /// # Errors
    /// Authentication refusal restores the same owner for the rightful account.
    pub fn cancel(
        &mut self,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> Result<crate::CancellationReceipt, ApplicationError> {
        let owner = self.owner.take().ok_or(LoanError::WrongLoan)?;
        match heap.cancel_application(owner, account) {
            crate::ApplicationCancellation::ApplicationCancelled(receipt) => Ok(receipt),
            crate::ApplicationCancellation::ApplicationRejected(error, owner) => {
                self.owner = Some(owner);
                Err(error)
            }
        }
    }
    /// Evaluate once using the existing native traversal and per-phase Work accounting.
    /// # Errors
    /// Preserves the unchanged owner on preparation or phase refusal.
    pub fn evaluate(&mut self, account: &mut ExecutionAccount) -> Result<(), ApplicationError> {
        let owner = self.owner.take().ok_or(LoanError::WrongLoan)?;
        match owner.evaluate(account) {
            ApplicationEvaluation::ApplicationCandidate(owner) => {
                self.owner = Some(owner);
                Ok(())
            }
            ApplicationEvaluation::ApplicationNotStarted(error, owner)
            | ApplicationEvaluation::ApplicationRejected(error, owner) => {
                self.owner = Some(owner);
                Err(error)
            }
        }
    }
    /// Publish the mapped intersection while retaining its original question association.
    /// # Errors
    /// Repair and authentication refusal preserve the same evaluated owner for retry.
    pub fn publish(
        &mut self,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> Result<LcShapeTile<'a, P>, ApplicationError> {
        let owner = self.owner.take().ok_or(LoanError::WrongLoan)?;
        match heap.publish_application(owner, account) {
            ApplicationPublication::ApplicationPublished(tile, receipt) => Ok(LcShapeTile {
                schema: self.schema,
                axes: self.axes,
                tile,
                receipt,
            }),
            ApplicationPublication::ApplicationRepair(error, owner)
            | ApplicationPublication::ApplicationRejected(error, owner) => {
                self.owner = Some(owner);
                Err(error)
            }
        }
    }
}
impl<P: Scalar + 'static> LcShapeTile<'_, P> {
    /// Complete-inventory admission borrows the heap through every current native reader.
    pub(super) fn current_reader<'a>(
        &'a self,
        heap: &'a Heap,
        account: &mut ExecutionAccount,
    ) -> Result<TileReader<'a, P>, LoanError> {
        self.tile.reader(heap, account)
    }

    /// Borrow the exact private pin; the bridge cannot synthesize a ticket or clone an edge.
    pub(in crate::loan::lc_shape) const fn pin(&self) -> &crate::BundlePin<P> {
        self.tile.pin()
    }

    /// Admission on an already reserved original ledger preserves the public reader tariff.
    pub(in crate::loan::lc_shape) fn current_reader_on<'a>(
        &'a self,
        heap: &'a Heap,
        account: u64,
        budget: &mut nrz_bindings::Budget,
    ) -> Result<TileReader<'a, P>, LoanError> {
        self.tile.reader_on(heap, account, budget)
    }

    /// Original schema retained by this publication and every independent outside pin.
    #[must_use]
    pub const fn schema(&self) -> &LcInputShape<P> {
        self.schema
    }
    /// Exact original native tuple roles; capacities do not reconstruct these identities.
    #[must_use]
    pub const fn axes(&self) -> LcShapeAxes {
        self.axes
    }
    /// Original qualified publication, including its completed tuple mask.
    #[must_use]
    pub const fn receipt(&self) -> PublicationReceipt {
        self.receipt
    }
    /// Read the protected immutable closure independently of current semantic availability.
    /// # Errors
    /// Refuses foreign accounts, invalid retained authority and exhausted original Work.
    pub fn reader(&self, account: &mut ExecutionAccount) -> Result<TileReader<'_, P>, LoanError> {
        self.tile.pin().historical_reader(account)
    }
    /// Acquire one ordinarily charged outside pin while retaining the same original question.
    /// # Errors
    /// Refuses stale/foreign authority, insufficient pin resources or exhausted Work.
    pub fn retain(
        &self,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> Result<Self, LoanError> {
        let pin = heap.retain_pin(self.tile.pin(), account)?;
        Ok(Self {
            schema: self.schema,
            axes: self.axes,
            tile: PublishedTile { pin },
            receipt: self.receipt,
        })
    }
}
