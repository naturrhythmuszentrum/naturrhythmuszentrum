// SPDX-License-Identifier: AGPL-3.0-only
//! Preparation is outside sampling; owners borrow the retained fixture graphs.
use super::support::{Fixture, TestResult, broad, ledger};
use nrz_bindings::readings::algebra::enclosure::EnclosureRequest;
use nrz_bindings::readings::algebra::{
    enclosure::prepared_expm1::{Counts, Policy},
    limits::Budget,
};
use nrz_matrix::{PreparedSigmoid, PreparedTanh, SigmoidPreparationPolicy, TanhPreparationPolicy};
use nrz_scalar::ArithmeticContext;

/// Match the ordinary harness's cold/warm basis lifecycle before owner admission.
fn warm_basis(fixture: &mut Fixture) -> TestResult {
    let request = broad(fixture, 44)?;
    let mut budget = ledger()?;
    for _ in 0..2 {
        drop(fixture.cas.prepare_expm1(
            &request,
            Policy::default(),
            &mut budget,
            &mut Counts::default(),
        )?);
    }
    Ok(())
}
/// Retain the sigmoid graph and plan, never an output or evaluation cache.
pub(super) fn sigmoid<'g>(
    fixture: &'g mut Fixture,
    request: &'g EnclosureRequest,
    arithmetic: &ArithmeticContext,
    budget: &mut Budget,
) -> TestResult<PreparedSigmoid<'g>> {
    warm_basis(fixture)?;
    PreparedSigmoid::new(
        &mut fixture.cas,
        request,
        [0.0, 2.0],
        40,
        SigmoidPreparationPolicy {
            bits: 40,
            requested_bits: 44,
            revision: 1,
        },
        Policy::default(),
        arithmetic,
        budget,
        &mut Counts::default(),
    )
    .map_err(|error| error.to_string().into())
}
/// Retain the tanh graph and plan, never an output or evaluation cache.
pub(super) fn tanh<'g>(
    fixture: &'g mut Fixture,
    request: &'g EnclosureRequest,
    arithmetic: &ArithmeticContext,
    budget: &mut Budget,
) -> TestResult<PreparedTanh<'g>> {
    warm_basis(fixture)?;
    PreparedTanh::new(
        &mut fixture.cas,
        request,
        [0.0, 1.0],
        40,
        TanhPreparationPolicy {
            bits: 40,
            requested_bits: 44,
            revision: 1,
        },
        Policy::default(),
        arithmetic,
        budget,
        &mut Counts::default(),
    )
    .map_err(|error| error.to_string().into())
}
