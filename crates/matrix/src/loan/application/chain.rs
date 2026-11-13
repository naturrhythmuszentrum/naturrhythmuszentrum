// SPDX-License-Identifier: AGPL-3.0-only
//! Opt-in current lineage checks borrow registry roots without retaining ancestors.

use super::{Evidence, member::Member};
use crate::loan::{
    charge,
    registry::{
        Bundle, DebitInventory, Declaration, Group, Identity, ProducerOutcome,
        QualifiedMaskVersion, State,
    },
};
use crate::{ApplicationRequest, BundlePin, ExecutionAccount, Heap, LoanError, PointwiseRequest};
use nrz_bindings::Budget;
use nrz_scalar::Scalar;

/// Admission policy is temporary; publication derives it from the actual immutable input.
#[derive(Clone, Copy)]
pub(super) enum Input {
    /// Existing entry points accept only host origins with their unchanged tariff.
    Host,
    /// Native continuation and mapped movement require current application lineage.
    Application,
}

impl Heap {
    /// Apply square to a published result in either native format with current numerical lineage.
    pub fn request_chained_pointwise_square<P: Scalar + 'static>(
        &mut self,
        request: PointwiseRequest<P>,
        account: &mut ExecutionAccount,
    ) -> ApplicationRequest<P> {
        super::admission::request_with_origin(
            self,
            request,
            crate::Elementwise::Square,
            Input::Application,
            account,
        )
    }
    /// Apply square root to a published binary64 result with current numerical lineage.
    pub fn request_chained_pointwise_square_root(
        &mut self,
        request: PointwiseRequest<f64>,
        account: &mut ExecutionAccount,
    ) -> ApplicationRequest<f64> {
        super::admission::request_with_origin(
            self,
            request,
            crate::Elementwise::SquareRoot,
            Input::Application,
            account,
        )
    }
    /// Apply reciprocal to a published binary64 result with current numerical lineage.
    pub fn request_chained_pointwise_reciprocal(
        &mut self,
        request: PointwiseRequest<f64>,
        account: &mut ExecutionAccount,
    ) -> ApplicationRequest<f64> {
        super::admission::request_with_origin(
            self,
            request,
            crate::Elementwise::Reciprocal,
            Input::Application,
            account,
        )
    }
}

/// Borrow the pinned immutable owner without introducing another retaining edge.
fn bundle<P: Scalar>(pin: &BundlePin<P>) -> Result<&Bundle, LoanError> {
    pin.record
        .payload
        .as_ref()
        .and_then(|pin| pin.bundle.payload.as_ref())
        .ok_or(LoanError::WrongLoan)
}

/// Publication cannot opt out of composition by changing request metadata.
pub(super) fn origin<P: Scalar>(pin: &BundlePin<P>) -> Result<Input, LoanError> {
    Ok(if bundle(pin)?.origin.evidence.is_some() {
        Input::Application
    } else {
        Input::Host
    })
}

/// Each detached checked reader authenticates only the immediate immutable application seal.
pub(super) fn seal_work<P: Scalar>(pin: &BundlePin<P>) -> Result<usize, LoanError> {
    let source = bundle(pin)?;
    Ok(source.origin.evidence.as_ref().map_or(0, |evidence| {
        1 + usize::from(evidence.endpoint_tokens.binary().is_some())
    }))
}

/// Exact capture comparisons also protect detached evaluation after heap destruction.
pub(super) fn captured<P: Scalar + 'static>(
    pin: &BundlePin<P>,
    evidence: &Evidence,
    account: u64,
) -> Result<(), LoanError> {
    let payload = pin.record.payload.as_ref().ok_or(LoanError::WrongLoan)?;
    if payload.account != account || pin.ticket()?.identity != evidence.source_identity {
        return Err(LoanError::WrongLoan);
    }
    matches_capture(bundle(pin)?, &evidence.source, &evidence.attachments)
}

/// Charge the ordinary current authentication before the additional origin visit and edges.
pub(super) fn authenticate<P: Scalar + 'static>(
    heap: &Heap,
    pin: &BundlePin<P>,
    mode: Input,
    budget: &mut Budget,
) -> Result<(), LoanError> {
    heap.current_bundle(pin, budget)?;
    let immediate = bundle(pin)?;
    match mode {
        Input::Host => {
            if immediate.origin.evidence.is_some() {
                return Err(LoanError::UnsupportedContract);
            }
            return Ok(());
        }
        Input::Application => charge(budget, 1)?,
    }
    if crate::storage::Format::of::<P>() != immediate.identity.format
        || immediate.origin.evidence.is_none()
    {
        return Err(LoanError::UnsupportedContract);
    }
    eligible(immediate)?;
    inventory(heap.loans.group(immediate.identity)?, immediate)?;
    let mut current = immediate;
    while let Some(evidence) = &current.origin.evidence {
        let left = parent(
            heap,
            current.identity,
            evidence.source_identity,
            &evidence.source,
            &evidence.attachments,
            budget,
        )?;
        if let Some(side) = evidence.endpoint_tokens.binary() {
            if left.origin.evidence.is_some() {
                return Err(LoanError::UnsupportedContract);
            }
            let right = parent(
                heap,
                current.identity,
                side.identity,
                &side.source,
                &side.attachments,
                budget,
            )?;
            if right.origin.evidence.is_some() {
                return Err(LoanError::UnsupportedContract);
            }
            if left.identity == right.identity
                || left.declaration.symbols.value() == right.declaration.symbols.value()
            {
                return Err(LoanError::WrongLoan);
            }
            return Ok(());
        }
        current = left;
    }
    Ok(())
}

/// One strictly decreasing edge, one exact registry lookup and the common owner tariff.
fn parent<'a>(
    heap: &'a Heap,
    child: Identity,
    identity: Identity,
    declaration: &Declaration,
    attachments: &[Option<QualifiedMaskVersion>; 3],
    budget: &mut Budget,
) -> Result<&'a Bundle, LoanError> {
    charge(budget, 1)?;
    if identity.scope != child.scope
        || identity.scope != heap.scope
        || identity.request >= child.request
        || identity.format != child.format
    {
        return Err(LoanError::WrongLoan);
    }
    charge(budget, 1)?;
    let group = heap.loans.group(identity).map_err(|error| match error {
        LoanError::StaleOccupation => LoanError::StaleDependency,
        other => other,
    })?;
    let State::Published(owner) = &group.state else {
        return Err(LoanError::StaleDependency);
    };
    let source = owner.payload.as_ref().ok_or(LoanError::WrongLoan)?;
    if source.identity != identity {
        return Err(LoanError::WrongLoan);
    }
    matches_capture(source, declaration, attachments)?;
    inventory(group, source)?;
    heap.current_owner(source, budget)?;
    eligible(source)?;
    Ok(source)
}

/// Captured symbolic witnesses and role attachments identify the exact published operand.
fn matches_capture(
    source: &Bundle,
    declaration: &Declaration,
    attachments: &[Option<QualifiedMaskVersion>; 3],
) -> Result<(), LoanError> {
    let actual = &source.declaration;
    if actual.symbols.value() != declaration.symbols.value()
        || actual.symbols.bound() != declaration.symbols.bound()
        || actual.symbols.error() != declaration.symbols.error()
        || actual.axis != declaration.axis
        || actual.extent != declaration.extent
        || actual.coverage != declaration.coverage
        || source.qualified.as_slice() != attachments.as_slice()
    {
        return Err(LoanError::WrongLoan);
    }
    Ok(())
}

/// Reject unsupported representations while preserving specific layout and coverage faults.
fn eligible(source: &Bundle) -> Result<(), LoanError> {
    if source.endpoints.is_some() || source.retained_law.is_some() {
        return Err(LoanError::UnsupportedContract);
    }
    if source.declaration.coverage != crate::Coverage::RepresentedInputs {
        return Err(LoanError::CoverageMismatch);
    }
    if source.layout.gather.is_some()
        || source.layout.axis != source.declaration.axis
        || source.layout.components.extent() != source.declaration.extent
        || !source.layout.components.writable_affine()?
    {
        return Err(LoanError::LayoutMismatch);
    }
    if let Some(evidence) = &source.origin.evidence {
        if evidence.prepared.is_some()
            || source.channels.iter().any(Option::is_none)
            || source.declaration.extent.columns() == 0
        {
            return Err(LoanError::UnsupportedContract);
        }
        match Member::check(&evidence.operation, evidence.law.kind(), None)? {
            Member::Transform(_) | Member::ProjectedSquare(_) | Member::Square(_) => {}
            Member::SquareRoot(_) | Member::Reciprocal(_) | Member::Binary(..) => {
                if source.identity.format != crate::storage::Format::Binary64 {
                    return Err(LoanError::UnsupportedContract);
                }
            }
            Member::PreparedExpm1(..) => return Err(LoanError::UnsupportedContract),
        }
    }
    super::contract::attachments(source)
}

/// Registry inventory and terminal receipt must describe this immutable origin exactly.
fn inventory(group: &Group, bundle: &Bundle) -> Result<(), LoanError> {
    let ProducerOutcome::Published(receipt) = group.outcome else {
        return Err(LoanError::WrongLoan);
    };
    let evidence = bundle.origin.evidence.as_ref();
    let matches = match &group.debits {
        DebitInventory::Host(_) => evidence.is_none(),
        DebitInventory::Application(_) => evidence
            .is_some_and(|item| item.prepared.is_none() && item.endpoint_tokens.binary().is_none()),
        DebitInventory::BinaryApplication(_) => evidence
            .is_some_and(|item| item.prepared.is_none() && item.endpoint_tokens.binary().is_some()),
        DebitInventory::PreparedApplication(_) => false,
    };
    if !matches
        || receipt.ticket.identity != bundle.identity
        || receipt.present != bundle.channels.each_ref().map(Option::is_some)
    {
        return Err(LoanError::WrongLoan);
    }
    Ok(())
}
