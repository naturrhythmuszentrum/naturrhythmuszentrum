// SPDX-License-Identifier: AGPL-3.0-only
//! Optimized naive LC traversal, with no certificate or completeness claim.

use crate::{CaseRequest, Operation, Scope, recipe::RecipeError};
use std::hint::black_box;

/// Explicit caller inputs and scope-specific prepared numerical state.
#[derive(Clone, Debug)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "the crate-visible kernel enum retains this prepared state"
)]
pub(crate) struct Lc {
    /// Named forward/element/inverse expression.
    operation: Operation,
    /// Prepared or preparation-inclusive timing scope.
    scope: Scope,
    /// Exact decoded caller arrays; no inferred domain defaults.
    inputs: Vec<Vec<f64>>,
    /// Explicit finite inverse-time cycle population.
    cycles: Vec<i64>,
    /// Per-axis-pair frequency or shared-denominator reciprocals.
    prepared: Vec<f64>,
    /// Explicit owned-payload allowance, also checked during preparation-inclusive allocation.
    byte_cap: u64,
    /// Complete consumed contiguous output population.
    output: Vec<f64>,
}

/// Fallibly reserve one explicit finite population.
fn zeros(length: usize, budget: &mut crate::memory::Budget) -> Result<Vec<f64>, RecipeError> {
    let mut out = budget.reserve(length)?;
    out.resize(length, 0.0);
    Ok(out)
}

/// Finite-normal intermediate admission for optimized naive expressions.
const fn ordinary(value: f64) -> Result<f64, RecipeError> {
    if value.is_normal() {
        Ok(value)
    } else {
        Err(RecipeError::Domain)
    }
}

impl Lc {
    /// Fixed storage evidence captured after allocation and before any item timer.
    pub(super) fn layout(&self) -> Result<[Option<crate::layout::BufferLayout>; 8], RecipeError> {
        use crate::layout::{BufferRole, buffer};
        let mut out = [None; 8];
        for (slot, input) in out.iter_mut().zip(&self.inputs) {
            *slot = buffer(input, BufferRole::Input, 0)?;
        }
        out[4] = buffer(&self.cycles, BufferRole::Cycles, 0)?;
        out[5] = buffer(&self.prepared, BufferRole::Prepared, 0)?;
        out[6] = buffer(&self.output, BufferRole::Output, 0)?;
        Ok(out)
    }

    /// Prepare explicit arrays and derived shared quantities outside kernel timing.
    pub(super) fn prepare(case: &CaseRequest, byte_cap: u64) -> Result<Self, RecipeError> {
        let output_count = population(case)?;
        let mut budget = crate::memory::Budget::new(byte_cap);
        budget.charge(size_of::<Self>())?;
        let mut inputs = budget.reserve(case.inputs.len())?;
        for values in &case.inputs {
            let mut decoded = zeros(values.len(), &mut budget)?;
            for (slot, bits) in decoded.iter_mut().zip(values) {
                *slot = f64::from_bits(bits.0);
            }
            inputs.push(decoded);
        }
        let mut out = Self {
            operation: case.operation,
            scope: case.scope,
            inputs,
            cycles: budget.copy(&case.cycles)?,
            prepared: Vec::new(),
            byte_cap,
            output: zeros(output_count, &mut budget)?,
        };
        out.prepare_shared()?;
        out.traverse()?;
        out.validate_intermediates()?;
        out.validate_output()?;
        out.reset();
        Ok(out)
    }

    /// Account actual vector capacities, headers, retained state and simultaneous root scratch.
    fn remaining_budget(&self) -> Result<crate::memory::Budget, RecipeError> {
        let mut elements = self
            .inputs
            .iter()
            .map(Vec::capacity)
            .chain([
                self.cycles.capacity(),
                self.prepared.capacity(),
                self.output.capacity(),
            ])
            .try_fold(0_usize, usize::checked_add)
            .ok_or(RecipeError::Overflow)?;
        elements = elements.checked_mul(8).ok_or(RecipeError::Overflow)?;
        let bytes = self
            .inputs
            .capacity()
            .checked_mul(size_of::<Vec<f64>>())
            .and_then(|n| n.checked_add(elements))
            .and_then(|n| n.checked_add(size_of::<Self>()))
            .ok_or(RecipeError::Overflow)?;
        let mut budget = crate::memory::Budget::new(self.byte_cap);
        budget.charge(bytes)?;
        Ok(budget)
    }

    /// Roots and frequency sharing are explicit preparation, never certified algebra.
    fn prepare_shared(&mut self) -> Result<(), RecipeError> {
        self.prepared = Vec::new();
        let mut budget = self.remaining_budget()?;
        match self.operation {
            Operation::ForwardFused | Operation::ForwardSeparate => {
                let [inductances, capacitances, _, _] = self.inputs.as_slice() else {
                    return Err(RecipeError::Domain);
                };
                let count = inductances
                    .len()
                    .checked_mul(capacitances.len())
                    .ok_or(RecipeError::Overflow)?;
                let mut roots_l = zeros(inductances.len(), &mut budget)?;
                let mut roots_c = zeros(capacitances.len(), &mut budget)?;
                for (root, value) in roots_l.iter_mut().zip(inductances) {
                    *root = ordinary(value.sqrt())?;
                }
                for (root, value) in roots_c.iter_mut().zip(capacitances) {
                    *root = ordinary(value.sqrt())?;
                }
                self.prepared = budget.reserve(count)?;
                for l in roots_l {
                    for c in &roots_c {
                        self.prepared.push(ordinary(1.0 / ordinary(l * c)?)?);
                    }
                }
            }
            Operation::Inductance | Operation::Capacitance => {
                let [products, denominators] = self.inputs.as_slice() else {
                    return Err(RecipeError::Domain);
                };
                if products.len() > 1 {
                    self.prepared = budget.reserve(denominators.len())?;
                    for value in denominators {
                        self.prepared.push(ordinary(1.0 / ordinary(*value)?)?);
                    }
                }
            }
            Operation::InverseTime => {
                let [_, omega, pi] = self.inputs.as_slice() else {
                    return Err(RecipeError::Domain);
                };
                if omega.len() != 1 || pi.len() != 1 {
                    return Err(RecipeError::Domain);
                }
                self.prepared = budget.reserve(1)?;
                self.prepared.push(ordinary(1.0 / ordinary(omega[0])?)?);
                ordinary(pi[0])?;
            }
            _ => return Err(RecipeError::Domain),
        }
        Ok(())
    }

    /// Forward traversal keeps the time axis innermost for each explicit initial phase.
    fn forward(&mut self) -> Result<(), RecipeError> {
        let [_, _, times, phases] = self.inputs.as_slice() else {
            return Err(RecipeError::Domain);
        };
        let mut output = self.output.iter_mut();
        for omega in black_box(self.prepared.as_slice()) {
            for phase in black_box(phases) {
                for time in black_box(times) {
                    let value = if self.operation == Operation::ForwardFused {
                        time.mul_add(*omega, *phase)
                    } else {
                        time * omega + phase
                    };
                    *output.next().ok_or(RecipeError::Domain)? = value;
                }
            }
        }
        Ok(())
    }

    /// Element and finite inverse-branch traversals preserve the selected populations.
    fn traverse(&mut self) -> Result<(), RecipeError> {
        match self.operation {
            Operation::ForwardFused | Operation::ForwardSeparate => self.forward()?,
            Operation::Inductance | Operation::Capacitance => {
                let [products, denominators] = self.inputs.as_slice() else {
                    return Err(RecipeError::Domain);
                };
                let mut output = self.output.iter_mut();
                for product in black_box(products) {
                    for (index, denominator) in black_box(denominators).iter().enumerate() {
                        let value = if products.len() > 1 {
                            product * self.prepared[index]
                        } else {
                            product / denominator
                        };
                        *output.next().ok_or(RecipeError::Domain)? = value;
                    }
                }
            }
            Operation::InverseTime => {
                let [phases, _, pi] = self.inputs.as_slice() else {
                    return Err(RecipeError::Domain);
                };
                let mut output = self.output.iter_mut();
                for phase in black_box(phases) {
                    for cycle in black_box(&self.cycles) {
                        let cycle = cycle_value(*cycle)?;
                        *output.next().ok_or(RecipeError::Domain)? =
                            (phase + 2.0 * pi[0] * cycle) * self.prepared[0];
                    }
                }
            }
            _ => return Err(RecipeError::Domain),
        }
        black_box(self.output.as_slice());
        Ok(())
    }

    /// Reset output storage outside the kernel timer.
    pub(super) fn reset(&mut self) {
        self.output.fill(0.0);
    }

    /// Each repeated pass consumes the complete output before reuse.
    pub(super) fn execute(&mut self, length: u64) -> Result<(), RecipeError> {
        for _ in 0..length {
            if self.scope == Scope::PreparationInclusive {
                self.prepared = Vec::new();
                self.prepare_shared()?;
                let outputs = self.output.len();
                self.output = Vec::new();
                self.output = zeros(outputs, &mut self.remaining_budget()?)?;
            }
            self.traverse()?;
        }
        Ok(())
    }

    /// Dry check permits explicitly intentional zero phase/time outputs, never nonfinite ones.
    fn validate_output(&self) -> Result<(), RecipeError> {
        let phase_output = matches!(
            self.operation,
            Operation::ForwardFused | Operation::ForwardSeparate | Operation::InverseTime
        );
        if self
            .output
            .iter()
            .any(|value| !(value.is_normal() || phase_output && *value == 0.0))
        {
            return Err(RecipeError::Domain);
        }
        Ok(())
    }

    /// Validate named intermediate domains without adding checks to timed traversals.
    fn validate_intermediates(&self) -> Result<(), RecipeError> {
        match self.operation {
            Operation::ForwardFused | Operation::ForwardSeparate => {
                let [_, _, times, _] = self.inputs.as_slice() else {
                    return Err(RecipeError::Domain);
                };
                for omega in &self.prepared {
                    for time in times {
                        if *time != 0.0 {
                            ordinary(time * omega)?;
                        }
                    }
                }
            }
            Operation::InverseTime => {
                let [phases, _, pi] = self.inputs.as_slice() else {
                    return Err(RecipeError::Domain);
                };
                let turn = ordinary(2.0 * pi[0])?;
                for phase in phases {
                    for cycle in &self.cycles {
                        let offset = if *cycle == 0 {
                            0.0
                        } else {
                            ordinary(turn * cycle_value(*cycle)?)?
                        };
                        let numerator = phase + offset;
                        if numerator != 0.0 {
                            ordinary(numerator)?;
                        }
                    }
                }
            }
            Operation::Inductance | Operation::Capacitance => {}
            _ => return Err(RecipeError::Domain),
        }
        Ok(())
    }

    /// Complete item dry evaluation outside timing.
    pub(super) fn validate(&mut self, length: u64) -> Result<(), RecipeError> {
        self.execute(length)?;
        self.validate_output()?;
        self.reset();
        Ok(())
    }
}

/// Binary64 exactly represents every admitted signed cycle index.
#[expect(
    clippy::cast_precision_loss,
    reason = "cycle indices are checked within 2^53-1"
)]
const fn cycle_value(value: i64) -> Result<f64, RecipeError> {
    if value.unsigned_abs() > crate::decimal::MAX_INTEGER {
        return Err(RecipeError::Overflow);
    }
    Ok(value as f64)
}

/// Explicit input shapes determine the output population before any allocation.
pub(super) fn population(case: &CaseRequest) -> Result<usize, RecipeError> {
    if case.inputs.iter().any(Vec::is_empty) {
        return Err(RecipeError::Domain);
    }
    match (case.operation, case.inputs.as_slice()) {
        (Operation::ForwardFused | Operation::ForwardSeparate, [_, _, _, _])
        | (Operation::Inductance | Operation::Capacitance, [_, _]) => case
            .inputs
            .iter()
            .try_fold(1_usize, |total, values| total.checked_mul(values.len())),
        (Operation::InverseTime, [phases, _, _]) => phases.len().checked_mul(case.cycles.len()),
        _ => return Err(RecipeError::Domain),
    }
    .filter(|n| *n > 0)
    .ok_or(RecipeError::Overflow)
}

#[cfg(test)]
mod tests {
    //! Hand-derived nonsquare and exceptional-domain oracles, without timing.

    use super::{Lc, Operation, RecipeError, Scope};

    /// Two L values, three C values and two times have a known factored-root traversal.
    fn forward(operation: Operation) -> Lc {
        Lc {
            operation,
            scope: Scope::Prepared,
            byte_cap: 4096,
            inputs: vec![
                vec![1.0, 4.0],
                vec![1.0, 4.0, 16.0],
                vec![0.0, 8.0],
                vec![1.0],
            ],
            cycles: Vec::new(),
            prepared: Vec::new(),
            output: vec![0.0; 12],
        }
    }

    #[test]
    fn roots_coexist_with_inputs_output_and_pair_preparation() -> Result<(), RecipeError> {
        // Four input headers, eight input scalars, twelve outputs, five roots,
        // and six pair frequencies coexist during this nonsquare preparation.
        let bytes = size_of::<Lc>() + 4 * size_of::<Vec<f64>>() + (8 + 12 + 5 + 6) * 8;
        let mut kernel = forward(Operation::ForwardFused);
        kernel.byte_cap = u64::try_from(bytes - 1).map_err(|_| RecipeError::Overflow)?;
        assert_eq!(kernel.prepare_shared(), Err(RecipeError::Allocation));
        kernel.byte_cap += 1;
        kernel.prepare_shared()?;
        assert_eq!(kernel.prepared.len(), 6);
        Ok(())
    }

    #[test]
    fn nonsquare_forward_preserves_time_inner_order() -> Result<(), RecipeError> {
        for operation in [Operation::ForwardFused, Operation::ForwardSeparate] {
            let mut kernel = forward(operation);
            kernel.prepare_shared()?;
            kernel.traverse()?;
            kernel.validate_intermediates()?;
            kernel.validate_output()?;
            assert_eq!(
                kernel.output,
                [1.0, 9.0, 1.0, 5.0, 1.0, 3.0, 1.0, 5.0, 1.0, 3.0, 1.0, 2.0]
            );
        }
        Ok(())
    }

    #[test]
    fn finite_phase_output_does_not_hide_subnormal_product() -> Result<(), RecipeError> {
        let mut kernel = forward(Operation::ForwardFused);
        kernel.inputs[2] = vec![f64::MIN_POSITIVE];
        kernel.output = vec![0.0; 6];
        kernel.prepare_shared()?;
        kernel.traverse()?;
        assert_eq!(kernel.validate_output(), Ok(()));
        assert_eq!(kernel.validate_intermediates(), Err(RecipeError::Domain));
        Ok(())
    }

    #[test]
    fn shared_element_reciprocals_and_finite_inverse_branches() -> Result<(), RecipeError> {
        for operation in [Operation::Inductance, Operation::Capacitance] {
            let mut kernel = Lc {
                operation,
                scope: Scope::Prepared,
                byte_cap: 4096,
                inputs: vec![vec![8.0, 16.0], vec![2.0, 4.0]],
                cycles: Vec::new(),
                prepared: Vec::new(),
                output: vec![0.0; 4],
            };
            kernel.prepare_shared()?;
            kernel.traverse()?;
            assert_eq!(kernel.output, [4.0, 2.0, 8.0, 4.0]);
        }
        let mut kernel = Lc {
            operation: Operation::InverseTime,
            scope: Scope::Prepared,
            byte_cap: 4096,
            inputs: vec![vec![2.0, 4.0], vec![2.0], vec![3.0]],
            cycles: vec![-1, 0, 1],
            prepared: Vec::new(),
            output: vec![0.0; 6],
        };
        kernel.prepare_shared()?;
        kernel.traverse()?;
        kernel.validate_intermediates()?;
        assert_eq!(kernel.output, [-2.0, 1.0, 4.0, -1.0, 2.0, 5.0]);
        Ok(())
    }
}
