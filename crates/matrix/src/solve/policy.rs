// SPDX-License-Identifier: AGPL-3.0-only
//! Local budgets name the homogeneous output difference coordinates explicitly.

use super::SolveError;
use crate::RegionContract;
use nrz_accuracy::{Budget, BudgetAssessment, Nonnegative};
use nrz_scalar::{ArithmeticContext, Scalar};
use nrz_units::{Descriptor, Role};

/// A Frobenius budget in one explicitly named homogeneous difference measure.
///
/// The absolute tolerance and positive characteristic scale both use `quantity`;
/// the relative coefficient is a ratio. Point outputs use their difference
/// descriptor. Coefficients are represented numbers or certified coefficient
/// enclosures supplied through the accuracy budget, never physical uncertainties.
#[derive(Clone, Debug)]
pub struct SolveBudget {
    /// Absolute tolerance and characteristic scale interpretation.
    quantity: Descriptor,
    /// Validated nonnegative coefficients and a strictly positive difference scale.
    numerical: Budget<f64>,
}

impl SolveBudget {
    /// Name the physical difference coordinates of all budget coefficients.
    /// # Errors
    /// Refuses a point descriptor; the operation additionally checks the exact
    /// output kind and measure before any backend computation.
    pub fn frobenius(quantity: Descriptor, numerical: Budget<f64>) -> Result<Self, SolveError> {
        if quantity.role() != Role::Difference {
            return Err(SolveError::BudgetContract);
        }
        Ok(Self {
            quantity,
            numerical,
        })
    }
    /// Common descriptor of the absolute tolerance and characteristic difference scale.
    #[must_use]
    pub const fn descriptor(&self) -> &Descriptor {
        &self.quantity
    }
    /// Validated numerical coefficients in those coordinates.
    #[must_use]
    pub const fn numerical(&self) -> Budget<f64> {
        self.numerical
    }
}

/// Bounded deterministic solve policy; no refinement or precision promotion is implied.
#[derive(Clone, Debug, Default)]
pub struct SolvePolicy {
    /// Absence is retained as `NotRequested`, never relabeled as a met budget.
    budget: Option<SolveBudget>,
    /// Explicit finite refinement and cumulative solve-work policy.
    recovery: Option<(nrz_accuracy::RecoveryPolicy, nrz_accuracy::Limits)>,
    /// Optional bounded external record identities, without numerical authority.
    keys: Option<super::SolveRecordKeys>,
}

impl SolvePolicy {
    /// Require this named output-norm budget before publishing any result.
    #[must_use]
    pub const fn with_budget(budget: SolveBudget) -> Self {
        Self {
            budget: Some(budget),
            recovery: None,
            keys: None,
        }
    }
    /// Requested local budget, if any.
    #[must_use]
    pub const fn budget(&self) -> Option<&SolveBudget> {
        self.budget.as_ref()
    }
    /// Check absolute/scale kinds, roles and measures against the actual restored output.
    pub(crate) fn validate(&self, output: &RegionContract) -> Result<(), SolveError> {
        if let Some(budget) = &self.budget {
            if budget.quantity != output.descriptor().difference() {
                return Err(SolveError::BudgetContract);
            }
            budget.numerical.tolerance(&ArithmeticContext::new()?)?;
        }
        Ok(())
    }
    /// Compare an exactly widened certified radius using accuracy's lower tolerance endpoint.
    pub(crate) fn assess<P: Scalar>(&self, radius: P) -> Result<BudgetAssessment<f64>, SolveError> {
        let context = ArithmeticContext::new()?;
        let upper = Nonnegative::new(radius.to_f64())?;
        Ok(nrz_accuracy::assess_budget(
            self.budget.as_ref().map(|budget| &budget.numerical),
            upper,
            &context,
        )?)
    }
}

impl SolvePolicy {
    /// Authorize finite refinement and optional fresh f64 retry under cumulative limits.
    #[must_use]
    pub const fn with_recovery(
        mut self,
        policy: nrz_accuracy::RecoveryPolicy,
        limits: nrz_accuracy::Limits,
    ) -> Self {
        self.recovery = Some((policy, limits));
        self
    }
    /// Bounded recovery settings; absence preserves single-attempt solve behavior.
    #[must_use]
    pub const fn recovery(&self) -> Option<(nrz_accuracy::RecoveryPolicy, nrz_accuracy::Limits)> {
        self.recovery
    }
}

impl SolvePolicy {
    /// Retain optional external plan/binding/point/node identities in this operation's record.
    #[must_use]
    pub fn with_record_keys(mut self, keys: super::SolveRecordKeys) -> Self {
        self.keys = Some(keys);
        self
    }
    /// External request identities, independently of heap and execution identities.
    #[must_use]
    pub const fn record_keys(&self) -> Option<&super::SolveRecordKeys> {
        self.keys.as_ref()
    }
}
