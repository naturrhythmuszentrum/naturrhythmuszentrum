// SPDX-License-Identifier: AGPL-3.0-only
//! Public prepared native/charged sampling with independent ordinary Work accounting.

#[path = "../examples/prepared_exponential/support.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared fixture helpers are private to this integration target"
)]
mod support;

use nrz_bindings::readings::algebra::{FaultKind, limits::Budget};
use nrz_matrix::{Expm1Evaluation, PreparedExpm1, sampling::*};
use nrz_scalar::{ArithmeticContext, Expm1Failure, Expm1Input, Expm1Output};
use std::{cell::Cell, time::Duration};
use support::{Fixture, TestResult, broad, ledger};

#[path = "host_sampling/controls.rs"]
mod controls;
#[path = "host_sampling/live.rs"]
mod live;

/// Controlled clock makes all expectations independent of CPU speed or tracing cost.
#[derive(Debug, Default)]
struct Clock(Cell<Duration>);

impl SamplingClock for Clock {
    fn now(&self) -> Duration {
        self.0.get()
    }
}

/// Host wrapper retains ordinary typed native or budget errors in a fixed record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Error {
    Native(Expm1Failure),
    Budget(FaultKind),
}

/// Each reset creates a new ledger; no prior counter or identity is reset.
#[derive(Debug)]
struct Context<'a> {
    /// Operation-owned cumulative accounting.
    budget: Budget,
    /// Independent live-resource inventory.
    live: &'a Cell<usize>,
}

impl Drop for Context<'_> {
    fn drop(&mut self) {
        self.live.set(self.live.get() - 1);
    }
}

/// Preserve the ordinary operation output until after receipt extraction.
#[derive(Debug)]
enum Value<'a> {
    /// One native evaluation, carrying exact authority by borrow.
    Native(Expm1Evaluation<'a>),
    /// Ordinary charged batch output, including per-cell native errors.
    Charged(Vec<Result<Expm1Evaluation<'a>, Expm1Failure>>),
}

/// Output carries the ordinary ledger reading established inside the timed call.
#[derive(Debug)]
struct Output<'a> {
    /// Ordinary operation value, neither reformatted nor re-executed for evidence.
    value: Value<'a>,
    /// Exact Work observed inside the chosen host operation.
    receipt: u64,
    /// Independent live output inventory.
    live: &'a Cell<usize>,
}

impl Drop for Output<'_> {
    fn drop(&mut self) {
        self.live.set(self.live.get() - 1);
    }
}

/// One host adapter chooses either native evaluation or the ordinary charged batch.
#[derive(Debug)]
struct Adapter<'a> {
    /// Native arithmetic environment, prepared outside sampling.
    arithmetic: &'a ArithmeticContext,
    /// Local deterministic clock.
    clock: &'a Clock,
    /// Resource inventory shared with each private lifetime.
    live: &'a Cell<usize>,
    /// Explicit scope selection; never selected from timings.
    charged: bool,
    /// Numerical output bits retained only for independent result comparison.
    outputs: Vec<[u64; 5]>,
}

impl<'a, 'graph> HostOperation<&'a PreparedExpm1<'graph>, Expm1Input> for Adapter<'a> {
    type Context = Context<'a>;
    type Output = Output<'a>;
    type Error = Error;
    type Work = u64;
    fn bounds(&self) -> Option<AdapterBounds> {
        Some(AdapterBounds {
            uninterrupted: Duration::from_millis(1),
            receipt: Duration::ZERO,
            disposal: Duration::ZERO,
            cleanup: Duration::ZERO,
        })
    }
    fn reset(
        &mut self,
        _: &&'a PreparedExpm1<'graph>,
        _: &Expm1Input,
        _: &SamplingDeadline<'_>,
    ) -> Result<Context<'a>, Error> {
        let budget = ledger().map_err(|error| Error::Budget(error.kind))?;
        self.live.set(self.live.get() + 1);
        Ok(Context {
            budget,
            live: self.live,
        })
    }
    fn snapshot(&mut self, context: &Context<'a>, _: &SamplingDeadline<'_>) -> Option<u64> {
        Some(context.budget.usage().work)
    }
    fn run(
        &mut self,
        prepared: &&'a PreparedExpm1<'graph>,
        input: &Expm1Input,
        context: &mut Context<'a>,
        _: &SamplingDeadline<'_>,
    ) -> Result<Output<'a>, Error> {
        self.clock
            .0
            .set(self.clock.0.get() + Duration::from_millis(1));
        let value = if self.charged {
            Value::Charged(
                prepared
                    .evaluate_batch(
                        std::slice::from_ref(input),
                        self.arithmetic,
                        &mut context.budget,
                    )
                    .map_err(|error| Error::Budget(error.kind))?,
            )
        } else {
            Value::Native(
                prepared
                    .evaluate(*input, self.arithmetic)
                    .map_err(Error::Native)?,
            )
        };
        self.live.set(self.live.get() + 1);
        Ok(Output {
            value,
            receipt: context.budget.usage().work,
            live: self.live,
        })
    }
    fn receipt(
        &mut self,
        context: &Context<'a>,
        output: &Result<Output<'a>, Error>,
        _: &SamplingDeadline<'_>,
    ) -> WorkReceipt<u64> {
        let exit = context.budget.usage().work;
        WorkReceipt {
            exit: Some(exit),
            operation: Some(output.as_ref().map_or(exit, |output| output.receipt)),
            status: ReceiptStatus::Complete,
        }
    }
    fn dispose(
        &mut self,
        output: Result<Output<'a>, Error>,
        _: &SamplingDeadline<'_>,
    ) -> Result<(), Error> {
        if let Ok(output) = output {
            match &output.value {
                Value::Native(value) => self.outputs.push(bits(value.reading())),
                Value::Charged(values) => {
                    for value in values {
                        match value {
                            Ok(value) => self.outputs.push(bits(value.reading())),
                            Err(error) => return Err(Error::Native(*error)),
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn cleanup(&mut self, context: Context<'a>, _: &SamplingDeadline<'_>) -> Result<(), Error> {
        drop(context);
        Ok(())
    }
}

/// Bitwise result comparison includes both mathematical and numerical enclosures.
const fn bits(output: Expm1Output) -> [u64; 5] {
    [
        output.endpoints[0].to_bits(),
        output.endpoints[1].to_bits(),
        output.center.to_bits(),
        output.total_radius.to_bits(),
        output.numerical_error.to_bits(),
    ]
}

/// Explicit policy for one warmed native operation and ten linear rows.
fn settings() -> Result<SamplingSettings, AnalysisError> {
    let analysis = AnalysisSettings::new(
        10,
        1000,
        0.95,
        901,
        AnalysisCaps {
            report_bytes: 8192,
            scratch_bytes: 65_536,
        },
    )?;
    Ok(SamplingSettings {
        analysis,
        warm_up: Duration::from_millis(1),
        measurement: Duration::from_millis(55),
        wall_budget: Duration::from_secs(1),
        max_total_iterations: 100,
        max_iterations_per_sample: 10,
        caps: AnalysisCaps {
            report_bytes: 65_536,
            scratch_bytes: 65_536,
        },
        bounds: SamplerBounds {
            uninterrupted: Duration::ZERO,
            finalization: Duration::ZERO,
        },
    })
}

/// Identity explicitly distinguishes charged/native scope and scoped tracing.
const fn representative(
    input: &Expm1Input,
    charged: bool,
    observed: bool,
) -> Representative<'_, Expm1Input> {
    Representative {
        item: input,
        elements_per_item: Some(1),
        identity: SamplingIdentity {
            representative: "minus-one-quarter",
            shape: &[1],
            operation_revision: "prepared-expm1-native-v1",
            workload: "expm1-domain-minus-two-to-zero",
            scope: if charged {
                "one-item evaluate_batch, including inventory prepayment"
            } else {
                "evaluate"
            },
            trace_configuration: if observed {
                "scoped counting subscriber"
            } else {
                "disabled"
            },
        },
    }
}
