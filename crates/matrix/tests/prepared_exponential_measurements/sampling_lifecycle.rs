// SPDX-License-Identifier: AGPL-3.0-only
//! Direct callback evidence, confined to the integration test build.
use super::sampling::{Context, Error, Output};
use super::{Adapter, Block, Phase, Prepared, Work};
use nrz_matrix::sampling::{AdapterBounds, HostOperation, SamplingDeadline, WorkReceipt};

/// The sampler drives the real adapter; only the test inspects its reset result.
#[derive(Debug)]
pub(super) struct Checked<'s> {
    /// Retained across distinct representatives, so stale adapter caches are observable.
    pub(super) inner: &'s mut Adapter,
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
        let context = self.inner.reset(prepared, item, deadline)?;
        let [(length, capacity), (platform_length, platform_capacity)] = context.scratch_layout();
        assert_eq!(
            [length, platform_length],
            [0, 0],
            "reset must return empty result storage before the operation timer"
        );
        let selected_capacity = match self.inner.phase {
            Phase::PlatformExp | Phase::PlatformExpm1 => platform_capacity,
            _ => capacity,
        };
        assert!(
            selected_capacity >= item.values.len(),
            "reset must reserve the whole result vector before the operation timer"
        );
        Ok(context)
    }

    fn snapshot(&mut self, context: &Context, deadline: &SamplingDeadline<'_>) -> Option<Work> {
        self.inner.snapshot(context, deadline)
    }

    fn run(
        &mut self,
        prepared: &Prepared<'a, 'g>,
        item: &Block,
        context: &mut Context,
        deadline: &SamplingDeadline<'_>,
    ) -> Result<Output<'a>, Error> {
        self.inner.run(prepared, item, context, deadline)
    }

    fn receipt(
        &mut self,
        context: &Context,
        output: &Result<Output<'a>, Error>,
        deadline: &SamplingDeadline<'_>,
    ) -> WorkReceipt<Work> {
        self.inner.receipt(context, output, deadline)
    }

    fn dispose(
        &mut self,
        output: Result<Output<'a>, Error>,
        deadline: &SamplingDeadline<'_>,
    ) -> Result<(), Error> {
        self.inner.dispose(output, deadline)
    }

    fn cleanup(&mut self, context: Context, deadline: &SamplingDeadline<'_>) -> Result<(), Error> {
        self.inner.cleanup(context, deadline)
    }
}
