// SPDX-License-Identifier: AGPL-3.0-only
//! Adapter for native operations with no exact ledger, allocation or retained mutation.

use nrz_matrix::sampling::{
    AdapterBounds, HostOperation, ReceiptStatus, SamplingDeadline, WorkReceipt,
};
use std::{fmt::Debug, time::Duration};

/// Native evaluation has zero exact Work by its operation contract.
#[derive(Debug)]
pub(super) struct Native<F> {
    /// Prepared operation and immutable input captured by borrow.
    pub(super) operation: F,
}

impl<F, O, E> HostOperation<(), ()> for Native<F>
where
    F: FnMut() -> Result<O, E>,
    E: Copy + Debug,
{
    type Context = ();
    type Output = O;
    type Error = E;
    type Work = u64;

    fn bounds(&self) -> Option<AdapterBounds> {
        // These are host execution assumptions, not measured scheduling guarantees.
        Some(AdapterBounds {
            uninterrupted: Duration::from_secs(1),
            receipt: Duration::from_secs(1),
            disposal: Duration::from_secs(1),
            cleanup: Duration::from_secs(1),
        })
    }

    fn reset(&mut self, (): &(), (): &(), _: &SamplingDeadline<'_>) -> Result<(), E> {
        Ok(())
    }

    fn snapshot(&mut self, (): &(), _: &SamplingDeadline<'_>) -> Option<u64> {
        Some(0)
    }

    fn run(&mut self, (): &(), (): &(), (): &mut (), _: &SamplingDeadline<'_>) -> Result<O, E> {
        (self.operation)()
    }

    fn receipt(&mut self, (): &(), _: &Result<O, E>, _: &SamplingDeadline<'_>) -> WorkReceipt<u64> {
        WorkReceipt {
            exit: Some(0),
            operation: Some(0),
            status: ReceiptStatus::Complete,
        }
    }

    fn dispose(&mut self, output: Result<O, E>, _: &SamplingDeadline<'_>) -> Result<(), E> {
        drop(output);
        Ok(())
    }

    fn cleanup(&mut self, (): (), _: &SamplingDeadline<'_>) -> Result<(), E> {
        Ok(())
    }
}
