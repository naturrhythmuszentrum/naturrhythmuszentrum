// SPDX-License-Identifier: AGPL-3.0-only
//! Block adapters: immutable preparation, private iteration accounting and consumed outputs.
use super::support::Snapshot;
use super::support::ledger;
use nrz_bindings::readings::algebra::{FaultKind, limits::Budget};
use nrz_matrix::{
    PreparedSigmoid, PreparedTanh, SigmoidEvaluation, TanhEvaluation,
    sampling::{
        AdapterBounds, AnalysisCaps, AnalysisError, AnalysisSettings, HostOperation, ReceiptStatus,
        Representative, SamplerBounds, SamplingDeadline, SamplingIdentity, SamplingSettings,
        WorkReceipt,
    },
};
use nrz_scalar::{ArithmeticContext, SigmoidFailure, SigmoidInput, TanhFailure, TanhInput};
use std::{hint::black_box, time::Duration};

/// Six separately named operations share one lifecycle implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Phase {
    /// Read every prepared sigmoid point.
    NativeSigmoid,
    /// Read every prepared tanh point.
    NativeTanh,
    /// Materialize and read the sigmoid block.
    BatchSigmoid,
    /// Materialize and read the tanh block.
    BatchTanh,
    /// Uncertified platform sigmoid block.
    PlatformSigmoid,
    /// Uncertified platform tanh block.
    PlatformTanh,
}
impl Phase {
    /// Stable identities, also used by the ordinary measurement harness.
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::NativeSigmoid => "native-sigmoid-reuse",
            Self::NativeTanh => "native-tanh-reuse",
            Self::BatchSigmoid => "batch-sigmoid",
            Self::BatchTanh => "batch-tanh",
            Self::PlatformSigmoid => "uncertified-sigmoid",
            Self::PlatformTanh => "uncertified-tanh",
        }
    }
    /// Platform operations have no engine ledger or invented Work evidence.
    const fn platform(self) -> bool {
        matches!(self, Self::PlatformSigmoid | Self::PlatformTanh)
    }
}

/// Both plans borrow separate frozen graphs for the entire sampling session.
#[derive(Debug)]
pub(super) struct Prepared<'a, 'g> {
    /// Frozen sigmoid authority.
    pub(super) sigmoid: &'a PreparedSigmoid<'g>,
    /// Frozen tanh authority.
    pub(super) tanh: &'a PreparedTanh<'g>,
    /// Admitted arithmetic environment.
    pub(super) arithmetic: &'a ArithmeticContext,
}

/// Original points and both represented operand arrays are prepared outside clocks.
#[derive(Debug)]
pub(super) struct Block {
    /// Original unsorted point population.
    pub(super) values: Vec<f64>,
    /// Preimported sigmoid operands.
    sigmoid: Vec<SigmoidInput>,
    /// Preimported tanh operands.
    tanh: Vec<TanhInput>,
}
impl Block {
    /// Import exact binary64 operands once, retaining unsorted population order.
    pub(super) fn new(values: Vec<f64>) -> Self {
        let sigmoid = values
            .iter()
            .map(|&x| SigmoidInput {
                endpoints: [2.0 * x, 2.0 * x],
                center: 2.0 * x,
                bits: 40,
            })
            .collect();
        let tanh = values
            .iter()
            .map(|&x| TanhInput {
                endpoints: [x, x],
                center: x,
                bits: 40,
            })
            .collect();
        Self {
            values,
            sigmoid,
            tanh,
        }
    }
}

/// Fixed-size original errors, with no formatting inside the timer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    /// Original engine exhaustion or arithmetic cause.
    Budget(FaultKind),
    /// Original sigmoid refusal.
    Sigmoid(SigmoidFailure),
    /// Original tanh refusal.
    Tanh(TanhFailure),
    /// Private output allocation failed.
    Allocation,
}

/// Engine evidence and platform non-participation are distinct from missing evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Work {
    /// Cumulative exact engine Work.
    Engine(u64),
    /// Platform execution does not participate in engine accounting.
    PlatformNoEngineWork,
}

/// One fresh ledger and bounded preallocated result storage per iteration.
#[derive(Debug)]
pub(super) struct Context {
    /// Sole execution ledger, absent for platform operations.
    budget: Option<Budget>,
    /// Ordered complete numerical readings.
    snapshots: Vec<Snapshot>,
    /// Ordered platform result bit patterns.
    baseline: Vec<u64>,
}

#[cfg(test)]
impl Context {
    /// Read actual scratch lengths and capacities at the callback boundary.
    pub(super) const fn scratch_layout(&self) -> [(usize, usize); 2] {
        [
            (self.snapshots.len(), self.snapshots.capacity()),
            (self.baseline.len(), self.baseline.capacity()),
        ]
    }
}

/// Storage whose destructor releases the retained batch evaluations.
#[derive(Debug)]
struct BatchOwners<T>(Vec<T>);
impl<T> Drop for BatchOwners<T> {
    fn drop(&mut self) {
        #[cfg(test)]
        let count = self.0.len();
        drop(std::mem::take(&mut self.0));
        #[cfg(test)]
        OWNERS_DROPPED.with(|n| n.set(n.get() + count));
    }
}

/// Ordinary batch owners survive receipt preservation and are released by dispose.
#[derive(Debug)]
enum Batch<'a> {
    /// Per-point execution retains no batch owners.
    None,
    /// Ordinary sigmoid batch owners.
    Sigmoid(BatchOwners<Result<SigmoidEvaluation<'a>, SigmoidFailure>>),
    /// Ordinary tanh batch owners.
    Tanh(BatchOwners<Result<TanhEvaluation<'a>, TanhFailure>>),
}

/// Complete results remain available until the sampler calls dispose.
#[derive(Debug)]
pub(super) struct Output<'a> {
    /// Ordered complete numerical readings.
    snapshots: Vec<Snapshot>,
    /// Ordered platform result bit patterns.
    baseline: Vec<u64>,
    /// Retained owners released only after receipt preservation.
    batch: Batch<'a>,
}

/// One phase adapter never owns mutable graph state or a preparation request.
#[derive(Debug)]
pub(super) struct Adapter {
    /// Selected reusable operation.
    pub(super) phase: Phase,
    /// Explicit host execution and finalization assumptions.
    pub(super) allowances: AdapterBounds,
}

impl<'a, 'g: 'a> HostOperation<Prepared<'a, 'g>, Block> for Adapter {
    type Context = Context;
    type Output = Output<'a>;
    type Error = Error;
    type Work = Work;
    fn bounds(&self) -> Option<AdapterBounds> {
        Some(self.allowances)
    }
    fn reset(
        &mut self,
        _: &Prepared<'a, 'g>,
        item: &Block,
        _: &SamplingDeadline<'_>,
    ) -> Result<Context, Error> {
        let budget = if self.phase.platform() {
            None
        } else {
            Some(ledger().map_err(|e| Error::Budget(e.kind))?)
        };
        let mut snapshots = Vec::new();
        let mut baseline = Vec::new();
        if self.phase.platform() {
            baseline
                .try_reserve_exact(item.values.len())
                .map_err(|_| Error::Allocation)?;
        } else {
            snapshots
                .try_reserve_exact(item.values.len())
                .map_err(|_| Error::Allocation)?;
        }
        Ok(Context {
            budget,
            snapshots,
            baseline,
        })
    }
    fn snapshot(&mut self, context: &Context, _: &SamplingDeadline<'_>) -> Option<Work> {
        Some(
            context
                .budget
                .as_ref()
                .map_or(Work::PlatformNoEngineWork, |budget| {
                    Work::Engine(budget.usage().work)
                }),
        )
    }
    fn run(
        &mut self,
        prepared: &Prepared<'a, 'g>,
        item: &Block,
        context: &mut Context,
        _: &SamplingDeadline<'_>,
    ) -> Result<Output<'a>, Error> {
        let batch = match self.phase {
            Phase::NativeSigmoid => {
                for input in &item.sigmoid {
                    let value = black_box(
                        evaluate_sigmoid(prepared, black_box(*input))
                            .map_err(Error::Sigmoid)?
                            .reading(),
                    );
                    context.snapshots.push(snapshot_sigmoid(Ok(value)));
                }
                Batch::None
            }
            Phase::NativeTanh => {
                for input in &item.tanh {
                    let value = black_box(
                        evaluate_tanh(prepared, black_box(*input))
                            .map_err(Error::Tanh)?
                            .reading(),
                    );
                    context.snapshots.push(snapshot_tanh(Ok(value)));
                }
                Batch::None
            }
            Phase::BatchSigmoid => {
                let budget = context.budget.as_mut().ok_or(Error::Allocation)?;
                let values = evaluate_sigmoid_batch(prepared, &item.sigmoid, budget)
                    .map_err(|e| Error::Budget(e.kind))?;
                for value in &values {
                    context.snapshots.push(snapshot_sigmoid(
                        value
                            .as_ref()
                            .map(|v| black_box(v.reading()))
                            .map_err(|e| *e),
                    ));
                }
                Batch::Sigmoid(BatchOwners(values))
            }
            Phase::BatchTanh => {
                let budget = context.budget.as_mut().ok_or(Error::Allocation)?;
                let values = evaluate_tanh_batch(prepared, &item.tanh, budget)
                    .map_err(|e| Error::Budget(e.kind))?;
                for value in &values {
                    context.snapshots.push(snapshot_tanh(
                        value
                            .as_ref()
                            .map(|v| black_box(v.reading()))
                            .map_err(|e| *e),
                    ));
                }
                Batch::Tanh(BatchOwners(values))
            }
            Phase::PlatformSigmoid | Phase::PlatformTanh => {
                for &input in &item.values {
                    let value = evaluate_platform(self.phase, input);
                    context.baseline.push(black_box(value).to_bits());
                }
                Batch::None
            }
        };
        Ok(Output {
            snapshots: std::mem::take(&mut context.snapshots),
            baseline: std::mem::take(&mut context.baseline),
            batch,
        })
    }
    fn receipt(
        &mut self,
        context: &Context,
        _: &Result<Output<'a>, Error>,
        deadline: &SamplingDeadline<'_>,
    ) -> WorkReceipt<Work> {
        let exit = self.snapshot(context, deadline);
        // These routes create no nested/private engine ledger. This is the sole
        // execution ledger, including batch preflight; native loops charge none.
        // Platform routes declare non-participation, not a numeric engine counter.
        WorkReceipt {
            exit,
            operation: exit,
            status: ReceiptStatus::Complete,
        }
    }
    fn dispose(
        &mut self,
        output: Result<Output<'a>, Error>,
        _: &SamplingDeadline<'_>,
    ) -> Result<(), Error> {
        if let Ok(mut output) = output {
            // Release ordinary owners only after preserving their evidence.
            match std::mem::replace(&mut output.batch, Batch::None) {
                Batch::None => {}
                Batch::Sigmoid(values) => drop(values),
                Batch::Tanh(values) => drop(values),
            }
            drop(std::mem::take(&mut output.snapshots));
            drop(std::mem::take(&mut output.baseline));
            drop(output);
        }
        Ok(())
    }
    fn cleanup(&mut self, context: Context, _: &SamplingDeadline<'_>) -> Result<(), Error> {
        drop(context);
        Ok(())
    }
}

/// An item is the complete population; element throughput is the point rate.
pub(super) const fn representative<'a>(
    block: &'a Block,
    phase: Phase,
    shape: &'a [usize; 1],
) -> Representative<'a, Block> {
    Representative {
        item: block,
        elements_per_item: Some(block.values.len() as u64),
        identity: SamplingIdentity {
            representative: phase.name(),
            shape,
            operation_revision: "prepared-block-v1",
            workload: "unsorted-prepared-binary64-40-bit",
            scope: if phase.platform() {
                concat!(
                    "whole block, consumed platform results; no engine ledger; ",
                    "not an allocation-equivalent baseline for batch evaluation"
                )
            } else {
                concat!(
                    "whole block, consumed complete readings; ",
                    "fresh execution ledger; empty registration inventory; ",
                    "batch includes allocation and reading"
                )
            },
            trace_configuration: "disabled",
        },
    }
}

/// Explicit, printed host assumptions, including allocation and complete cleanup.
pub(super) const fn allowances() -> AdapterBounds {
    AdapterBounds {
        uninterrupted: CALLBACK_ALLOWANCE,
        receipt: CALLBACK_ALLOWANCE,
        disposal: CALLBACK_ALLOWANCE,
        cleanup: CALLBACK_ALLOWANCE,
    }
}

/// Host assumption per callback and sampler uninterrupted/finalization segment.
const CALLBACK_ALLOWANCE: Duration = Duration::from_secs(1);
/// Positive timed warm-up target.
const WARM_UP: Duration = Duration::from_millis(10);
/// Timed measurement target before rounding the linear schedule.
const MEASUREMENT: Duration = Duration::from_millis(50);
/// Cooperative wall deadline covering collection and analysis.
const WALL_BUDGET: Duration = Duration::from_secs(30);
/// Finite total attempted iterations, including warm-up.
const MAX_TOTAL_ITERATIONS: u64 = 16_384;
/// Finite iterations in each sample or warm-up group.
const MAX_ITERATIONS_PER_SAMPLE: u64 = 4096;
/// Number of complete linear-schedule samples.
const SAMPLES: usize = 10;
/// Deterministic within-process bootstrap count.
const RESAMPLES: usize = 1000;
/// Nominal bootstrap coverage.
const CONFIDENCE: f64 = 0.95;
/// Explicit deterministic analysis seed.
const ANALYSIS_SEED: u128 = 20_261_113;
/// Separate bounded analysis allocations.
const ANALYSIS_CAPS: AnalysisCaps = AnalysisCaps {
    report_bytes: 65_536,
    scratch_bytes: 262_144,
};
/// Separate bounded collection allocations.
const COLLECTION_CAPS: AnalysisCaps = AnalysisCaps {
    report_bytes: 16_777_216,
    scratch_bytes: 262_144,
};

/// Named finite settings; no machine-derived defaults or calibration policy.
pub(super) fn settings() -> Result<SamplingSettings, AnalysisError> {
    Ok(SamplingSettings {
        analysis: AnalysisSettings::new(
            SAMPLES,
            RESAMPLES,
            CONFIDENCE,
            ANALYSIS_SEED,
            ANALYSIS_CAPS,
        )?,
        warm_up: WARM_UP,
        measurement: MEASUREMENT,
        wall_budget: WALL_BUDGET,
        max_total_iterations: MAX_TOTAL_ITERATIONS,
        max_iterations_per_sample: MAX_ITERATIONS_PER_SAMPLE,
        caps: COLLECTION_CAPS,
        bounds: SamplerBounds {
            uninterrupted: CALLBACK_ALLOWANCE,
            finalization: CALLBACK_ALLOWANCE,
        },
    })
}

/// Copy complete endpoint, centre and error evidence inside the timed operation.
fn snapshot_sigmoid(value: Result<nrz_scalar::SigmoidOutput, SigmoidFailure>) -> Snapshot {
    Snapshot::Sigmoid(value.map(|v| {
        [
            v.endpoints[0],
            v.endpoints[1],
            v.center,
            v.total_radius,
            v.numerical_error,
        ]
        .map(f64::to_bits)
    }))
}
/// Copy complete endpoint, centre and error evidence inside the timed operation.
fn snapshot_tanh(value: Result<nrz_scalar::TanhOutput, TanhFailure>) -> Snapshot {
    Snapshot::Tanh(value.map(|v| {
        [
            v.endpoints[0],
            v.endpoints[1],
            v.center,
            v.total_radius,
            v.numerical_error,
        ]
        .map(f64::to_bits)
    }))
}

#[cfg(test)]
std::thread_local! {
    /// Real output destruction, not a detached lifetime token.
    pub(super) static DROPPED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
#[cfg(test)]
impl Drop for Output<'_> {
    fn drop(&mut self) {
        drop(std::mem::take(&mut self.snapshots));
        drop(std::mem::take(&mut self.baseline));
        drop(std::mem::replace(&mut self.batch, Batch::None));
        DROPPED.with(|count| count.set(count.get() + 1));
    }
}
#[cfg(test)]
impl Output<'_> {
    /// Complete consumed output while owners are still live before receipt.
    pub(super) fn readings(&self) -> (&[Snapshot], &[u64]) {
        (&self.snapshots, &self.baseline)
    }
}

#[cfg(test)]
impl Output<'_> {
    /// Batch ownership is still live at timer stop and receipt.
    pub(super) fn owners(&self) -> usize {
        match &self.batch {
            Batch::None => 0,
            Batch::Sigmoid(v) => v.0.len(),
            Batch::Tanh(v) => v.0.len(),
        }
    }
}

/// Native call boundary records the operand only in test builds.
fn evaluate_sigmoid<'a>(
    prepared: &Prepared<'a, '_>,
    input: SigmoidInput,
) -> Result<SigmoidEvaluation<'a>, SigmoidFailure> {
    #[cfg(test)]
    evaluation(input.center / 2.0);
    prepared.sigmoid.evaluate(input, prepared.arithmetic)
}
/// Native call boundary records the operand only in test builds.
fn evaluate_tanh<'a>(
    prepared: &Prepared<'a, '_>,
    input: TanhInput,
) -> Result<TanhEvaluation<'a>, TanhFailure> {
    #[cfg(test)]
    evaluation(input.center);
    prepared.tanh.evaluate(input, prepared.arithmetic)
}
/// Batch call boundary records the exact supplied traversal order in test builds.
fn evaluate_sigmoid_batch<'a>(
    prepared: &Prepared<'a, '_>,
    inputs: &[SigmoidInput],
    budget: &mut Budget,
) -> Result<
    Vec<Result<SigmoidEvaluation<'a>, SigmoidFailure>>,
    nrz_bindings::readings::algebra::Fault,
> {
    #[cfg(test)]
    for input in inputs {
        evaluation(input.center / 2.0);
    }
    prepared
        .sigmoid
        .evaluate_batch(inputs, prepared.arithmetic, budget)
}
/// Batch call boundary records the exact supplied traversal order in test builds.
fn evaluate_tanh_batch<'a>(
    prepared: &Prepared<'a, '_>,
    inputs: &[TanhInput],
    budget: &mut Budget,
) -> Result<Vec<Result<TanhEvaluation<'a>, TanhFailure>>, nrz_bindings::readings::algebra::Fault> {
    #[cfg(test)]
    for input in inputs {
        evaluation(input.center);
    }
    prepared
        .tanh
        .evaluate_batch(inputs, prepared.arithmetic, budget)
}
/// Platform arithmetic is observable at its actual call boundary in test builds.
fn evaluate_platform(phase: Phase, input: f64) -> f64 {
    #[cfg(test)]
    evaluation(input);
    if phase == Phase::PlatformSigmoid {
        1.0 / (1.0 + (-2.0 * black_box(input)).exp())
    } else {
        black_box(input).tanh()
    }
}
#[cfg(test)]
std::thread_local! {
    /// Callback attribution is supplied by the independent lifecycle driver.
    pub(super) static CALLBACK: std::cell::Cell<&'static str> = const { std::cell::Cell::new("outside") };
    /// Actual arithmetic calls, in the order supplied to each operation.
    pub(super) static EVALUATIONS: std::cell::RefCell<Vec<(&'static str, u64)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Real retained batch storage released, separately from Output destruction.
    pub(super) static OWNERS_DROPPED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
/// A sampling call owns only its own test attribution inventory.
#[cfg(test)]
pub(super) fn begin_call() {
    EVALUATIONS.with(|events| events.borrow_mut().clear());
    CALLBACK.with(|current| current.set("outside"));
}
#[cfg(test)]
fn evaluation(input: f64) {
    EVALUATIONS.with(|events| {
        events
            .borrow_mut()
            .push((CALLBACK.with(std::cell::Cell::get), input.to_bits()));
    });
}
