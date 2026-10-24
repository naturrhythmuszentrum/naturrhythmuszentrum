// SPDX-License-Identifier: AGPL-3.0-only
//! Pure numerical budget probes reuse immutable inputs instead of rebuilding exact proofs per limit.

use super::*;

/// Constructor and immutable numerical cases have separate retained graph authorities.
pub(super) enum Case {
    /// All exact correspondence nodes already exist in the same constructor fixture as the cold probe.
    Contract(Box<fixture::Plan>),
    /// Live immutable sources and both complete numerical images.
    Numerical(Box<Numerical>),
}

/// Preparing inputs is independent of the one measured public operation.
pub(super) struct Numerical {
    /// Original storage with stable selected-entry versions.
    heap: Heap,
    /// Complete original value and companion selection.
    binding: ReadingBinding<f64>,
    /// Fixed exact references and both certified coordinate images.
    contract: CoordinateContract,
    /// Complete normalized source for restoration tests.
    normalized: CoordinateReading<f64>,
    /// Complete restored source for named-norm tests.
    restored: CoordinateReading<f64>,
    /// Fixed arithmetic revision, shared by every attempt.
    context: ArithmeticContext,
}

impl Case {
    /// Prepare one finite scenario under unchanged production defaults.
    pub(super) fn new(operation: u32) -> TestResult<Self> {
        if operation == 0 {
            return Ok(Self::Contract(Box::new(fixture::plan()?)));
        }
        let (heap, binding, contract, _) = fixture::imported::<f64>(true)?;
        let context = ArithmeticContext::new()?;
        let normalized = heap.normalize_reading(&binding, &contract, &context, &mut ledger()?)?;
        let restored = heap.restore_reading(&normalized, &context, &mut ledger()?)?;
        Ok(Self::Numerical(Box::new(Numerical {
            heap,
            binding,
            contract,
            normalized,
            restored,
            context,
        })))
    }

    /// Work never resets inside an operation, including a caller's earlier ledger charge.
    pub(super) fn attempt(
        &mut self,
        operation: u32,
        limits: Limits,
        precharge: u64,
    ) -> TestResult<u64> {
        let mut budget = Budget::new(limits)?;
        budget.work(
            precharge,
            nrz_bindings::readings::algebra::Location::new("coordinate precharge"),
        )?;
        match self {
            Self::Contract(prepared) => {
                CoordinateContract::new(
                    &mut prepared.case.cas,
                    prepared.contract.frame(),
                    LiteralLimits::default(),
                    &mut budget,
                )?;
            }
            Self::Numerical(case) => case.evaluate(operation, &mut budget)?,
        }
        Ok(budget.usage().work)
    }
}

impl Numerical {
    /// Numerical entry points borrow the exact same live definitions at every attempted limit.
    fn evaluate(&self, operation: u32, budget: &mut Budget) -> TestResult {
        match operation {
            1 => {
                self.heap.normalize_reading(
                    &self.binding,
                    &self.contract,
                    &self.context,
                    budget,
                )?;
            }
            2 => {
                self.heap
                    .restore_reading(&self.normalized, &self.context, budget)?;
            }
            _ => {
                let kind = match operation {
                    3 => NormKind::One,
                    4 => NormKind::Euclidean,
                    _ => NormKind::Infinity,
                };
                self.heap.coordinate_norm(
                    &self.restored,
                    &self.contract,
                    kind,
                    &self.context,
                    budget,
                )?;
            }
        }
        Ok(())
    }
}

/// Measured Work, staged bytes and output bytes for the immutable default-profile live fixture.
pub(super) const fn profiles() -> [[u64; 3]; 6] {
    [
        [219_970, 153_713, 60_522],
        [40_736, 60_778, 60_986],
        [37_663, 61_242, 61_450],
        [37_711, 122_212, 122_388],
        [37_711, 122_212, 122_388],
        [37_711, 122_212, 122_388],
    ]
}
