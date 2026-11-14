// SPDX-License-Identifier: AGPL-3.0-only
//! Independent ordered binary values, correspondence, resources and both retaining edges.

mod broadcast;
mod bytes;
mod cartesian;
mod chain;
mod constructor_bytes;
mod correspondence;
mod fixture;
mod observation;
mod oracle;
mod ownership;
mod physical;
mod reclaim_work;
mod resources;
mod seals;
mod spare;
mod values;
mod work;

use super::{TestResult, candidate, published, requested};
use crate::{
    BinaryApplicationRequest, BinaryElementwise, ExecutionAccount, Heap, OwnedApplication,
    PointwiseBinaryRequest,
};
use fixture::{custom, fixture};
use nrz_bindings::Limits;

/// Both published inputs and their sole ordinary demand precede the binary request.
struct Fixture<P: nrz_scalar::Scalar = f64> {
    /// Original live owner facade.
    heap: Heap,
    /// Immutable cumulative account.
    account: ExecutionAccount,
    /// Original two-pin request.
    request: PointwiseBinaryRequest<P>,
}
/// Select the public entry point independently of the request's captured family.
fn admit<P: nrz_scalar::Scalar + 'static>(
    heap: &mut Heap,
    request: PointwiseBinaryRequest<P>,
    function: BinaryElementwise,
    account: &mut ExecutionAccount,
) -> BinaryApplicationRequest<P> {
    match function {
        BinaryElementwise::Product => heap.request_pointwise_product(request, account),
        BinaryElementwise::Quotient => heap.request_pointwise_quotient(request, account),
    }
}
/// No internal completion constructor is used by successful controls.
fn ready<P: nrz_scalar::Scalar>(
    result: BinaryApplicationRequest<P>,
) -> TestResult<OwnedApplication<P>> {
    match result {
        BinaryApplicationRequest::ApplicationReady(owner) => Ok(owner),
        other @ BinaryApplicationRequest::ApplicationRefused(..) => {
            Err(format!("{other:?}").into())
        }
    }
}
/// Spend only the original immutable allowance, retaining all prior usage.
fn leave(account: &mut ExecutionAccount, remaining: u64) -> TestResult {
    account.ledger.work(
        account.limits().work - account.usage().work - remaining,
        crate::loan::AT,
    )?;
    Ok(())
}

/// Fixture values are exact dyadics in both formats; refuse accidental rounded test data.
fn native<P: nrz_scalar::Scalar>(value: f64) -> P {
    let converted = P::parse_nearest(&value.to_string()).expect("finite fixture");
    assert_eq!(converted.to_f64(), value);
    converted
}

/// Withdraw registry aliases while substituting a test seal, then restore both aliases exactly.
fn alter<P: nrz_scalar::Scalar + 'static>(
    heap: &mut crate::Heap,
    pin: &mut crate::BundlePin<P>,
    change: impl FnOnce(&mut crate::loan::registry::Bundle) -> TestResult,
) -> TestResult {
    use std::rc::Rc;
    let root = pin.ticket()?.identity.root.slot;
    let pin_slot = pin.record.payload.as_ref().ok_or("pin")?.index;
    let crate::loan::registry::State::Published(owner) =
        &heap.loans.groups[root].as_ref().ok_or("group")?.state
    else {
        return Err("published group".into());
    };
    assert!(Rc::ptr_eq(
        owner,
        &pin.record.payload.as_ref().ok_or("pin")?.bundle
    ));
    let mut index = heap.loans.pins[pin_slot].take().ok_or("pin index")?;
    index.witness = None;
    heap.loans.groups[root].as_mut().ok_or("group")?.state = crate::loan::registry::State::Reserved;
    let payload = Rc::get_mut(&mut pin.record)
        .and_then(|record| record.payload.as_mut())
        .ok_or("unique pin")?;
    let bundle = Rc::get_mut(&mut payload.bundle)
        .and_then(|owner| owner.payload.as_mut())
        .ok_or("unique bundle")?;
    change(bundle)?;
    heap.loans.groups[root].as_mut().ok_or("group")?.state =
        crate::loan::registry::State::Published(Rc::clone(&payload.bundle));
    index.witness = Some(Rc::downgrade(&pin.record));
    heap.loans.pins[pin_slot] = Some(index);
    Ok(())
}
