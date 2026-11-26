// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed native request parts precede source publication and cannot manufacture a pin.

use super::native::Native;
use crate::{
    BundlePin, CheckedOperation, ExecutionLimits, HostTileDeclaration, LcInputShape, LcOmegaOutput,
    LcShapeAxes, LoanError, LoanTicket, MaskVersion,
};
use nrz_bindings::readings::algebra::enclosure::lc_root::Certificate;
use nrz_scalar::Scalar;

/// Complete mapped request parts, before its producer supplies the original source pin.
#[derive(Debug)]
pub struct Mapping<P: Scalar> {
    /// Original destination declaration.
    pub destination: HostTileDeclaration<P>,
    /// Original qualified demand version.
    pub demand: MaskVersion,
    /// Original checked map and operation.
    pub operation: CheckedOperation,
    /// Existing immutable per-call native limits.
    pub limits: ExecutionLimits,
}

/// Admitted dependency interpretation remains fixed even after native owners change phase.
#[derive(Debug)]
#[expect(
    clippy::large_enum_variant,
    reason = "fixed admitted records include request parts without allocating a separate box"
)]
pub(super) enum Contract<'i, P: Scalar, const N: usize> {
    /// Only an explicit request can issue this owned host expectation.
    Input(Option<HostTileDeclaration<P>>),
    /// A mapped consumer waits for one exact host node and its original schema.
    Mapped {
        /// Original producer slot.
        source: usize,
        /// Borrowed original inventory member.
        schema: &'i LcInputShape<P>,
        /// Original ordered tuple roles.
        axes: LcShapeAxes,
        /// Moved exactly once into the native request after fan-out settles.
        parts: Option<Mapping<P>>,
    },
    /// Every original positional input participates, even when not numerically selected.
    Omega {
        /// Fixed original constraint positions; no omitted or deduplicated source list.
        sources: [usize; N],
        /// Original requested output scope.
        demand: MaskVersion,
        /// The original specialization authenticates a consuming preparation return.
        specialization: u64,
        /// The construction/replay choice cannot change on retry.
        certificate: Option<&'i Certificate>,
    },
    /// A terminal delivery delegates reads to the producer's sealed result.
    Delivery {
        /// Exact original checked producer.
        source: usize,
    },
}

/// Closed dispositions preserve dependency causes without retaining variable native errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    /// Original expectation or native continuation still needs work.
    Pending,
    /// An exact issued host loan remains outside the graph.
    Borrowed,
    /// Native publication completed before any edge notification.
    Published,
    /// A no-loan expectation closed without contribution.
    Absent,
    /// Required input settled without enough contribution.
    MissingInput,
    /// Explicit native cancellation completed.
    Cancelled,
    /// One producer refused; the original positional cause remains named.
    DependencyRefused(usize),
    /// A strict nonempty partial intersection cannot enter the checked evaluator.
    UnsupportedPartial,
    /// The original finite attempt envelope was exhausted.
    AttemptsExhausted,
    /// The caller received an intact native refusal and its continuation.
    Returned,
    /// A native typed refusal is terminal; its full cause was returned intact.
    Refused,
    /// A proven capacity or allocation obstruction awaits an explicit retry.
    ResourceWait,
    /// An independent native occupation must release its ownership first.
    OwnershipWait,
    /// An explicit original-closure repair or cancellation is required.
    Repair,
}

/// A proposal becomes mutable only inside its non-cloneable admitted owner.
#[derive(Debug)]
pub struct Node<'i, P: Scalar, const N: usize> {
    /// Fixed declared interpretation and borrowed originals.
    pub(super) contract: Contract<'i, P, N>,
    /// Exact currently owned native continuation, never reconstructed from a ticket.
    pub(super) native: Option<Native<'i, P, N>>,
    /// The initial host pin or the separately charged fan-out edge for this consumer.
    pub(super) pin: Option<BundlePin<P>>,
    /// Escaped host loan locator grants no writable or cancellation authority.
    pub(super) loan: Option<LoanTicket>,
    /// Closed state stays available after numerical owners are released.
    pub(super) state: State,
    /// A fan-out retry names its original producer while retaining the consumer attempt envelope.
    pub(super) waiting_for: Option<usize>,
    /// Recorded outgoing position prevents repeated retention after a retry.
    pub(super) fanout: usize,
    /// Original attempt receipt position, incremented only after sink preparation.
    pub(super) probes: u64,
    /// First reserved probe destination for this original node.
    pub(super) probe_base: usize,
    /// Fixed terminal evidence destination after successful installation.
    pub(super) evidence: Option<super::super::sink::Handle>,
    /// Consuming-return evidence is distinct from a later terminal disposition in the same slot.
    pub(super) returned_evidence: Option<super::super::sink::Handle>,
    /// Sink acknowledgement can commit before a later walk acknowledgement refuses.
    pub(super) evidence_acknowledged: bool,
    /// Native terminal delivery was acknowledged exactly once.
    pub(super) native_delivery: bool,
}
impl<'i, P: Scalar, const N: usize> Node<'i, P, N> {
    /// An owned no-loan expectation can later issue exactly its declared host input.
    #[must_use]
    pub const fn input(declaration: HostTileDeclaration<P>) -> Self {
        Self::new(Contract::Input(Some(declaration)), None)
    }
    /// Store admitted request parts without acquiring or fabricating a source pin.
    #[must_use]
    pub const fn mapped(
        source: usize,
        schema: &'i LcInputShape<P>,
        axes: &LcShapeAxes,
        parts: Mapping<P>,
    ) -> Self {
        Self::new(
            Contract::Mapped {
                source,
                schema,
                axes: *axes,
                parts: Some(parts),
            },
            None,
        )
    }
    /// Construction or caller-selected certificate replay is fixed at admission.
    #[must_use]
    pub const fn omega(
        declaration: LcOmegaOutput<'i, P, N>,
        sources: [usize; N],
        demand: MaskVersion,
        certificate: Option<&'i Certificate>,
    ) -> Self {
        Self::new(
            Contract::Omega {
                sources,
                demand,
                specialization: declaration.specialization_key(),
                certificate,
            },
            Some(Native::OmegaDeclaration(declaration)),
        )
    }
    /// Delivery owns no unsealed result or ordinary output-symbol conversion.
    #[must_use]
    pub const fn delivery(source: usize) -> Self {
        Self::new(Contract::Delivery { source }, None)
    }
    /// Fixed inline initialization adds no collection capacity or numerical edge.
    const fn new(contract: Contract<'i, P, N>, native: Option<Native<'i, P, N>>) -> Self {
        Self {
            contract,
            native,
            pin: None,
            loan: None,
            state: State::Pending,
            waiting_for: None,
            fanout: 0,
            probes: 0,
            probe_base: 0,
            evidence: None,
            returned_evidence: None,
            evidence_acknowledged: false,
            native_delivery: false,
        }
    }
}

impl<P: Scalar, const N: usize> Node<'_, P, N> {
    /// Admit both counter and destination before entering any native phase or sink mutation.
    pub(super) fn next_probe(&self) -> Result<(u64, usize), LoanError> {
        let next = self.probes.checked_add(1).ok_or(LoanError::CountOverflow)?;
        let position = self
            .probe_base
            .checked_add(usize::try_from(self.probes).map_err(|_| LoanError::CountOverflow)?)
            .ok_or(LoanError::CountOverflow)?;
        Ok((next, position))
    }
}
