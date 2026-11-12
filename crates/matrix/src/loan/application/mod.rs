// SPDX-License-Identifier: AGPL-3.0-only
//! One transferred source pin and one boxed core implement the closed pointwise laws.

mod admission;
pub(super) mod binary;
mod chain;
mod contract;
pub use binary::{
    BinaryApplicationRequest, BinaryRegion, PointwiseBinaryOperation, PointwiseBinaryRequest,
};
mod endpoints;
pub(super) mod evaluation;
mod member;
mod prepared;
mod publication;
pub use prepared::{Expm1ApplicationRequest, PointwiseExpm1Request, PreparedExpm1Operation};

use super::registry::{
    Bundle, CancellationReceipt, Declaration, Identity, LoanTicket, PublicationReceipt,
    QualifiedMaskVersion, Token,
};
use super::{BundlePin, ExecutionAccount, HostTileDeclaration, LoanError, PublishedTile};
use crate::{CheckedOperation, ExecutionLimits, Heap, KernelError, MaskVersion, StructureScope};
use nrz_scalar::Scalar;
use std::rc::Rc;

/// All new wrapper phases use the existing numerical family on the original account.
pub(super) const FAMILY: &str = "matrix.kernel";

/// A closed application refusal preserves the original typed cause.
#[derive(Clone, Debug, thiserror::Error)]
pub enum ApplicationError {
    /// Ownership, selected coverage or original-account refusal.
    #[error("application ownership: {0}")]
    Loan(#[from] LoanError),
    /// Existing checked operation or numerical environment refusal.
    #[error("application operation: {0}")]
    Operation(#[from] crate::OperationError),
    /// Binary structure retains the original operand position and assertion scope.
    #[error("unsupported binary structural zero on {position:?}, entry={entry}, scope={scope:?}")]
    UnsupportedBinaryStructure {
        /// Original region position.
        position: BinaryRegion,
        /// Original flat entry.
        entry: usize,
        /// Original assertion scope.
        scope: StructureScope,
    },
    /// Structural assertions require a later scope-sensitive numerical law.
    #[error("unsupported structural zero on output={output}, entry={entry}, scope={scope:?}")]
    UnsupportedStructure {
        /// False selects the input shape; true selects the output shape.
        output: bool,
        /// Original column-major entry.
        entry: usize,
        /// Original retained assertion scope.
        scope: StructureScope,
    },
}

/// Complete request, including the one owning input edge returned on admission refusal.
#[derive(Debug)]
pub struct PointwiseRequest<P: Scalar> {
    /// Original checked closed operation and its meaning witnesses.
    pub operation: CheckedOperation,
    /// Exact owned source edge transferred on successful admission.
    pub input: BundlePin<P>,
    /// Independently declared fresh center, total and error symbols.
    pub destination: HostTileDeclaration<P>,
    /// Ordinary demand on the source's exact native layout.
    pub demand: MaskVersion,
    /// Existing execution ceilings for the per-cell law and complete output payload.
    pub limits: ExecutionLimits,
}

/// Admission never discards or replaces the supplied input pin.
#[derive(Debug)]
#[expect(
    clippy::large_enum_variant,
    reason = "admission refusal returns the entire original request without an unplanned allocation"
)]
pub enum ApplicationRequest<P: Scalar> {
    /// Detached ready owner with complete prepaid cleanup.
    ApplicationReady(OwnedApplication<P>),
    /// Exact original request, before output allocation or after private-prefix cleanup.
    ApplicationRefused(ApplicationError, PointwiseRequest<P>),
}
/// Whole traversal either starts after complete prepayment or leaves the ready owner unchanged.
#[derive(Debug)]
pub enum ApplicationEvaluation<P: Scalar> {
    /// Every selected cell has a terminal numerical outcome.
    ApplicationCandidate(OwnedApplication<P>),
    /// Evaluation preparation refused without modifying output bytes or ready fields.
    ApplicationNotStarted(ApplicationError, OwnedApplication<P>),
    /// Authentication or phase rejected with the same owner and phase.
    ApplicationRejected(ApplicationError, OwnedApplication<P>),
}
/// Publication repair retains evaluated evidence and never reruns a scalar operation.
#[derive(Debug)]
pub enum ApplicationPublication<P: Scalar> {
    /// Independent immutable output and its initial use pin.
    ApplicationPublished(PublishedTile<P>, PublicationReceipt),
    /// Authenticated candidate retained for publication retry or cancellation.
    ApplicationRepair(ApplicationError, OwnedApplication<P>),
    /// Authentication or phase refusal with the unchanged owner.
    ApplicationRejected(ApplicationError, OwnedApplication<P>),
}
/// Every owning phase can spend its existing prepaid cancellation entitlement.
#[derive(Debug)]
pub enum ApplicationCancellation<P: Scalar> {
    /// Terminal absence after the input edge and private output owners are released.
    ApplicationCancelled(CancellationReceipt),
    /// Same owner returned after an authentication rejection.
    ApplicationRejected(ApplicationError, OwnedApplication<P>),
}
/// Fixed-size inline outcome; a refused component's placeholders grant no filled authority.
#[derive(Clone, Debug)]
pub enum CellOutcome {
    /// Outside the captured original demand.
    NotRequested,
    /// Requested but absent from at least one required input attachment.
    MissingInput,
    /// Available input, before the single complete evaluation attempt.
    Pending,
    /// Every component has successful evidence from the selected closed law.
    Completed,
    /// First prepared numerical refusal; every later component still runs.
    PreparedRefused {
        /// Original component within the cell.
        component: usize,
        /// Lossless native validation or arithmetic refusal.
        reason: nrz_scalar::Expm1Failure,
    },
    /// First numerical refusal in component order; later components are still evaluated.
    CellRefused {
        /// Original component within this logical cell.
        component: usize,
        /// Original typed kernel reason, including its original flat entry.
        reason: KernelError,
    },
}
/// Sole application capability; no output setter or public completion constructor exists.
pub struct OwnedApplication<P: Scalar> {
    /// One box survives ready, candidate and every repair unchanged.
    pub(super) core: Box<ApplicationCore<P>>,
}
/// Input and per-cell owners precede the common core's final liveness witness in drop order.
pub(super) struct ApplicationCore<P: Scalar> {
    /// Fixed Lower/Upper result storage; no endpoint descriptor or separate header.
    pub(super) endpoints: Option<[super::payload::LoanChannel; 2]>,
    /// Exact transferred use edge, released before witness expiration.
    pub(super) input: Option<BundlePin<P>>,
    /// Private checked one-cell numerical law; its extent allocation ends at publication.
    pub(super) contract: crate::kernels::KernelContract,
    /// Exact allocation token for the one-element extent owner.
    pub(super) extent_token: Token,
    /// Original A, reduced only by numerical refusals to completed coverage.
    pub(super) filled: Vec<u64>,
    /// The pre-existing filled-word debit receives the publication shell's bytes.
    pub(super) filled_token: Token,
    /// Original operation, selection and sealed per-cell evidence.
    pub(super) evidence: Evidence,
    /// Population of original A, fixed before any numerical traversal.
    pub(super) population: usize,
    /// Sole legal transition is ready to candidate.
    pub(super) candidate: bool,
    /// Common numerical owners and shells; its witness is the final field destroyed.
    pub(super) host: super::lifecycle::HostCore,
}
impl<P: Scalar> std::fmt::Debug for OwnedApplication<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedApplication")
            .field(
                "ticket",
                &LoanTicket {
                    identity: self.core.host.identity,
                },
            )
            .field(
                "phase",
                &if self.core.candidate {
                    "candidate"
                } else {
                    "ready"
                },
            )
            .finish_non_exhaustive()
    }
}
impl<P: Scalar> std::fmt::Debug for ApplicationCore<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApplicationCore")
            .field(
                "ticket",
                &LoanTicket {
                    identity: self.host.identity,
                },
            )
            .field("phase", &if self.candidate { "candidate" } else { "ready" })
            .finish_non_exhaustive()
    }
}
/// Immutable application evidence owns outcomes and symbolic metadata, never a source pin.
pub(super) struct Evidence {
    /// Closed prepared correspondence, absent for Square.
    pub(super) prepared: Option<PreparedExpm1Operation>,
    /// Fixed Lower/Upper payload identities sealed before evaluation.
    pub(in crate::loan) endpoint_tokens: binary::Auxiliary,
    /// Non-owning identity of the immutable completed mask, installed only at publication.
    pub(super) completed_mask: Option<std::ptr::NonNull<crate::mask::storage::MaskRecord>>,
    /// Original checked operation, including frozen declarations.
    pub(super) operation: CheckedOperation,
    /// Structural exclusion is sealed to these exact immutable input contracts.
    pub(super) excluded_inputs: Rc<[crate::RegionContract]>,
    /// Structural exclusion is sealed to these exact immutable output contracts.
    pub(super) excluded_outputs: Rc<[crate::RegionContract]>,
    /// Original checked closed recipe identity.
    pub(super) law: Rc<crate::kernels::KernelContract>,
    /// Historical source declaration retains only symbolic witnesses.
    pub(super) source: Declaration,
    /// Historical source occupation; current result access never resolves it.
    pub(super) source_identity: Identity,
    /// Every original filled attachment was authenticated before capture.
    pub(super) attachments: [Option<QualifiedMaskVersion>; 3],
    /// Historical ordinary demand may retire immediately after admission.
    pub(super) demand: MaskVersion,
    /// Exact destination occupation authenticated at completion/publication.
    pub(super) destination: Identity,
    /// Scalar arithmetic revision established for the whole traversal.
    pub(super) revision: u32,
    /// Outcomes retain original D and A without another word allocation.
    pub(super) outcomes: Vec<CellOutcome>,
    /// Unique outcome backing debit follows this owner into the immutable bundle.
    pub(super) token: Token,
}
impl std::fmt::Debug for Evidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Evidence")
            .field(
                "ticket",
                &LoanTicket {
                    identity: self.destination,
                },
            )
            .finish_non_exhaustive()
    }
}
/// Current authentication distinguishes host imports from independently owned kernel results.
#[derive(Debug)]
pub(super) struct Origin {
    /// Absence identifies a host producer; presence owns fixed inline application evidence.
    pub(super) evidence: Option<Evidence>,
}
impl Origin {
    /// Host publication carries no numerical completion evidence.
    pub(super) const fn host() -> Self {
        Self { evidence: None }
    }
    /// Application evidence moves inline without another allocation or source edge.
    pub(super) const fn application(evidence: Evidence) -> Self {
        Self {
            evidence: Some(evidence),
        }
    }
    /// Authenticate the result itself, without resolving historical numerical occupations.
    pub(super) fn validate_current(&self, bundle: &Bundle) -> Result<(), LoanError> {
        endpoints::validate_bundle(bundle)?;
        self.evidence.as_ref().map_or_else(
            || {
                bundle
                    .declaration
                    .symbols
                    .check_import()
                    .map_err(|_| LoanError::StaleDependency)
            },
            |evidence| evidence.validate(&bundle.declaration, bundle.identity),
        )
    }
    /// Borrow only the immutable inline outcome from the retaining pin.
    pub(super) fn outcome(&self, cell: usize) -> Result<&CellOutcome, LoanError> {
        let evidence = self
            .evidence
            .as_ref()
            .ok_or(LoanError::UnsupportedContract)?;
        let outcome = evidence
            .outcomes
            .get(cell)
            .ok_or(LoanError::LayoutMismatch)?;
        if evidence.endpoint_tokens.binary().is_some() {
            member::Member::check(
                &evidence.operation,
                evidence.law.kind(),
                evidence.prepared.as_ref(),
            )?
            .validate_outcome(outcome, cell, evidence.source.extent)?;
        }
        Ok(outcome)
    }
}
impl<P: Scalar + 'static> OwnedApplication<P> {
    /// Diagnostic locator conveys no ownership or numerical retention.
    #[must_use]
    pub const fn ticket(&self) -> LoanTicket {
        LoanTicket {
            identity: self.core.host.identity,
        }
    }
    /// Inspect one cell outcome on the original continuation account.
    /// # Errors
    /// Refuses a foreign account, invalid cell or exhausted Work.
    pub fn application_outcome(
        &self,
        cell: usize,
        account: &mut ExecutionAccount,
    ) -> Result<&CellOutcome, LoanError> {
        account.authenticate(self.core.host.identity.account)?;
        account
            .ledger
            .with_work_phase(FAMILY, "application outcome", |budget| {
                super::charge(budget, 1)?;
                self.core
                    .evidence
                    .outcomes
                    .get(cell)
                    .ok_or(LoanError::LayoutMismatch)
            })
    }
    /// Prepay and complete the entire selected traversal without a heap borrow.
    pub fn evaluate(self, account: &mut ExecutionAccount) -> ApplicationEvaluation<P> {
        evaluation::evaluate(self, account)
    }
}
impl Heap {
    /// Admit the closed unary square lifting and transfer exactly the supplied source edge.
    pub fn request_pointwise_square<P: Scalar + 'static>(
        &mut self,
        request: PointwiseRequest<P>,
        account: &mut ExecutionAccount,
    ) -> ApplicationRequest<P> {
        admission::request(self, request, crate::Elementwise::Square, account)
    }
    /// Admit binary64 reciprocal over represented input boxes separated from zero.
    /// Numerical domain refusals remain local to selected cells during evaluation.
    pub fn request_pointwise_reciprocal(
        &mut self,
        request: PointwiseRequest<f64>,
        account: &mut ExecutionAccount,
    ) -> ApplicationRequest<f64> {
        admission::request(self, request, crate::Elementwise::Reciprocal, account)
    }
    /// Admit binary64 positive square root over nonnegative represented input boxes.
    /// Zero is supported; numerical domain refusals remain local to selected cells.
    pub fn request_pointwise_square_root(
        &mut self,
        request: PointwiseRequest<f64>,
        account: &mut ExecutionAccount,
    ) -> ApplicationRequest<f64> {
        admission::request(self, request, crate::Elementwise::SquareRoot, account)
    }
    /// Publish complete sealed evidence or retain the same candidate for repair.
    pub fn publish_application<P: Scalar + 'static>(
        &mut self,
        owner: OwnedApplication<P>,
        account: &mut ExecutionAccount,
    ) -> ApplicationPublication<P> {
        publication::publish(self, owner, account)
    }
    /// Cancel any rightful owning phase without fresh evaluation Work.
    pub fn cancel_application<P: Scalar + 'static>(
        &mut self,
        owner: OwnedApplication<P>,
        account: &mut ExecutionAccount,
    ) -> ApplicationCancellation<P> {
        publication::cancel(self, owner, account)
    }
}

#[cfg(test)]
mod tests;
