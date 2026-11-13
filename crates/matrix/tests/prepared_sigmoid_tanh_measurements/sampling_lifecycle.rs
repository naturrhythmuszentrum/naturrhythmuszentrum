// SPDX-License-Identifier: AGPL-3.0-only
//! Assert actual callback postconditions before the sampler advances its clock.
use super::*;
use nrz_matrix::sampling::{AdapterBounds, HostOperation, SamplingDeadline, WorkReceipt};
use sampling::{Context, Error, Output};
/// The real adapter is retained across distinguishable blocks.
#[derive(Debug)]
pub(super) struct Checked<'s> {
    /// Private implementation under test.
    pub(super) inner: &'s mut Adapter,
    /// Full ordinary results for this block.
    pub(super) expected: &'s [Snapshot],
    /// Full ordinary platform bits for this block.
    pub(super) baseline: &'s [u64],
}
impl<'a, 'g: 'a> HostOperation<Prepared<'a, 'g>, Block> for Checked<'_> {
    type Context = Context;
    type Output = Output<'a>;
    type Error = Error;
    type Work = Work;
    fn bounds(&self) -> Option<AdapterBounds> {
        self.inner.bounds()
    }
    fn reset(
        &mut self,
        prepared: &Prepared<'a, 'g>,
        item: &Block,
        deadline: &SamplingDeadline<'_>,
    ) -> Result<Context, Error> {
        let _span = tracing::trace_span!("nrz.measure", family = "sampler-reset").entered();
        sampling::CALLBACK.with(|current| current.set("reset"));
        let events_before = sampling::EVALUATIONS.with(|events| events.borrow().len());
        let context = self.inner.reset(prepared, item, deadline)?;
        let [(n, c), (bn, bc)] = context.scratch_layout();
        assert_eq!([n, bn], [0, 0], "reset must not evaluate or consume");
        let capacity = if self.baseline.is_empty() { c } else { bc };
        assert!(
            capacity >= item.values.len(),
            "reset reserves the whole block before timing"
        );
        no_evaluation(events_before);
        Ok(context)
    }
    fn snapshot(&mut self, context: &Context, deadline: &SamplingDeadline<'_>) -> Option<Work> {
        let _span = tracing::trace_span!("nrz.measure", family = "sampler-entry").entered();
        sampling::CALLBACK.with(|current| current.set("entry"));
        let events_before = sampling::EVALUATIONS.with(|events| events.borrow().len());
        let result = self.inner.snapshot(context, deadline);
        no_evaluation(events_before);
        result
    }
    fn run(
        &mut self,
        prepared: &Prepared<'a, 'g>,
        item: &Block,
        context: &mut Context,
        deadline: &SamplingDeadline<'_>,
    ) -> Result<Output<'a>, Error> {
        let _span = tracing::trace_span!("nrz.measure", family = "sampler-run").entered();
        sampling::CALLBACK.with(|current| current.set("run"));
        let events_before = sampling::EVALUATIONS.with(|events| events.borrow().len());
        let before = sampling::DROPPED.with(Cell::get);
        let output = self.inner.run(prepared, item, context, deadline)?;
        assert_eq!(
            output.readings(),
            (self.expected, self.baseline),
            "run consumes complete ordered results before timer stop"
        );
        let owners = if matches!(self.inner.phase, Phase::BatchSigmoid | Phase::BatchTanh) {
            item.values.len()
        } else {
            0
        };
        assert_eq!(
            output.owners(),
            owners,
            "batch owners survive until disposal"
        );
        assert_eq!(
            sampling::DROPPED.with(Cell::get),
            before,
            "run does not dispose"
        );
        let expected: Vec<_> = item.values.iter().map(|v| ("run", v.to_bits())).collect();
        sampling::EVALUATIONS.with(|events| {
            assert_eq!(
                &events.borrow()[events_before..],
                expected,
                "evaluation follows block order exactly once in timed run"
            );
        });
        Ok(output)
    }
    fn receipt(
        &mut self,
        context: &Context,
        output: &Result<Output<'a>, Error>,
        deadline: &SamplingDeadline<'_>,
    ) -> WorkReceipt<Work> {
        let _span = tracing::trace_span!("nrz.measure", family = "sampler-receipt").entered();
        sampling::CALLBACK.with(|current| current.set("receipt"));
        let events_before = sampling::EVALUATIONS.with(|events| events.borrow().len());
        let receipt = self.inner.receipt(context, output, deadline);
        assert_eq!(
            output.as_ref().expect("successful run").readings(),
            (self.expected, self.baseline),
            "receipt preserves the already consumed result"
        );
        no_evaluation(events_before);
        receipt
    }
    fn dispose(
        &mut self,
        output: Result<Output<'a>, Error>,
        deadline: &SamplingDeadline<'_>,
    ) -> Result<(), Error> {
        let _span = tracing::trace_span!("nrz.measure", family = "sampler-dispose").entered();
        sampling::CALLBACK.with(|current| current.set("dispose"));
        let events_before = sampling::EVALUATIONS.with(|events| events.borrow().len());
        let before = sampling::DROPPED.with(Cell::get);
        let owners = output.as_ref().expect("successful output").owners();
        let owners_before = sampling::OWNERS_DROPPED.with(Cell::get);
        let result = self.inner.dispose(output, deadline);
        assert_eq!(
            sampling::OWNERS_DROPPED.with(Cell::get),
            owners_before + owners,
            "dispose releases retained batch owners"
        );
        no_evaluation(events_before);
        assert_eq!(
            sampling::DROPPED.with(Cell::get),
            before + 1,
            "dispose destroys the real output"
        );
        result
    }
    fn cleanup(&mut self, context: Context, deadline: &SamplingDeadline<'_>) -> Result<(), Error> {
        let _span = tracing::trace_span!("nrz.measure", family = "sampler-cleanup").entered();
        sampling::CALLBACK.with(|current| current.set("cleanup"));
        let events_before = sampling::EVALUATIONS.with(|events| events.borrow().len());
        let result = self.inner.cleanup(context, deadline);
        no_evaluation(events_before);
        result
    }
}

/// Every untimed callback must leave the arithmetic event inventory unchanged.
fn no_evaluation(before: usize) {
    sampling::EVALUATIONS.with(|events| {
        assert_eq!(
            events.borrow().len(),
            before,
            "arithmetic belongs only to the timed run"
        );
    });
}
