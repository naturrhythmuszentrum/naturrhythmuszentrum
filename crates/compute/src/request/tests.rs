// SPDX-License-Identifier: AGPL-3.0-only
//! Admission fixtures derived from the explicit design limits.

use super::*;

/// Explicit one-case fixture, not an instrument default.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Used by crate siblings through the owning module boundary."
)]
pub(crate) fn request() -> RunRequest {
    RunRequest {
        version: [1, 0, 0],
        run_id: "fixture".to_owned(),
        seeds: vec![Seed(1)],
        cases: vec![CaseRequest {
            operation: Operation::Add,
            width: Width::Scalar,
            dependency: Dependency::Dependent,
            scope: Scope::Prepared,
            shape: Vec::new(),
            item_length: 1,
            workers: 1,
            placement: Placement::Unbound,
            allowed_cpus: Vec::new(),
            budget_ns: 1_000_000,
            diagonal_margin: None,
            inputs: Vec::new(),
            cycles: Vec::new(),
        }],
        sampling: SamplingPolicy {
            samples: 10,
            resamples: 1000,
            confidence: Binary64(0.95_f64.to_bits()),
            warm_up_ns: 1,
            measurement_ns: 1,
            analysis_report_bytes: 65_536,
            analysis_scratch_bytes: 262_144,
        },
        resources: ResourcePolicy {
            budget_ns: 1_000_000,
            cases: 1,
            repetitions: 2,
            attempts: 1000,
            iterations_per_row: 100,
            operations_per_item: 1_000_000,
            buffer_bytes: 1_048_576,
            scratch_bytes: 8_388_608,
            report_bytes: 16_777_216,
            workers: 1,
            worker_stack_bytes: 1_048_576,
        },
    }
}

/// Explicit valid host assumptions; sums fit the outer declarations.
const fn bounds() -> ExecutionBounds {
    ExecutionBounds {
        uninterrupted_ns: 1000,
        finalization_ns: 1000,
        item_ns: 100,
        cancellation_read_ns: 1,
        sampler_uninterrupted_ns: 100,
        sampler_finalization_ns: 100,
        receipt_ns: 100,
        disposal_ns: 100,
        cleanup_ns: 100,
        report_ns: 100,
        wait_poll_ns: 100,
    }
}

#[test]
fn budget_and_bound_wire_refusals_precede_work() {
    let mut request = request();
    assert_eq!(request.validate(Some(bounds())), Ok(()));
    request.resources.budget_ns = MAX_INTEGER + 1;
    assert_eq!(
        request.validate(Some(bounds())).map_err(|e| e.cause),
        Err(RequestCause::Overflow)
    );
    request.resources.budget_ns = 1_000_000;
    for field in 0..3 {
        let mut bounds = bounds();
        match field {
            0 => bounds.uninterrupted_ns = MAX_INTEGER + 1,
            1 => bounds.finalization_ns = MAX_INTEGER + 1,
            _ => bounds.item_ns = MAX_INTEGER + 1,
        }
        assert_eq!(
            request.validate(Some(bounds)).map_err(|e| e.cause),
            Err(RequestCause::Overflow)
        );
    }
}

#[test]
fn independent_full_seeds_must_have_distinct_effective_offsets() {
    let mut request = request();
    request.seeds = vec![Seed(1), Seed(1_u128 << 64)];
    assert_eq!(
        request.validate(None).map_err(|e| e.cause),
        Err(RequestCause::Identity)
    );
    request.seeds = vec![Seed(1), Seed(2)];
    assert_eq!(request.validate(None), Ok(()));
}

#[test]
fn sampler_minima_and_open_confidence_are_preserved() {
    let mut request = request();
    request.sampling.samples = 9;
    assert_eq!(
        request.validate(None).map_err(|e| e.cause),
        Err(RequestCause::Policy)
    );
    request.sampling.samples = 10;
    request.sampling.resamples = 999;
    assert_eq!(
        request.validate(None).map_err(|e| e.cause),
        Err(RequestCause::Policy)
    );
    request.sampling.resamples = 1000;
    request.sampling.confidence = Binary64(1.0_f64.to_bits());
    assert_eq!(
        request.validate(None).map_err(|e| e.cause),
        Err(RequestCause::Policy)
    );
}

#[test]
fn finalization_components_add_instead_of_taking_maximum() {
    let request = request();
    let mut bounds = bounds();
    bounds.finalization_ns = 100;
    assert_eq!(
        request.validate(Some(bounds)).map_err(|e| e.cause),
        Err(RequestCause::Policy)
    );
}

#[test]
fn owned_request_clones_nested_identity_with_a_fallible_cap()
-> Result<(), Box<dyn std::error::Error>> {
    let mut source = request();
    source.cases[0].inputs = vec![vec![
        Binary64(1.0_f64.to_bits()),
        Binary64(2.0_f64.to_bits()),
    ]];
    source.cases[0].cycles = vec![-1, 0, 1];
    source.cases[0].allowed_cpus = vec![2, 4];
    source.cases[0].shape = vec![2, 3];
    source.cases[0].inputs[0].try_reserve_exact(1024)?;
    let owned = crate::memory::clone_request(
        &source,
        &mut crate::memory::Budget::new(source.resources.report_bytes),
    )?;
    assert_eq!(owned, source);
    source.cases[0].inputs[0][0] = Binary64(3.0_f64.to_bits());
    assert_eq!(owned.cases[0].inputs[0][0], Binary64(1.0_f64.to_bits()));
    source.resources.report_bytes = 1;
    assert_eq!(
        crate::memory::clone_request(
            &source,
            &mut crate::memory::Budget::new(source.resources.report_bytes)
        ),
        Err(crate::recipe::RecipeError::Allocation)
    );
    Ok(())
}

#[test]
fn capture_and_sampler_do_not_each_receive_the_whole_report_cap() -> Result<(), RequestError> {
    let request = request();
    crate::memory::preflight(&request, 0, 1)?;
    assert_eq!(
        crate::memory::preflight(&request, request.resources.report_bytes, 1).map_err(|e| e.cause),
        Err(RequestCause::Capacity),
    );
    let mut constrained = request;
    constrained.resources.scratch_bytes = constrained.sampling.analysis_scratch_bytes;
    constrained.resources.workers = 2;
    assert_eq!(
        crate::memory::preflight(&constrained, 0, 1).map_err(|e| e.cause),
        Err(RequestCause::Capacity)
    );
    Ok(())
}
