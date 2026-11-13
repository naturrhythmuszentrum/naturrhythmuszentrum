// SPDX-License-Identifier: AGPL-3.0-only
//! Complete owner phases, with independent clocks and retained result identities.
use super::{Clock, Observation, Snapshot, Tracked, checkpoint, support, timed};
use nrz_bindings::readings::algebra::enclosure::prepared_expm1::{Counts, Outcome, Policy};
use nrz_matrix::{PreparedSigmoid, PreparedTanh, SigmoidPreparationPolicy, TanhPreparationPolicy};
use nrz_scalar::{ArithmeticContext, SigmoidInput, TanhInput};
use std::{cell::Cell, hint::black_box, rc::Rc};
use support::{Fixture, TestResult, broad, ledger, original};

/// Each cold and warm phase ends at checked basis authority, not owner import.
fn prepare(f: &mut Fixture, rows: &mut Vec<Observation>, family: &'static str) -> TestResult {
    let request = broad(f, 44)?;
    let mut b = ledger()?;
    for warm in [false, true] {
        let mut counts = Counts::default();
        let before = b.usage();
        let phase = if warm { "prepare-warm" } else { "prepare-cold" };
        checkpoint(
            phase,
            "setup",
            usize::from((f.cas.retained_judgment_bytes() > 0) == warm),
        );
        let (value, seconds) = timed(phase, || {
            let value = f
                .cas
                .prepare_expm1(&request, Policy::default(), &mut b, &mut counts)?;
            checkpoint(
                phase,
                "complete",
                usize::from(matches!(&value, Outcome::Ready(_))),
            );
            Ok(value)
        })?;
        let Outcome::Ready(authority) = value else {
            return Err("basis unavailable".into());
        };
        if warm && (counts.proposals != 0 || counts.checks != 0) {
            return Err("warm authority rebuilt".into());
        }
        if !warm && (counts.proposals == 0 || counts.checks == 0) {
            return Err("cold authority missing".into());
        }
        tracing::event!(name:"nrz.measure.counter",target:"nrz::work",tracing::Level::TRACE,family=if warm {"prepare-warm"}else{"prepare-cold"},operation=if warm {"cache hit"}else{"cache miss"},count=1_u64);
        rows.push(Observation::new(family,if warm {"prepare-warm"} else {"prepare-cold"},seconds,Some(b.usage().work-before.work),format!("before={before:?}; after={:?}; counts={counts:?}; degree={}; retained_judgment_bytes={}",b.usage(),authority.proof().degree,f.cas.retained_judgment_bytes())));
    }
    Ok(())
}

/// The fixed family path preserves every completion boundary and original request.
#[expect(
    clippy::too_many_lines,
    reason = "adjacent clocks retain the complete lifetime of one authority and its borrowed outputs"
)]
pub(super) fn sigmoid(values: &[f64]) -> TestResult<Vec<Observation>> {
    let context = ArithmeticContext::new()?;
    let mut f = Fixture::new()?;
    let request = original(&mut f, false)?;
    let mut rows = Vec::new();
    prepare(&mut f, &mut rows, "sigmoid")?;
    let mut budget = ledger()?;
    let mut counts = Counts::default();
    let before = budget.usage();
    let live = Rc::new(Cell::new(0));
    checkpoint("admit-sigmoid", "setup", 1);
    let (owner, seconds) = timed("admit-sigmoid", || {
        let value = budget.with_work_phase("measurement", "admit-sigmoid", |budget| {
            PreparedSigmoid::new(
                &mut f.cas,
                &request,
                [0.0, 2.0],
                40,
                SigmoidPreparationPolicy {
                    bits: 40,
                    requested_bits: 44,
                    revision: 1,
                },
                Policy::default(),
                &context,
                budget,
                &mut counts,
            )
            .map_err(|error| -> Box<dyn std::error::Error> { error.to_string().into() })
        })?;
        checkpoint("admit-sigmoid", "complete", 1);
        Ok(value)
    })?;
    let owner = Tracked::new(owner, &live);
    rows.push(Observation::new(
        "sigmoid",
        "admit-sigmoid",
        seconds,
        Some(budget.usage().work - before.work),
        format!(
            "before={before:?}; after={:?}; counts={counts:?}",
            budget.usage()
        ),
    ));
    let mut receiver = Fixture::new()?;
    let auxiliary = broad(&mut receiver, 44)?;
    let mut replay_budget = ledger()?;
    let mut replay_counts = Counts::default();
    checkpoint("replay-sigmoid", "setup", 1);
    let (checked, seconds) = timed("replay-sigmoid", || {
        let value = receiver.cas.check_expm1(
            &auxiliary,
            Policy::default(),
            owner.get()?.basis().proof(),
            &mut replay_budget,
            &mut replay_counts,
        )?;
        checkpoint("replay-sigmoid", "complete", 1);
        Ok(value)
    })?;
    if checked.proof() != owner.get()?.basis().proof() {
        return Err("replay evidence differs".into());
    }
    rows.push(Observation::new(
        "sigmoid",
        "replay-sigmoid",
        seconds,
        Some(replay_budget.usage().work),
        format!(
            "usage={:?}; counts={replay_counts:?}; retained_judgment_bytes={}",
            replay_budget.usage(),
            receiver.cas.retained_judgment_bytes()
        ),
    ));
    checkpoint("import-sigmoid", "setup", 1);
    let (inputs, seconds) = timed("import-sigmoid", || {
        let value: Vec<SigmoidInput> = values
            .iter()
            .map(|x| SigmoidInput {
                endpoints: [*x * 2.0, *x * 2.0],
                center: *x * 2.0,
                bits: 40,
            })
            .collect::<Vec<_>>();
        checkpoint("import-sigmoid", "complete", value.len());
        Ok(value)
    })?;
    rows.push(Observation::new(
        "sigmoid",
        "import-sigmoid",
        seconds,
        None,
        format!(
            "represented_points={}; materialized_bytes={}",
            inputs.len(),
            std::mem::size_of_val(inputs.as_slice())
        ),
    ));
    let before = budget.usage();
    let mut snapshots = Vec::with_capacity(inputs.len());
    checkpoint("native-sigmoid", "setup", snapshots.capacity());
    let ((), seconds) = timed("native-sigmoid", || {
        for input in &inputs {
            snapshots.push(Snapshot::Sigmoid(
                black_box(owner.get()?.evaluate(black_box(*input), &context)).map(|v| {
                    let v = v.reading();
                    [
                        v.endpoints[0],
                        v.endpoints[1],
                        v.center,
                        v.total_radius,
                        v.numerical_error,
                    ]
                    .map(f64::to_bits)
                }),
            ));
        }
        checkpoint("native-sigmoid", "complete", snapshots.len());
        Ok(())
    })?;
    if budget.usage().work != before.work {
        return Err("native exact Work".into());
    }
    let mut native = Observation::new(
        "sigmoid",
        "native-sigmoid",
        seconds,
        Some(0),
        format!(
            "before={before:?}; after={:?}; preallocated complete snapshots",
            budget.usage()
        ),
    );
    native.results = snapshots;
    rows.push(native);
    let before = budget.usage();
    checkpoint("batch-sigmoid", "setup", 1);
    let (outputs, seconds) = timed("batch-sigmoid", || {
        let value = owner
            .get()?
            .evaluate_batch(&inputs, &context, &mut budget)?;
        checkpoint("batch-sigmoid", "complete", value.len());
        Ok(value)
    })?;
    let outputs = Tracked::new(outputs, &live);
    rows.push(Observation::new(
        "sigmoid",
        "batch-sigmoid",
        seconds,
        Some(budget.usage().work - before.work),
        format!(
            "before={before:?}; after={:?}; materialized_outputs={}",
            budget.usage(),
            outputs.get()?.len()
        ),
    ));
    let mut snapshots = Vec::with_capacity(inputs.len());
    checkpoint("read-sigmoid", "setup", snapshots.capacity());
    let ((), seconds) = timed("read-sigmoid", || {
        for value in outputs.get()? {
            snapshots.push(Snapshot::Sigmoid(
                value
                    .as_ref()
                    .map(|v| {
                        let v = v.reading();
                        [
                            v.endpoints[0],
                            v.endpoints[1],
                            v.center,
                            v.total_radius,
                            v.numerical_error,
                        ]
                        .map(f64::to_bits)
                    })
                    .map_err(Clone::clone),
            ));
        }
        checkpoint("read-sigmoid", "complete", snapshots.len());
        Ok(())
    })?;
    if rows
        .iter()
        .find(|r| r.phase == "native-sigmoid")
        .ok_or("native row")?
        .results
        != snapshots
        || snapshots.len() != inputs.len()
    {
        return Err("batch association or materialization".into());
    }
    let mut reading = Observation::new(
        "sigmoid",
        "read-sigmoid",
        seconds,
        None,
        "consumed complete readings into preallocated snapshots".into(),
    );
    reading.results = snapshots;
    rows.push(reading);
    checkpoint("release-sigmoid", "setup", live.get() as usize);
    let clock = Clock::start("release-sigmoid");
    let seconds = {
        let _span =
            tracing::trace_span!("nrz.measure", family = "release-sigmoid", revision = 1_u64)
                .entered();
        drop(outputs);
        drop(owner);
        checkpoint("release-sigmoid", "complete", live.get() as usize);
        clock.stop()
    };
    if live.get() != 0 {
        return Err("release boundary retains output or owner".into());
    }
    rows.push(Observation::new(
        "sigmoid",
        "release-sigmoid",
        seconds,
        None,
        "materialized outputs and owner dropped; fixture graph remains outside boundary".into(),
    ));
    let mut platform = Vec::with_capacity(values.len());
    checkpoint("uncertified-sigmoid", "setup", platform.capacity());
    let ((), seconds) = timed("uncertified-sigmoid", || {
        for x in values {
            let x = black_box(*x * 2.0);
            platform.push(black_box(1.0 / (1.0 + (-x).exp())).to_bits());
        }
        checkpoint("uncertified-sigmoid", "complete", platform.len());
        Ok(())
    })?;
    let mut baseline=Observation::new("sigmoid","uncertified-sigmoid",seconds,None,"platform values consumed; preallocated snapshots; no engine call or certificate; not allocation-equivalent to batch".into());
    baseline.results = platform.into_iter().map(Snapshot::Platform).collect();
    rows.push(baseline);
    Ok(rows)
}

/// The fixed family path preserves every completion boundary and original request.
#[expect(
    clippy::too_many_lines,
    reason = "adjacent clocks retain the complete lifetime of one authority and its borrowed outputs"
)]
pub(super) fn tanh(values: &[f64]) -> TestResult<Vec<Observation>> {
    let context = ArithmeticContext::new()?;
    let mut f = Fixture::new()?;
    let request = original(&mut f, true)?;
    let mut rows = Vec::new();
    prepare(&mut f, &mut rows, "tanh")?;
    let mut budget = ledger()?;
    let mut counts = Counts::default();
    let before = budget.usage();
    let live = Rc::new(Cell::new(0));
    checkpoint("admit-tanh", "setup", 1);
    let (owner, seconds) = timed("admit-tanh", || {
        let value = budget.with_work_phase("measurement", "admit-tanh", |budget| {
            PreparedTanh::new(
                &mut f.cas,
                &request,
                [0.0, 1.0],
                40,
                TanhPreparationPolicy {
                    bits: 40,
                    requested_bits: 44,
                    revision: 1,
                },
                Policy::default(),
                &context,
                budget,
                &mut counts,
            )
            .map_err(|error| -> Box<dyn std::error::Error> { error.to_string().into() })
        })?;
        checkpoint("admit-tanh", "complete", 1);
        Ok(value)
    })?;
    let owner = Tracked::new(owner, &live);
    rows.push(Observation::new(
        "tanh",
        "admit-tanh",
        seconds,
        Some(budget.usage().work - before.work),
        format!(
            "before={before:?}; after={:?}; counts={counts:?}",
            budget.usage()
        ),
    ));
    let mut receiver = Fixture::new()?;
    let auxiliary = broad(&mut receiver, 44)?;
    let mut replay_budget = ledger()?;
    let mut replay_counts = Counts::default();
    checkpoint("replay-tanh", "setup", 1);
    let (checked, seconds) = timed("replay-tanh", || {
        let value = receiver.cas.check_expm1(
            &auxiliary,
            Policy::default(),
            owner.get()?.basis().proof(),
            &mut replay_budget,
            &mut replay_counts,
        )?;
        checkpoint("replay-tanh", "complete", 1);
        Ok(value)
    })?;
    if checked.proof() != owner.get()?.basis().proof() {
        return Err("replay evidence differs".into());
    }
    rows.push(Observation::new(
        "tanh",
        "replay-tanh",
        seconds,
        Some(replay_budget.usage().work),
        format!(
            "usage={:?}; counts={replay_counts:?}; retained_judgment_bytes={}",
            replay_budget.usage(),
            receiver.cas.retained_judgment_bytes()
        ),
    ));
    checkpoint("import-tanh", "setup", 1);
    let (inputs, seconds) = timed("import-tanh", || {
        let value: Vec<TanhInput> = values
            .iter()
            .map(|x| TanhInput {
                endpoints: [*x * 1.0, *x * 1.0],
                center: *x * 1.0,
                bits: 40,
            })
            .collect::<Vec<_>>();
        checkpoint("import-tanh", "complete", value.len());
        Ok(value)
    })?;
    rows.push(Observation::new(
        "tanh",
        "import-tanh",
        seconds,
        None,
        format!(
            "represented_points={}; materialized_bytes={}",
            inputs.len(),
            std::mem::size_of_val(inputs.as_slice())
        ),
    ));
    let before = budget.usage();
    let mut snapshots = Vec::with_capacity(inputs.len());
    checkpoint("native-tanh", "setup", snapshots.capacity());
    let ((), seconds) = timed("native-tanh", || {
        for input in &inputs {
            snapshots.push(Snapshot::Tanh(
                black_box(owner.get()?.evaluate(black_box(*input), &context)).map(|v| {
                    let v = v.reading();
                    [
                        v.endpoints[0],
                        v.endpoints[1],
                        v.center,
                        v.total_radius,
                        v.numerical_error,
                    ]
                    .map(f64::to_bits)
                }),
            ));
        }
        checkpoint("native-tanh", "complete", snapshots.len());
        Ok(())
    })?;
    if budget.usage().work != before.work {
        return Err("native exact Work".into());
    }
    let mut native = Observation::new(
        "tanh",
        "native-tanh",
        seconds,
        Some(0),
        format!(
            "before={before:?}; after={:?}; preallocated complete snapshots",
            budget.usage()
        ),
    );
    native.results = snapshots;
    rows.push(native);
    let before = budget.usage();
    checkpoint("batch-tanh", "setup", 1);
    let (outputs, seconds) = timed("batch-tanh", || {
        let value = owner
            .get()?
            .evaluate_batch(&inputs, &context, &mut budget)?;
        checkpoint("batch-tanh", "complete", value.len());
        Ok(value)
    })?;
    let outputs = Tracked::new(outputs, &live);
    rows.push(Observation::new(
        "tanh",
        "batch-tanh",
        seconds,
        Some(budget.usage().work - before.work),
        format!(
            "before={before:?}; after={:?}; materialized_outputs={}",
            budget.usage(),
            outputs.get()?.len()
        ),
    ));
    let mut snapshots = Vec::with_capacity(inputs.len());
    checkpoint("read-tanh", "setup", snapshots.capacity());
    let ((), seconds) = timed("read-tanh", || {
        for value in outputs.get()? {
            snapshots.push(Snapshot::Tanh(
                value
                    .as_ref()
                    .map(|v| {
                        let v = v.reading();
                        [
                            v.endpoints[0],
                            v.endpoints[1],
                            v.center,
                            v.total_radius,
                            v.numerical_error,
                        ]
                        .map(f64::to_bits)
                    })
                    .map_err(Clone::clone),
            ));
        }
        checkpoint("read-tanh", "complete", snapshots.len());
        Ok(())
    })?;
    if rows
        .iter()
        .find(|r| r.phase == "native-tanh")
        .ok_or("native row")?
        .results
        != snapshots
        || snapshots.len() != inputs.len()
    {
        return Err("batch association or materialization".into());
    }
    let mut reading = Observation::new(
        "tanh",
        "read-tanh",
        seconds,
        None,
        "consumed complete readings into preallocated snapshots".into(),
    );
    reading.results = snapshots;
    rows.push(reading);
    checkpoint("release-tanh", "setup", live.get() as usize);
    let clock = Clock::start("release-tanh");
    let seconds = {
        let _span = tracing::trace_span!("nrz.measure", family = "release-tanh", revision = 1_u64)
            .entered();
        drop(outputs);
        drop(owner);
        checkpoint("release-tanh", "complete", live.get() as usize);
        clock.stop()
    };
    if live.get() != 0 {
        return Err("release boundary retains output or owner".into());
    }
    rows.push(Observation::new(
        "tanh",
        "release-tanh",
        seconds,
        None,
        "materialized outputs and owner dropped; fixture graph remains outside boundary".into(),
    ));
    let mut platform = Vec::with_capacity(values.len());
    checkpoint("uncertified-tanh", "setup", platform.capacity());
    let ((), seconds) = timed("uncertified-tanh", || {
        for x in values {
            let x = black_box(*x * 1.0);
            platform.push(black_box(x.tanh()).to_bits());
        }
        checkpoint("uncertified-tanh", "complete", platform.len());
        Ok(())
    })?;
    let mut baseline=Observation::new("tanh","uncertified-tanh",seconds,None,"platform values consumed; preallocated snapshots; no engine call or certificate; not allocation-equivalent to batch".into());
    baseline.results = platform.into_iter().map(Snapshot::Platform).collect();
    rows.push(baseline);
    Ok(rows)
}
