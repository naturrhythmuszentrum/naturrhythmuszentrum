// SPDX-License-Identifier: AGPL-3.0-only
//! Independent ordered binary values, correspondence, resources and both retaining edges.

mod bytes;
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
struct Fixture {
    /// Original live owner facade.
    heap: Heap,
    /// Immutable cumulative account.
    account: ExecutionAccount,
    /// Original two-pin request.
    request: PointwiseBinaryRequest,
}
/// Select the public entry point independently of the request's captured family.
fn admit(
    heap: &mut Heap,
    request: PointwiseBinaryRequest,
    function: BinaryElementwise,
    account: &mut ExecutionAccount,
) -> BinaryApplicationRequest {
    match function {
        BinaryElementwise::Product => heap.request_pointwise_product(request, account),
        BinaryElementwise::Quotient => heap.request_pointwise_quotient(request, account),
    }
}
/// No internal completion constructor is used by successful controls.
fn ready(result: BinaryApplicationRequest) -> TestResult<OwnedApplication<f64>> {
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
