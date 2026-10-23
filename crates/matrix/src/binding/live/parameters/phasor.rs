// SPDX-License-Identifier: AGPL-3.0-only
//! One immutable statistical family retains the common source and every mean/covariance target version.

use super::{AT, CompanionEvaluation, CompanionExecution, CompanionExecutionError as Error};
use crate::{Heap, ReadingBinding};
use nrz_bindings::{
    Budget,
    readings::algebra::limits::{BudgetKind, BudgetUsage},
};
use nrz_scalar::{ArithmeticContext, Scalar};
use std::rc::Rc;

/// Complete affine current statistics evaluated from one shared voltage covariance source.
///
/// The family contains the two means, both variances and their signed cross-covariance,
/// with separate numerical parameter errors. It evaluates declared finite-moment data;
/// it does not certify a statistical model or publish PSD evidence into the heap.
/// Mean numerical radii remain attached to the retained certified target readings.
#[derive(Clone, Debug)]
pub struct PhasorStatistics<P: Scalar> {
    /// Left variance, cross block and right variance, all constructed before this result escapes.
    evaluations: [CompanionEvaluation<P>; 3],
    /// Both represented mean currents captured from their certified scalar result bindings.
    means: [P; 2],
    /// Conservative simultaneous immutable family and exact-plan payload allowance.
    bytes: u64,
    /// Complete cumulative caller usage before and after construction.
    usage: [BudgetUsage; 2],
}

impl<P: Scalar + 'static> PhasorStatistics<P> {
    /// Both restored mean current coordinates while all sources and output targets remain current.
    /// # Errors
    /// Refuses any changed source, parameter, mean target or arithmetic revision.
    pub fn means(&self, heap: &Heap, context: &ArithmeticContext) -> Result<[P; 2], Error> {
        self.moments(heap, context)?;
        Ok(self.means)
    }
    /// Left variance, signed cross-covariance and right variance, in that order.
    /// # Errors
    /// Refuses any changed source, parameter, mean target or arithmetic revision.
    pub fn moments(&self, heap: &Heap, context: &ArithmeticContext) -> Result<[P; 3], Error> {
        let mut result = [P::ZERO; 3];
        for (entry, evaluation) in result.iter_mut().zip(&self.evaluations) {
            let [value] = evaluation.values(heap, context)? else {
                return Err(Error::Identity);
            };
            *entry = *value;
        }
        Ok(result)
    }
    /// Numerical errors in the three covariance parameters, separate from physical variances.
    /// # Errors
    /// Refuses any changed source, parameter, mean target or arithmetic revision.
    pub fn parameter_errors(
        &self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<[P; 3], Error> {
        let mut result = [P::ZERO; 3];
        for (entry, evaluation) in result.iter_mut().zip(&self.evaluations) {
            let [value] = evaluation.errors(heap, context)? else {
                return Err(Error::Identity);
            };
            *entry = *value;
        }
        Ok(result)
    }
    /// All original equations, shared parameter identities and ordered target closures.
    #[must_use]
    pub const fn evaluations(&self) -> &[CompanionEvaluation<P>; 3] {
        &self.evaluations
    }
    /// Conservative retained payload charged beside the three complete evaluations.
    #[must_use]
    pub const fn retained_payload_bytes(&self) -> u64 {
        self.bytes
    }
    /// Complete cumulative caller usage; no per-block reset is introduced.
    #[must_use]
    pub const fn usage(&self) -> [BudgetUsage; 2] {
        self.usage
    }
}

impl Heap {
    /// Evaluate a complete shared-voltage statistical family through the original-source executor.
    ///
    /// The exact fixed-reactance companion laws must already exist. Both current means must
    /// come from their actual certified published scalar readings. A later refusal returns no
    /// statistical prefix; the independent deterministic solve families retain their own scope.
    /// # Errors
    /// Refuses unrelated sources, substituted current means, missing affine laws, stale versions and limits.
    pub fn evaluate_phasor_statistics<P: Scalar + 'static>(
        &self,
        executions: &[CompanionExecution<P>; 3],
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<PhasorStatistics<P>, Error> {
        let before = budget.usage();
        check(executions, budget)?;
        let bytes = storage(executions, budget)?;
        budget.with_retained_payload(bytes, AT, |budget| {
            let evaluations = [
                self.evaluate_companion(&executions[0], context, budget)?,
                self.evaluate_companion(&executions[1], context, budget)?,
                self.evaluate_companion(&executions[2], context, budget)?,
            ];
            let mut means = [P::ZERO; 2];
            for (value, execution) in means.iter_mut().zip([&executions[0], &executions[2]]) {
                let values = self.reading_entries(&execution.targets()[0])?;
                let [mean] = values.as_slice() else {
                    return Err(Error::Identity);
                };
                *value = *mean;
            }
            budget.check(BudgetKind::OutputBytes, bytes, AT)?;
            Ok(PhasorStatistics {
                evaluations,
                means,
                bytes,
                usage: [before, budget.usage()],
            })
        })
    }
}

/// All three covariance blocks must use one actual parameter/error pair and matching mean versions.
fn check<P: Scalar>(
    executions: &[CompanionExecution<P>; 3],
    budget: &mut Budget,
) -> Result<(), Error> {
    for execution in executions {
        budget.work(4, AT)?;
        if execution.targets().len() != 2 || execution.exact().target().phasor_law().is_none() {
            return Err(Error::Identity);
        }
        for (expected, actual) in executions[0].sources().iter().zip(execution.sources()) {
            if !same(expected, actual) {
                return Err(Error::Identity);
            }
        }
        for target in execution.targets() {
            if target.reading().entries().len() != 1 || target.phasor_solve().is_none() {
                return Err(Error::Identity);
            }
        }
    }
    let left = &executions[0].targets()[0];
    let right = &executions[2].targets()[0];
    let left_variance = same(left, &executions[0].targets()[1]);
    let right_variance = same(right, &executions[2].targets()[1]);
    let cross_left = same(left, &executions[1].targets()[0]);
    let cross_right = same(right, &executions[1].targets()[1]);
    if !left_variance || !right_variance || !cross_left || !cross_right {
        return Err(Error::Identity);
    }
    Ok(())
}

/// Equal exact descriptors cannot substitute another numerical selection or version group.
fn same<P: Scalar>(left: &ReadingBinding<P>, right: &ReadingBinding<P>) -> bool {
    Rc::ptr_eq(&left.group, &right.group) && left.index == right.index
}

/// Compose retained ownership by allocation identity, including each shared equation exactly once.
fn storage<P: Scalar>(
    executions: &[CompanionExecution<P>; 3],
    budget: &mut Budget,
) -> Result<u64, Error> {
    let mut bytes = 4096_u64;
    let mut seen: Vec<&nrz_bindings::readings::PhasorSolveDefinition> = Vec::with_capacity(6);
    for execution in executions {
        bytes = bytes
            .checked_add(execution.retained_payload_bytes())
            .ok_or(crate::HeapError::ExtentOverflow)?;
        let law = execution
            .exact()
            .target()
            .phasor_law()
            .ok_or(Error::Identity)?;
        for (index, equation) in law.equations().iter().enumerate() {
            budget.work(7, AT)?;
            if index == 1 && equation.shares_allocation(&law.equations()[0]) {
                continue;
            }
            if seen
                .iter()
                .any(|previous| equation.shares_allocation(previous))
            {
                bytes = bytes
                    .checked_sub(equation.retained_payload_bytes())
                    .ok_or(crate::HeapError::ExtentOverflow)?;
            } else {
                seen.push(equation);
            }
        }
    }
    Ok(bytes)
}
