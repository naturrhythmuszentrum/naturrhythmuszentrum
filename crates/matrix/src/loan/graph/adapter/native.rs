// SPDX-License-Identifier: AGPL-3.0-only
//! Native continuations remain distinct through request, execution and publication.

use crate::{
    ApplicationError, ExecutionAccount, Heap, LcApplicationEvaluated,
    LcApplicationEvaluationFailure, LcApplicationFailed, LcApplicationPending, LcOmegaOutput,
    LcOmegaPrepared, LcProductError, LcPublishedResult, LcShapeApplication, LcShapeAxes,
    LcShapeError, LcShapeRequest, LcShapeTile, LoanError, MaskVersion, PointwiseRequest,
};
use nrz_bindings::{Registry, readings::algebra::enclosure::lc_root::Certificate};
use nrz_scalar::Scalar;

/// Full structured causes escape to the caller; graph records retain only closed dispositions.
#[derive(Debug, thiserror::Error)]
pub enum Failure {
    /// Mapped request, execution or publication retained its exact continuation.
    #[error("mapped continuation: {0}")]
    Mapped(ApplicationError),
    /// Consuming correspondence preparation returned the identical declaration.
    #[error("checked preparation: {0}")]
    Preparation(LcShapeError),
    /// Native preparation, publication or cancellation retained its exact owner.
    #[error("native continuation: {0}")]
    Native(LoanError),
    /// Execution distinguishes an unstarted continuation from a cancel-only failure.
    #[error("checked execution: {0}")]
    Execution(LcProductError),
}

/// Each variant owns only the native capability for its exact admitted phase.
#[derive(Debug)]
pub(super) enum Native<'i, P: Scalar, const N: usize> {
    /// The original schema and complete request survive request refusal.
    MappedRequest {
        /// Original inventory member, never reconstructed from symbols.
        schema: &'i crate::LcInputShape<P>,
        /// Original qualified tuple roles.
        axes: LcShapeAxes,
        /// Original checked operation, source pin, destination and demand.
        request: PointwiseRequest<P>,
    },
    /// No numerical traversal has entered.
    MappedReady(LcShapeApplication<'i, P>),
    /// The completed mapped candidate may publish or cancel, never evaluate again.
    MappedEvaluated(LcShapeApplication<'i, P>),
    /// Actual mapped intersection, including a legitimate empty intersection.
    MappedPublished(LcShapeTile<'i, P>),
    /// Original declaration chooses construction or explicit replay once.
    OmegaDeclaration(LcOmegaOutput<'i, P, N>),
    /// A complete checked join has not yet acquired numerical pending storage.
    OmegaPrepared(LcOmegaPrepared<'i, P, N>),
    /// Private numerical storage has not entered traversal.
    OmegaPending(LcApplicationPending<'i, P, N>),
    /// Completed sealed candidate can only publish or cancel.
    OmegaEvaluated(LcApplicationEvaluated<'i, P, N>),
    /// A started failed traversal can only cancel.
    OmegaFailed(LcApplicationFailed<'i, P, N>),
    /// Sealed result supplies only its existing specialized readers.
    OmegaPublished(LcPublishedResult<P>),
    /// Explicit native cancellation has completed.
    Cancelled,
}

/// One admitted native call preserves its next owner together with any structured refusal.
pub(super) struct Transition<'i, P: Scalar, const N: usize> {
    /// Exact original or successor owner, moved without allocation.
    pub(super) owner: Native<'i, P, N>,
    /// Never retained in graph storage or reduced to a fabricated mathematical outcome.
    pub(super) failure: Option<Failure>,
}
impl<'i, P: Scalar, const N: usize> Transition<'i, P, N> {
    /// A phase commit preserves the returned native owner before notification is considered.
    pub(super) const fn accepted(owner: Native<'i, P, N>) -> Self {
        Self {
            owner,
            failure: None,
        }
    }
    /// Refusal preserves the exact native owner and the full cause independently.
    pub(super) const fn refused(owner: Native<'i, P, N>, failure: Failure) -> Self {
        Self {
            owner,
            failure: Some(failure),
        }
    }
}
impl<P: Scalar, const N: usize> Native<'_, P, N> {
    /// Borrow actual law ownership across native phases without inventing an occupation.
    pub(super) fn retained_correspondence(
        &self,
    ) -> Option<&std::rc::Rc<nrz_bindings::LcOmegaCorrespondence>> {
        match self {
            Self::OmegaPrepared(owner) => Some(&owner.correspondence),
            Self::OmegaPending(owner) => owner.retained_correspondence(),
            Self::OmegaEvaluated(owner) => owner.retained_correspondence(),
            Self::OmegaFailed(owner) => owner.retained_correspondence(),
            Self::OmegaPublished(owner) => owner.retained_correspondence(),
            _ => None,
        }
    }
}

impl<'i, P: Scalar + 'static, const N: usize> Native<'i, P, N> {
    /// Request, evaluate and publish enter different calls with the same execution account.
    pub(super) fn advance_mapped(
        self,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> Transition<'i, P, N> {
        match self {
            Self::MappedRequest {
                schema,
                axes,
                request,
            } => match schema.request(heap, &axes, request, account) {
                LcShapeRequest::Ready(owner) => Transition::accepted(Self::MappedReady(owner)),
                LcShapeRequest::Refused(error, request) => Transition::refused(
                    Self::MappedRequest {
                        schema,
                        axes,
                        request,
                    },
                    Failure::Mapped(error),
                ),
            },
            Self::MappedReady(mut owner) => match owner.evaluate(account) {
                Ok(()) => Transition::accepted(Self::MappedEvaluated(owner)),
                Err(error) => Transition::refused(Self::MappedReady(owner), Failure::Mapped(error)),
            },
            Self::MappedEvaluated(mut owner) => match owner.publish(heap, account) {
                Ok(tile) => Transition::accepted(Self::MappedPublished(tile)),
                Err(error) => {
                    Transition::refused(Self::MappedEvaluated(owner), Failure::Mapped(error))
                }
            },
            other => Transition::refused(other, Failure::Native(LoanError::WrongLoan)),
        }
    }

    /// Preparation borrows every original positional tile and the admission-selected evidence.
    pub(super) fn prepare_omega(
        self,
        heap: &mut Heap,
        tiles: &[&LcShapeTile<'i, P>],
        registry: &Registry,
        certificate: Option<&Certificate>,
        account: &mut ExecutionAccount,
    ) -> Transition<'i, P, N> {
        let Self::OmegaDeclaration(declaration) = self else {
            return Transition::refused(self, Failure::Native(LoanError::WrongLoan));
        };
        let result = match certificate {
            Some(certificate) => {
                declaration.prepare_replay(heap, tiles, registry, certificate, account)
            }
            None => declaration.prepare(heap, tiles, registry, account),
        };
        match result {
            Ok(owner) => Transition::accepted(Self::OmegaPrepared(owner)),
            Err(failure) => {
                let (declaration, error) = failure.into_parts();
                Transition::refused(
                    Self::OmegaDeclaration(declaration),
                    Failure::Preparation(error),
                )
            }
        }
    }

    /// Pending allocation, traversal and publication cannot collapse into one dispatch.
    pub(super) fn advance_omega(
        self,
        heap: &mut Heap,
        demand: MaskVersion,
        account: &mut ExecutionAccount,
    ) -> Transition<'i, P, N> {
        match self {
            Self::OmegaPrepared(owner) => match owner.admit(heap, demand, account) {
                Ok(owner) => Transition::accepted(Self::OmegaPending(owner)),
                Err(failure) => {
                    let (owner, error) = failure.into_parts();
                    Transition::refused(Self::OmegaPrepared(owner), Failure::Native(error))
                }
            },
            Self::OmegaPending(owner) => match owner.evaluate(account) {
                Ok(owner) => Transition::accepted(Self::OmegaEvaluated(owner)),
                Err(LcApplicationEvaluationFailure::Pending { owner, error }) => {
                    Transition::refused(Self::OmegaPending(owner), Failure::Execution(error))
                }
                Err(LcApplicationEvaluationFailure::Failed { owner, error }) => {
                    Transition::refused(Self::OmegaFailed(owner), Failure::Execution(error))
                }
            },
            Self::OmegaEvaluated(owner) => match owner.publish(heap, account) {
                Ok(owner) => Transition::accepted(Self::OmegaPublished(owner)),
                Err(failure) => Transition::refused(
                    Self::OmegaEvaluated(failure.owner),
                    Failure::Native(failure.error),
                ),
            },
            other => Transition::refused(other, Failure::Native(LoanError::WrongLoan)),
        }
    }

    /// Only capabilities actually held here use their existing authenticated cancellation route.
    pub(super) fn cancel(
        self,
        heap: &mut Heap,
        account: &mut ExecutionAccount,
    ) -> Transition<'i, P, N> {
        match self {
            Self::MappedReady(mut owner) => match owner.cancel(heap, account) {
                Ok(_) => Transition::accepted(Self::Cancelled),
                Err(error) => Transition::refused(Self::MappedReady(owner), Failure::Mapped(error)),
            },
            Self::MappedEvaluated(mut owner) => match owner.cancel(heap, account) {
                Ok(_) => Transition::accepted(Self::Cancelled),
                Err(error) => {
                    Transition::refused(Self::MappedEvaluated(owner), Failure::Mapped(error))
                }
            },
            Self::OmegaPending(mut owner) => match owner.cancel(heap, account) {
                Ok(_) => Transition::accepted(Self::Cancelled),
                Err(error) => {
                    Transition::refused(Self::OmegaPending(owner), Failure::Native(error))
                }
            },
            Self::OmegaEvaluated(mut owner) => match owner.cancel(heap, account) {
                Ok(_) => Transition::accepted(Self::Cancelled),
                Err(error) => {
                    Transition::refused(Self::OmegaEvaluated(owner), Failure::Native(error))
                }
            },
            Self::OmegaFailed(mut owner) => match owner.cancel(heap, account) {
                Ok(_) => Transition::accepted(Self::Cancelled),
                Err(error) => Transition::refused(Self::OmegaFailed(owner), Failure::Native(error)),
            },
            Self::MappedRequest { .. }
            | Self::OmegaDeclaration(_)
            | Self::OmegaPrepared(_)
            | Self::Cancelled => Transition::accepted(Self::Cancelled),
            other => Transition::refused(other, Failure::Native(LoanError::WrongLoan)),
        }
    }
}

impl Failure {
    /// Original native wrappers retain the same cumulative Work identity.
    pub(in crate::loan::graph) fn loan_work_stopped(error: &LoanError) -> bool {
        super::refusal::loan(error, None) == super::refusal::Class::WorkStopped
    }

    /// The original cumulative Work dimension remains a graph-wide stop through native wrappers.
    pub(in crate::loan::graph) fn work_stopped(&self) -> bool {
        super::refusal::classify(self, None) == super::refusal::Class::WorkStopped
    }
}
