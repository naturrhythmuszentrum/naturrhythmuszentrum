// SPDX-License-Identifier: AGPL-3.0-only
//! Checked preparation, traversal, storage and amortization ledgers for naive LC recipes.

use super::{Numerators, RecipeError};
use crate::{CaseRequest, Operation, Scope};
use serde::{Deserialize, Serialize};

/// Named primitive counts; FMA contributes two conventional floating operations.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationCounts {
    /// Square roots.
    pub roots: u64,
    /// Divisions, including reciprocal preparation.
    pub divisions: u64,
    /// Separately rounded multiplications.
    pub multiplications: u64,
    /// Separately rounded additions.
    pub additions: u64,
    /// Fused multiply-add evaluations.
    pub fmas: u64,
}

impl OperationCounts {
    /// Exact conventional operation count, without counting loop/control work.
    fn floating(self) -> Result<u64, RecipeError> {
        [
            self.roots,
            self.divisions,
            self.multiplications,
            self.additions,
            product(self.fmas, 2)?,
        ]
        .into_iter()
        .try_fold(0, sum)
    }
}

/// One full traversal and its separately named preparation; no measured quantity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LcLedger {
    /// Exact ordered populations of the caller's numerical arrays.
    pub input_populations: Vec<u64>,
    /// Explicit inverse-query branch population, zero for other expressions.
    pub cycle_population: u64,
    /// Primitive work in one shared preparation.
    pub preparation: OperationCounts,
    /// Primitive work in one complete output traversal.
    pub traversal: OperationCounts,
    /// Logical owned input payload, including integer cycles.
    pub input_bytes: u64,
    /// Retained shared-frequency/reciprocal payload.
    pub prepared_bytes: u64,
    /// Simultaneous per-axis root scratch, absent for non-forward expressions.
    pub root_scratch_bytes: u64,
    /// Full consumed output population in bytes.
    pub output_bytes: u64,
    /// Uses of each prepared root on the L and C axes, respectively.
    pub root_reuse: [u64; 2],
    /// Output uses of each prepared reciprocal; zero when no reciprocal is prepared.
    pub reciprocal_reuse: u64,
    /// Logical reads/writes in preparation, including output initialization.
    pub preparation_bytes: u64,
    /// Logical reads/writes in one traversal, never physical bus traffic.
    pub traversal_bytes: u64,
}

/// All ledger counts obey the wire integer domain before allocation or sampling.
fn sum(a: u64, b: u64) -> Result<u64, RecipeError> {
    a.checked_add(b)
        .filter(|n| *n <= crate::decimal::MAX_INTEGER)
        .ok_or(RecipeError::Overflow)
}

/// Checked dimensions and populations never wrap or round.
fn product(a: u64, b: u64) -> Result<u64, RecipeError> {
    a.checked_mul(b)
        .filter(|n| *n <= crate::decimal::MAX_INTEGER)
        .ok_or(RecipeError::Overflow)
}

impl LcLedger {
    /// Reconstruct the ledger from exact recipe populations, without preparing numerical inputs.
    /// # Errors
    /// Refuses incompatible shapes, empty populations and count overflow.
    pub fn from_case(case: &CaseRequest) -> Result<Self, RecipeError> {
        let bytes = case
            .inputs
            .len()
            .checked_mul(size_of::<u64>())
            .ok_or(RecipeError::Overflow)?;
        Self::from_case_owned(
            case,
            &mut crate::memory::Budget::new(
                u64::try_from(bytes).map_err(|_| RecipeError::Overflow)?,
            ),
        )
    }

    /// Reserve the population vector against the collector's existing ownership allowance.
    pub(crate) fn from_case_owned(
        case: &CaseRequest,
        budget: &mut crate::memory::Budget,
    ) -> Result<Self, RecipeError> {
        let mut populations = budget.reserve(case.inputs.len())?;
        for values in &case.inputs {
            populations.push(u64::try_from(values.len()).map_err(|_| RecipeError::Overflow)?);
        }
        let cycles = u64::try_from(case.cycles.len()).map_err(|_| RecipeError::Overflow)?;
        Self::from_populations(case.operation, populations, cycles)
    }

    /// Population-only reconstruction also supports summary validation without raw input arrays.
    /// # Errors
    /// Refuses incompatible operation/population combinations and count overflow.
    pub fn from_populations(
        operation: Operation,
        populations: Vec<u64>,
        cycles: u64,
    ) -> Result<Self, RecipeError> {
        if populations.contains(&0) {
            return Err(RecipeError::Domain);
        }
        let input_bytes = product(populations.iter().copied().try_fold(cycles, sum)?, 8)?;
        let mut ledger = Self {
            input_populations: populations,
            cycle_population: cycles,
            preparation: OperationCounts::default(),
            traversal: OperationCounts::default(),
            input_bytes,
            prepared_bytes: 0,
            root_scratch_bytes: 0,
            output_bytes: 0,
            root_reuse: [0; 2],
            reciprocal_reuse: 0,
            preparation_bytes: 0,
            traversal_bytes: 0,
        };
        match operation {
            Operation::ForwardFused | Operation::ForwardSeparate => ledger.forward(operation)?,
            Operation::Inductance | Operation::Capacitance => ledger.element()?,
            Operation::InverseTime => ledger.inverse()?,
            _ => return Err(RecipeError::Domain),
        }
        ledger.preparation_bytes = sum(ledger.preparation_bytes, ledger.output_bytes)?;
        Ok(ledger)
    }

    /// Independent axes share roots and pair reciprocals across explicit time/phase populations.
    fn forward(&mut self, operation: Operation) -> Result<(), RecipeError> {
        let [l, c, times, phases] = self.input_populations.as_slice() else {
            return Err(RecipeError::Domain);
        };
        if self.cycle_population != 0 {
            return Err(RecipeError::Domain);
        }
        let pairs = product(*l, *c)?;
        let reuse = product(*times, *phases)?;
        let outputs = product(pairs, reuse)?;
        self.preparation.roots = sum(*l, *c)?;
        self.preparation.divisions = pairs;
        self.preparation.multiplications = pairs;
        if operation == Operation::ForwardFused {
            self.traversal.fmas = outputs;
        } else {
            self.traversal.multiplications = outputs;
            self.traversal.additions = outputs;
        }
        self.prepared_bytes = product(pairs, 8)?;
        self.root_scratch_bytes = product(self.preparation.roots, 8)?;
        self.output_bytes = product(outputs, 8)?;
        self.root_reuse = [*c, *l];
        self.reciprocal_reuse = reuse;
        self.preparation_bytes = product(sum(self.preparation.roots, pairs)?, 24)?;
        self.traversal_bytes = product(outputs, 32)?;
        Ok(())
    }

    /// A denominator is shared only when multiple products use its reciprocal.
    fn element(&mut self) -> Result<(), RecipeError> {
        let [products, denominators] = self.input_populations.as_slice() else {
            return Err(RecipeError::Domain);
        };
        if self.cycle_population != 0 {
            return Err(RecipeError::Domain);
        }
        let outputs = product(*products, *denominators)?;
        if *products > 1 {
            self.preparation.divisions = *denominators;
            self.traversal.multiplications = outputs;
            self.prepared_bytes = product(*denominators, 8)?;
            self.reciprocal_reuse = *products;
            self.preparation_bytes = product(*denominators, 16)?;
        } else {
            self.traversal.divisions = outputs;
        }
        self.output_bytes = product(outputs, 8)?;
        self.traversal_bytes = product(outputs, 24)?;
        Ok(())
    }

    /// One frequency reciprocal is shared across the entire finite branch population.
    fn inverse(&mut self) -> Result<(), RecipeError> {
        let [phases, 1, 1] = self.input_populations.as_slice() else {
            return Err(RecipeError::Domain);
        };
        if self.cycle_population == 0 {
            return Err(RecipeError::Domain);
        }
        let outputs = product(*phases, self.cycle_population)?;
        self.preparation.divisions = 1;
        self.traversal.multiplications = product(outputs, 3)?;
        self.traversal.additions = outputs;
        self.prepared_bytes = 8;
        self.output_bytes = product(outputs, 8)?;
        self.reciprocal_reuse = outputs;
        self.preparation_bytes = 16;
        self.traversal_bytes = product(outputs, 40)?;
        Ok(())
    }

    /// Work in the selected timed scope, with one complete output consumption per repetition.
    /// # Errors
    /// Refuses an unsupported scope, empty item or integer overflow.
    pub fn numerators(&self, scope: Scope, length: u64) -> Result<Numerators, RecipeError> {
        if length == 0 || !matches!(scope, Scope::Prepared | Scope::PreparationInclusive) {
            return Err(RecipeError::Domain);
        }
        let preparation = scope == Scope::PreparationInclusive;
        let floating = sum(
            self.traversal.floating()?,
            if preparation {
                self.preparation.floating()?
            } else {
                0
            },
        )?;
        let bytes = sum(
            self.traversal_bytes,
            if preparation {
                self.preparation_bytes
            } else {
                0
            },
        )?;
        Ok(Numerators {
            items: 1,
            elements: product(self.output_bytes / 8, length)?,
            vector_operations: None,
            floating_operations: Some(product(floating, length)?),
            logical_bytes: Some(product(bytes, length)?),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonsquare_forward_records_every_preparation_and_output() -> Result<(), RecipeError> {
        let ledger = LcLedger::from_populations(Operation::ForwardFused, vec![2, 3, 2, 1], 0)?;
        assert_eq!(
            ledger.preparation,
            OperationCounts {
                roots: 5,
                divisions: 6,
                multiplications: 6,
                additions: 0,
                fmas: 0,
            }
        );
        assert_eq!(
            ledger.traversal,
            OperationCounts {
                fmas: 12,
                ..OperationCounts::default()
            }
        );
        assert_eq!(
            (
                ledger.input_bytes,
                ledger.prepared_bytes,
                ledger.root_scratch_bytes,
                ledger.output_bytes
            ),
            (64, 48, 40, 96)
        );
        assert_eq!(ledger.root_reuse, [3, 2]);
        assert_eq!(ledger.reciprocal_reuse, 2);
        assert_eq!(
            ledger.numerators(Scope::PreparationInclusive, 2)?,
            Numerators {
                items: 1,
                elements: 24,
                vector_operations: None,
                floating_operations: Some(82),
                logical_bytes: Some(1488),
            }
        );
        let separate = LcLedger::from_populations(Operation::ForwardSeparate, vec![2, 3, 2, 1], 0)?;
        assert_eq!(separate.traversal.multiplications, 12);
        assert_eq!(separate.traversal.additions, 12);
        assert_eq!(separate.traversal.fmas, 0);
        Ok(())
    }

    #[test]
    fn shared_denominators_and_finite_branches_have_distinct_amortization()
    -> Result<(), RecipeError> {
        let shared = LcLedger::from_populations(Operation::Inductance, vec![2, 2], 0)?;
        assert_eq!(shared.preparation.divisions, 2);
        assert_eq!(shared.traversal.multiplications, 4);
        assert_eq!(shared.reciprocal_reuse, 2);
        let single = LcLedger::from_populations(Operation::Capacitance, vec![1, 2], 0)?;
        assert_eq!(single.preparation, OperationCounts::default());
        assert_eq!(single.traversal.divisions, 2);
        assert_eq!(single.reciprocal_reuse, 0);
        let inverse = LcLedger::from_populations(Operation::InverseTime, vec![2, 1, 1], 3)?;
        assert_eq!(inverse.preparation.divisions, 1);
        assert_eq!(inverse.traversal.multiplications, 18);
        assert_eq!(inverse.traversal.additions, 6);
        assert_eq!(inverse.input_bytes, 56);
        assert_eq!(inverse.reciprocal_reuse, 6);
        assert_eq!(
            inverse
                .numerators(Scope::PreparationInclusive, 1)?
                .floating_operations,
            Some(25)
        );
        Ok(())
    }

    #[test]
    fn malformed_or_overflowing_populations_have_no_ledger() {
        assert_eq!(
            LcLedger::from_populations(Operation::InverseTime, vec![2, 2, 1], 3),
            Err(RecipeError::Domain)
        );
        assert_eq!(
            LcLedger::from_populations(Operation::ForwardFused, vec![2, 3, 2, 1], 1),
            Err(RecipeError::Domain)
        );
        assert_eq!(
            LcLedger::from_populations(
                Operation::ForwardFused,
                vec![crate::decimal::MAX_INTEGER, 2, 1, 1],
                0
            ),
            Err(RecipeError::Overflow)
        );
    }
}
