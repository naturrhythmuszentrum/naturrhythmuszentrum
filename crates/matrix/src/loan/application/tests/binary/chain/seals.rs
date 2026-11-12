// SPDX-License-Identifier: AGPL-3.0-only
//! Origin, original operation, ordered captures and completion remain separate authentication seals.

use super::super::super::ready;
use super::{TestResult, admit, candidate, prefix, product, request};
use crate::loan::{
    fault,
    registry::{Bundle, State},
};
use crate::{
    ApplicationError, ApplicationEvaluation, ApplicationPublication, ApplicationRequest,
    Elementwise, LoanError,
};
use std::rc::Rc;

/// Withdraw only registry aliases while mutating test evidence, then restore their exact pairing.
fn alter(
    pin: &mut crate::BundlePin<f64>,
    heap: &mut crate::Heap,
    change: impl FnOnce(&mut Bundle) -> TestResult,
) -> TestResult {
    let root = pin.ticket()?.identity.root.slot;
    let slot = pin.record.payload.as_ref().ok_or("pin")?.index;
    let mut index = heap.loans.pins[slot].take().ok_or("index")?;
    index.witness = None;
    heap.loans.groups[root].as_mut().ok_or("group")?.state = State::Reserved;
    let payload = Rc::get_mut(&mut pin.record)
        .and_then(|record| record.payload.as_mut())
        .ok_or("unique pin")?;
    let bundle = Rc::get_mut(&mut payload.bundle)
        .and_then(|owner| owner.payload.as_mut())
        .ok_or("unique bundle")?;
    change(bundle)?;
    heap.loans.groups[root].as_mut().ok_or("group")?.state =
        State::Published(Rc::clone(&payload.bundle));
    index.witness = Some(Rc::downgrade(&pin.record));
    heap.loans.pins[slot] = Some(index);
    Ok(())
}

/// Completed masks, revisions, ordered original operands and exact side tokens cannot be substituted.
#[test]
fn chain_producer_seals_refuse_admission_before_allocation() -> TestResult {
    for choice in 0..5 {
        let (mut heap, mut account, tile, _) = product(1, 1)?;
        let mut request = request(
            &mut heap,
            tile.into_pin(),
            Elementwise::SquareRoot,
            2,
            &mut account,
        )?;
        alter(&mut request.input, &mut heap, |bundle| {
            let evidence = bundle.origin.evidence.as_mut().ok_or("origin")?;
            let crate::loan::application::binary::Auxiliary::Binary(side) =
                &mut evidence.endpoint_tokens
            else {
                return Err("side".into());
            };
            match choice {
                0 => evidence.completed_mask = None,
                1 => evidence.revision = 0,
                2 => std::mem::swap(&mut evidence.source_identity, &mut side.identity),
                3 => std::mem::swap(&mut evidence.source, &mut side.source),
                4 => side.token.key = evidence.token.key,
                _ => return Err("choice".into()),
            }
            Ok(())
        })?;
        let pointer = Rc::as_ptr(&request.input.record);
        let injection = fault::Injection::after(0);
        fault::reset_reads();
        match admit(&mut heap, request, Elementwise::SquareRoot, &mut account) {
            ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::WrongLoan),
                request,
            ) => {
                assert_eq!(Rc::as_ptr(&request.input.record), pointer);
                assert!(request.input.historical_reader(&mut account).is_err());
            }
            other => return Err(format!("choice {choice}: {other:?}").into()),
        }
        assert_eq!([fault::Injection::attempts(), fault::reads()], [0; 2]);
        drop(injection);
    }
    Ok(())
}

/// Self and reversed requests fail the strict decreasing-order check before any lookup allocation.
#[test]
fn chain_self_and_reversed_ancestry_refuse() -> TestResult {
    for reversed in [false, true] {
        let (mut heap, mut account, tile, _) = prefix(1, true)?;
        let mut request = request(
            &mut heap,
            tile.into_pin(),
            Elementwise::Reciprocal,
            2,
            &mut account,
        )?;
        alter(&mut request.input, &mut heap, |bundle| {
            let evidence = bundle.origin.evidence.as_mut().ok_or("origin")?;
            evidence.source_identity.request = bundle.identity.request + u64::from(reversed);
            for attachment in evidence.attachments.iter_mut().flatten() {
                attachment.identity = evidence.source_identity;
            }
            Ok(())
        })?;
        let injection = fault::Injection::after(0);
        assert!(matches!(
            admit(&mut heap, request, Elementwise::Reciprocal, &mut account),
            ApplicationRequest::ApplicationRefused(ApplicationError::Loan(LoanError::WrongLoan), _)
        ));
        assert_eq!(fault::Injection::attempts(), 0);
        drop(injection);
    }
    Ok(())
}

/// Same-valued self-valid foreign results and altered captures cannot replace an admitted source.
#[test]
fn chain_detached_capture_and_law_substitutions_refuse_before_load() -> TestResult {
    for publication in [false, true] {
        for choice in 0..7 {
            detached_substitution(publication, choice)?;
        }
    }
    Ok(())
}

/// The unchanged returned core remains the rightful owner after any fixed capture substitution.
fn detached_substitution(publication: bool, choice: usize) -> TestResult {
    let (mut heap, mut account, tile, _) = product(1, 1)?;
    let request = request(
        &mut heap,
        tile.into_pin(),
        Elementwise::SquareRoot,
        2,
        &mut account,
    )?;
    let mut owner = ready(admit(
        &mut heap,
        request,
        Elementwise::SquareRoot,
        &mut account,
    ))?;
    let (_foreign_heap, _foreign_account, foreign, _) = product(1, 1)?;
    let mut replacement = Some(foreign.into_pin());
    if publication {
        owner = candidate(owner, &mut account)?;
    }
    substitute(&mut owner, &mut replacement, choice)?;
    let pointer = std::ptr::from_ref(owner.core.as_ref());
    fault::reset_reads();
    fault::reset_scalars();
    let injection = fault::Injection::after(0);
    let owner = refused(owner, publication, &mut heap, &mut account)?;
    assert_eq!(std::ptr::from_ref(owner.core.as_ref()), pointer);
    assert_eq!(
        [
            fault::reads(),
            fault::scalars(),
            fault::Injection::attempts()
        ],
        [0; 3]
    );
    drop(injection);
    Ok(())
}

/// The replacement original and law are valid members; neither is the captured root law.
fn substitute(
    owner: &mut crate::OwnedApplication<f64>,
    replacement: &mut Option<crate::BundlePin<f64>>,
    choice: usize,
) -> TestResult {
    match choice {
        0 => std::mem::swap(&mut owner.core.input, replacement),
        1 => owner.core.evidence.source_identity.request += 1,
        2 => owner.core.evidence.attachments[0] = None,
        3 => owner.core.evidence.source.symbols = owner.core.host.declaration.symbols.clone(),
        4 => {
            owner.core.contract = crate::kernels::KernelContract {
                inputs: Rc::clone(&owner.core.contract.inputs),
                output: owner.core.contract.output,
                recipe: crate::kernels::contract::Recipe::Elementwise(Elementwise::Reciprocal),
                work: 12,
            }
        }
        5 | 6 => {
            let operation = crate::OperationDraft::map_elementwise(
                owner.core.evidence.operation.inputs()[0].shape().clone(),
                owner.core.evidence.operation.outputs()[0].shape().clone(),
                Elementwise::Reciprocal,
            )?
            .check(crate::ExecutionLimits::default())?;
            if choice == 5 {
                owner.core.evidence.operation = operation;
            } else {
                let crate::binding::NativeRecipe::Kernel(law) = operation.recipe else {
                    return Err("law".into());
                };
                owner.core.evidence.law = law;
            }
        }
        _ => return Err("choice".into()),
    }
    Ok(())
}

/// Both boundaries retain typed authentication faults before numerical access and allocation.
fn refused(
    owner: crate::OwnedApplication<f64>,
    publication: bool,
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
) -> TestResult<crate::OwnedApplication<f64>> {
    let (error, owner) = if publication {
        match heap.publish_application(owner, account) {
            ApplicationPublication::ApplicationRepair(error, owner)
            | ApplicationPublication::ApplicationRejected(error, owner) => (error, owner),
            other @ ApplicationPublication::ApplicationPublished(..) => {
                return Err(format!("{other:?}").into());
            }
        }
    } else {
        match owner.evaluate(account) {
            ApplicationEvaluation::ApplicationNotStarted(error, owner)
            | ApplicationEvaluation::ApplicationRejected(error, owner) => (error, owner),
            other @ ApplicationEvaluation::ApplicationCandidate(_) => {
                return Err(format!("{other:?}").into());
            }
        }
    };
    assert!(
        matches!(
            error,
            ApplicationError::Loan(
                LoanError::WrongLoan
                    | LoanError::UnsupportedContract
                    | LoanError::ForeignOwner
                    | LoanError::CoverageMismatch
                    | LoanError::TargetMismatch
            )
        ),
        "{error:?}"
    );
    Ok(owner)
}

/// Inventory and terminal outcome are composition requirements independent of ordinary readers.
#[test]
fn chain_inventory_and_receipt_pairings_refuse() -> TestResult {
    for choice in 0..3 {
        let (mut heap, mut account, tile, _) = product(1, 1)?;
        let request = request(
            &mut heap,
            tile.into_pin(),
            Elementwise::SquareRoot,
            2,
            &mut account,
        )?;
        let identity = request.input.ticket()?.identity;
        let group = heap.loans.groups[identity.root.slot]
            .as_mut()
            .ok_or("group")?;
        match choice {
            0 => group.debits = crate::loan::registry::DebitInventory::Host([None; 15]),
            1 => group.outcome = crate::ProducerOutcome::Pending,
            2 => {
                let crate::ProducerOutcome::Published(receipt) = &mut group.outcome else {
                    return Err("receipt".into());
                };
                receipt.present[2] = false;
            }
            _ => return Err("choice".into()),
        }
        let injection = fault::Injection::after(0);
        assert!(matches!(
            admit(&mut heap, request, Elementwise::SquareRoot, &mut account),
            ApplicationRequest::ApplicationRefused(ApplicationError::Loan(LoanError::WrongLoan), _)
        ));
        assert_eq!(fault::Injection::attempts(), 0);
        drop(injection);
    }
    Ok(())
}
