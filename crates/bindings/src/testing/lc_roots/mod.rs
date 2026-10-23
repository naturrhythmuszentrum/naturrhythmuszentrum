// SPDX-License-Identifier: AGPL-3.0-only
//! Public CAS construction of the unspecialized original LC determinant for bindings tests.

#[path = "mutations.rs"]
pub(crate) mod mutations;

#[path = "proposal/mod.rs"]
mod proposal;

#[path = "family.rs"]
mod family;
#[path = "request.rs"]
mod request;

use nrz_bindings::readings::algebra::{
    Cas, Handle, Reference,
    limits::{Budget, Limits, LimitsOverride},
    request::{Outcome, Request},
    syntax::{AtomicSort, Draft, ExpressionDraft, Field, Operator, Sort},
};

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Exact construction state; every operation uses the supplied cumulative ledger.
pub(crate) struct Fixture {
    pub(crate) cas: Cas,
    pub(crate) header: Handle,
    pub(crate) real: Handle,
    pub(crate) complex: Handle,
    pub(crate) complex_space: Handle,
}

/// Original declarations, strict base guards and determinant with four divisor occurrences.
pub(crate) fn original(signs: [i8; 2], budget: &mut Budget) -> TestResult<(Fixture, Request)> {
    changed(signs, [1, 1], false, budget)
}

/// Independently rebuild a changed physical coefficient or an invalid operand beneath zero.
pub(crate) fn changed(
    signs: [i8; 2],
    scales: [i64; 2],
    invalid_zero: bool,
    budget: &mut Budget,
) -> TestResult<(Fixture, Request)> {
    let mut fixture = family::store(budget)?;
    let frame = family::build(&mut fixture, signs, budget)?;
    let request = request::build(&mut fixture, &frame, scales, invalid_zero, budget)?;
    Ok((fixture, request))
}

/// Existing finite CAS acceptance profile, never changed by the bindings adapter.
pub(crate) fn ledger() -> TestResult<Budget> {
    Ok(Budget::new(Limits::blind_cubic())?)
}

/// Preserve original graph ownership for every admitted reference.
pub(crate) fn reference(value: &Handle) -> Reference {
    Reference::Existing(value.clone())
}

/// Public graph admission only; no cross-crate source fixtures or private CAS state.
pub(crate) fn admit(cas: &mut Cas, value: Draft, budget: &mut Budget) -> TestResult<Handle> {
    cas.admit(&[value], budget)?
        .pop()
        .ok_or_else(|| "LC admission".into())
}

/// Explicit Real or Complex scalar syntax without implicit field coercion.
pub(crate) fn expression(
    fixture: &mut Fixture,
    field: Field,
    operator: Operator,
    operands: &[Handle],
    budget: &mut Budget,
) -> TestResult<Handle> {
    admit(
        &mut fixture.cas,
        Draft::Expression(ExpressionDraft {
            header: reference(&fixture.header),
            scope: Vec::new(),
            operator,
            sort: Sort::Value(AtomicSort::Scalar(field)),
            operands: operands.iter().map(reference).collect(),
            space: None,
            binding: None,
        }),
        budget,
    )
}

/// Generate the original theorem; generation and export retain separate ledgers.
pub(crate) struct Theorem {
    pub(crate) fixture: Fixture,
    pub(crate) request: Request,
    pub(crate) set: Handle,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) fn proof() -> TestResult<Theorem> {
    let (mut fixture, request) = original([1, 1], &mut ledger()?)?;
    let outcome = fixture.cas.run(&request);
    let Outcome::Solved { answer, usage, .. } = outcome else {
        return Err(format!("LC generation: {outcome:?}").into());
    };
    assert_eq!(usage.work, 838_465);
    let witness = answer.coverage.ok_or("LC coverage")?;
    let bytes = fixture.cas.export_witness(&witness, &mut ledger()?)?;
    assert_eq!(bytes.len(), 6_140);
    Ok(Theorem {
        fixture,
        request,
        set: answer.set,
        bytes,
    })
}

/// Rebuild candidate syntax only; the receiver contains no checked proof before replay.
pub(crate) fn proposal(fixture: &mut Fixture, theorem: &Theorem) -> TestResult<Handle> {
    proposal::copy(fixture, theorem, &mut ledger()?)
}

/// Preserve the unchanged upstream original-LC generation policy.
fn profile() -> LimitsOverride {
    let limits = Limits::blind_cubic();
    LimitsOverride {
        arithmetic_steps: Some(limits.arithmetic_steps),
        work: Some(limits.work),
        replay_work: Some(limits.replay_work),
        component_bits: Some(limits.component_bits),
        intermediate_bits: Some(limits.intermediate_bits),
        coefficient_ops: Some(limits.coefficient_ops),
        gcd_steps: Some(limits.gcd_steps),
        tower_reductions: Some(limits.tower_reductions),
        orbit_permutations: Some(limits.orbit_permutations),
        orbit_terms: Some(limits.orbit_terms),
        symmetric_steps: Some(limits.symmetric_steps),
        tower_levels: Some(limits.tower_levels),
        tower_degree: Some(limits.tower_degree),
        orbit_members: Some(limits.orbit_members),
        resolvent_depth: Some(limits.resolvent_depth),
        resolvent_degree: Some(limits.resolvent_degree),
        selection_slots: Some(limits.selection_slots),
        solution_branches: Some(limits.solution_branches),
        witness_bytes: Some(limits.witness_bytes),
        output_bytes: Some(limits.output_bytes),
        condition_atoms: Some(limits.condition_atoms),
        condition_steps: Some(limits.condition_steps),
        ..LimitsOverride::default()
    }
}
