// SPDX-License-Identifier: AGPL-3.0-only
//! Block adapters: immutable preparation, private iteration accounting and consumed outputs.
use super::common::{Snapshot, snapshot_exp, snapshot_expm1};
use super::support::ledger;
use nrz_bindings::readings::algebra::{FaultKind, limits::Budget};
use nrz_matrix::{
    ExpEvaluation, Expm1Evaluation, PreparedExp, PreparedExpm1,
    sampling::{
        AdapterBounds, AnalysisCaps, AnalysisError, AnalysisSettings, HostOperation, ReceiptStatus,
        Representative, SamplerBounds, SamplingDeadline, SamplingIdentity, SamplingSettings,
        WorkReceipt,
    },
};
use nrz_scalar::{ArithmeticContext, ExpFailure, ExpInput, Expm1Failure, Expm1Input};
use std::{hint::black_box, time::Duration};

/// Six separately named operations share one lifecycle implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Phase {
    /// Read every prepared expm1 point.
    NativeExpm1,
    /// Read every prepared exp point.
    NativeExp,
    /// Materialize and read the expm1 block.
    BatchExpm1,
    /// Materialize and read the exp block.
    BatchExp,
    /// Uncertified platform expm1 block.
    PlatformExpm1,
    /// Uncertified platform exp block.
    PlatformExp,
}
impl Phase {
    /// Stable identities, also used by the ordinary measurement harness.
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::NativeExpm1 => "native-expm1-reuse",
            Self::NativeExp => "native-exp-reuse",
            Self::BatchExpm1 => "batch-expm1",
            Self::BatchExp => "batch-exp",
            Self::PlatformExpm1 => "uncertified-expm1",
            Self::PlatformExp => "uncertified-exp",
        }
    }
    /// Platform operations have no engine ledger or invented Work evidence.
    const fn platform(self) -> bool {
        matches!(self, Self::PlatformExpm1 | Self::PlatformExp)
    }
}

/// Both plans borrow separate frozen graphs for the entire sampling session.
#[derive(Debug)]
pub(super) struct Prepared<'a, 'g> {
    /// Frozen expm1 authority.
    pub(super) expm1: &'a PreparedExpm1<'g>,
    /// Frozen exp authority.
    pub(super) exp: &'a PreparedExp<'g>,
    /// Admitted arithmetic environment.
    pub(super) arithmetic: &'a ArithmeticContext,
}

/// Original points and both represented operand arrays are prepared outside clocks.
#[derive(Debug)]
pub(super) struct Block {
    /// Original unsorted point population.
    pub(super) values: Vec<f64>,
    /// Preimported expm1 operands.
    expm1: Vec<Expm1Input>,
    /// Preimported exp operands.
    exp: Vec<ExpInput>,
}
impl Block {
    /// Import exact binary64 operands once, retaining unsorted population order.
    pub(super) fn new(values: Vec<f64>) -> Self {
        let expm1 = values
            .iter()
            .map(|&x| Expm1Input {
                endpoints: [x, x],
                center: x,
                bits: 40,
            })
            .collect();
        let exp = values
            .iter()
            .map(|&x| ExpInput {
                endpoints: [x, x],
                center: x,
                bits: 40,
            })
            .collect();
        Self { values, expm1, exp }
    }
}

/// Fixed-size original errors, with no formatting inside the timer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    /// Original engine exhaustion or arithmetic cause.
    Budget(FaultKind),
    /// Original expm1 refusal.
    Expm1(Expm1Failure),
    /// Original exp refusal.
    Exp(ExpFailure),
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

/// Ordinary batch owners survive receipt preservation and are released by dispose.
#[derive(Debug)]
enum Batch<'a> {
    /// Per-point execution retains no batch owners.
    None,
    /// Ordinary expm1 batch owners.
    Expm1(Vec<Result<Expm1Evaluation<'a>, Expm1Failure>>),
    /// Ordinary exp batch owners.
    Exp(Vec<Result<ExpEvaluation<'a>, ExpFailure>>),
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

/// Optional complete-result consumer, invoked after timing.
type Inspect = fn(&[Snapshot], &[u64]);

/// One phase adapter never owns mutable graph state or a preparation request.
#[derive(Debug)]
pub(super) struct Adapter {
    /// Selected reusable operation.
    pub(super) phase: Phase,
    /// Explicit host execution and finalization assumptions.
    pub(super) allowances: AdapterBounds,
    /// Optional test-side sink runs after the clock, before output disposal.
    pub(super) inspect: Option<Inspect>,
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
            Phase::NativeExpm1 => {
                for input in &item.expm1 {
                    let value = black_box(
                        prepared
                            .expm1
                            .evaluate(black_box(*input), prepared.arithmetic)
                            .map_err(Error::Expm1)?
                            .reading(),
                    );
                    context.snapshots.push(snapshot_expm1(Ok(value)));
                }
                Batch::None
            }
            Phase::NativeExp => {
                for input in &item.exp {
                    let value = black_box(
                        prepared
                            .exp
                            .evaluate(black_box(*input), prepared.arithmetic)
                            .map_err(Error::Exp)?
                            .reading(),
                    );
                    context.snapshots.push(snapshot_exp(Ok(value)));
                }
                Batch::None
            }
            Phase::BatchExpm1 => {
                let budget = context.budget.as_mut().ok_or(Error::Allocation)?;
                let values = prepared
                    .expm1
                    .evaluate_batch(&item.expm1, prepared.arithmetic, budget)
                    .map_err(|e| Error::Budget(e.kind))?;
                for value in &values {
                    context.snapshots.push(snapshot_expm1(
                        value
                            .as_ref()
                            .map(|v| black_box(v.reading()))
                            .map_err(|e| *e),
                    ));
                }
                Batch::Expm1(values)
            }
            Phase::BatchExp => {
                let budget = context.budget.as_mut().ok_or(Error::Allocation)?;
                let values = prepared
                    .exp
                    .evaluate_batch(&item.exp, prepared.arithmetic, budget)
                    .map_err(|e| Error::Budget(e.kind))?;
                for value in &values {
                    context.snapshots.push(snapshot_exp(
                        value
                            .as_ref()
                            .map(|v| black_box(v.reading()))
                            .map_err(|e| *e),
                    ));
                }
                Batch::Exp(values)
            }
            Phase::PlatformExpm1 | Phase::PlatformExp => {
                for &input in &item.values {
                    let value = if self.phase == Phase::PlatformExpm1 {
                        black_box(input).exp_m1()
                    } else {
                        black_box(input).exp()
                    };
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
        if let Ok(output) = output {
            if let Some(inspect) = self.inspect {
                inspect(&output.snapshots, &output.baseline);
            }
            // Explicitly release the ordinary owners after evidence is preserved.
            match output.batch {
                Batch::None => {}
                Batch::Expm1(values) => drop(values),
                Batch::Exp(values) => drop(values),
            }
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
            scope: "whole block, consumed complete readings; batch includes allocation and reading",
            trace_configuration: "disabled",
        },
    }
}

/// Explicit, printed host assumptions, including allocation and complete cleanup.
pub(super) const fn allowances() -> AdapterBounds {
    AdapterBounds {
        uninterrupted: Duration::from_secs(1),
        receipt: Duration::from_secs(1),
        disposal: Duration::from_secs(1),
        cleanup: Duration::from_secs(1),
    }
}

/// Named finite settings; no machine-derived defaults or calibration policy.
pub(super) fn settings() -> Result<SamplingSettings, AnalysisError> {
    Ok(SamplingSettings {
        analysis: AnalysisSettings::new(
            10,
            1000,
            0.95,
            20_261_110,
            AnalysisCaps {
                report_bytes: 65_536,
                scratch_bytes: 262_144,
            },
        )?,
        warm_up: Duration::from_millis(10),
        measurement: Duration::from_millis(50),
        wall_budget: Duration::from_secs(30),
        max_total_iterations: 16_384,
        max_iterations_per_sample: 4096,
        caps: AnalysisCaps {
            report_bytes: 16_777_216,
            scratch_bytes: 262_144,
        },
        bounds: SamplerBounds {
            uninterrupted: Duration::from_secs(1),
            finalization: Duration::from_secs(1),
        },
    })
}
