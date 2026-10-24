// SPDX-License-Identifier: AGPL-3.0-only
//! Owner measurements separate inventory, storage and each coordinate operation under production defaults.

use super::reading_testing::{TestResult, ledger};
use super::state_fixture::{self, Plan};
use nrz_accuracy::NormKind;
use nrz_bindings::Budget;
use nrz_matrix::{
    CoordinateReading, DependencyPolicy, Heap, HeapLimits, ReadingBinding, ReadingCoverage,
};
use nrz_scalar::ArithmeticContext;

/// Prerequisites retained once while one public owner operation is measured.
pub(crate) struct Case {
    /// Exact state inventory and original coordinate contract.
    pub(crate) plan: Plan,
    /// Numerical owner of the imported state and its companions.
    heap: Heap,
    /// Original whole-state reading selected before normalization.
    input: ReadingBinding<f64>,
    /// Normalized result retained for restoration measurements.
    normalized: CoordinateReading<f64>,
    /// Physical result retained for named norm measurements.
    restored: CoordinateReading<f64>,
    /// One compatible arithmetic revision for every evaluation.
    context: ArithmeticContext,
}

impl Case {
    /// Prepare complete exact and numerical prerequisites outside the operation clock.
    pub(crate) fn new() -> TestResult<Self> {
        let plan = state_fixture::plan(&super::references()?)?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let imported = state_fixture::import(&plan, &mut heap, DependencyPolicy::Entries)?;
        let input = imported.bindings()[0].clone();
        let context = ArithmeticContext::new()?;
        let normalized =
            heap.normalize_reading(&input, &plan.contract, &context, &mut ledger()?)?;
        let restored = heap.restore_reading(&normalized, &context, &mut ledger()?)?;
        Ok(Self {
            plan,
            heap,
            input,
            normalized,
            restored,
            context,
        })
    }

    /// Execute one named inventory, storage or coordinate operation on the supplied ledger.
    pub(crate) fn run(&self, operation: usize, budget: &mut Budget) -> TestResult {
        match operation {
            0 => {
                ReadingCoverage::new(
                    self.plan.coverage.entries().to_vec(),
                    self.plan.coverage.readings().to_vec(),
                    self.plan.coverage.overlaps().to_vec(),
                    budget,
                )?;
            }
            1 | 2 => {
                let mut heap = Heap::new(HeapLimits::default())?;
                let policy = if operation == 1 {
                    DependencyPolicy::Entries
                } else {
                    DependencyPolicy::WholeBase
                };
                state_fixture::import_with_budget(&self.plan, &mut heap, policy, budget)?;
            }
            3 => {
                self.heap.normalize_reading(
                    &self.input,
                    &self.plan.contract,
                    &self.context,
                    budget,
                )?;
            }
            4 => {
                self.heap
                    .restore_reading(&self.normalized, &self.context, budget)?;
            }
            5 => {
                self.heap.coordinate_norm(
                    &self.restored,
                    &self.plan.contract,
                    NormKind::Euclidean,
                    &self.context,
                    budget,
                )?;
            }
            _ => return Err("unknown owner operation".into()),
        }
        Ok(())
    }
}
