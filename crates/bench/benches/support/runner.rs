// SPDX-License-Identifier: AGPL-3.0-only
//! Prepared native exponential and trivial-call observations in both sampling hosts.

use super::{
    BenchResult, fixture, observe,
    report::{compare, export, replay},
};
use criterion::{Criterion, SamplingMode};
use nrz_bindings::readings::algebra::enclosure::prepared_expm1::{Counts, Policy};
use nrz_matrix::PreparedExpm1;
use nrz_scalar::{ArithmeticContext, Expm1Input};
use std::{convert::Infallible, hint::black_box};

/// Identical minimal payload in both timers, with no numerical or Work claim.
const fn trivial() -> u64 {
    black_box(black_box(17_u64))
}

/// Preparation, preflight and host reporting remain outside every timed closure.
pub(super) fn run() -> BenchResult {
    let mut fixture = fixture::Fixture::new()?;
    let request = fixture::broad(&mut fixture, 40)?;
    let arithmetic = ArithmeticContext::new()?;
    let plan = PreparedExpm1::new(
        &mut fixture.cas,
        &request,
        [-2.0, 0.0],
        Policy::default(),
        &arithmetic,
        &mut fixture::ledger()?,
        &mut Counts::default(),
    )?;
    let input = Expm1Input {
        endpoints: [-0.25, -0.25],
        center: -0.25,
        bits: 40,
    };
    // The existing independently checked operation succeeds before timing starts.
    let expected = plan.evaluate(input, &arithmetic)?.reading();
    let evaluate = || plan.evaluate(black_box(input), black_box(&arithmetic));
    let before = observe("prepared-expm1", "before-criterion", evaluate)?;
    let empty_before = observe("trivial-identity", "before-criterion", || {
        Ok::<_, Infallible>(trivial())
    })?;
    export("before-criterion", &before)?;
    export("before-criterion", &empty_before)?;
    let fixed = replay(&before)?;
    let empty_fixed = replay(&empty_before)?;

    let mut criterion = Criterion::default()
        .sample_size(20)
        .nresamples(2_000)
        .without_plots()
        .configure_from_args();
    {
        let mut group = criterion.benchmark_group("host-sampling");
        group.sampling_mode(SamplingMode::Linear);
        group.bench_function("prepared-expm1", |bencher| bencher.iter(evaluate));
        group.bench_function("trivial-identity", |bencher| bencher.iter(trivial));
        group.finish();
    }

    let after = observe("prepared-expm1", "after-criterion", evaluate)?;
    let empty_after = observe("trivial-identity", "after-criterion", || {
        Ok::<_, Infallible>(trivial())
    })?;
    export("after-criterion", &after)?;
    export("after-criterion", &empty_after)?;
    compare(&before, &after, &fixed)?;
    compare(&empty_before, &empty_after, &empty_fixed)?;
    let retained = plan.evaluate(input, &arithmetic)?.reading();
    if retained.center.to_bits() != expected.center.to_bits() {
        return Err("prepared operation changed across benchmark hosts".into());
    }
    criterion.final_summary();
    drop(criterion);
    Ok(())
}
