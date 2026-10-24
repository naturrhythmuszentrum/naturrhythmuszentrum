// SPDX-License-Identifier: AGPL-3.0-only
//! Publication authenticates actual owners against their original source, law and finite roles.

use super::{Channel, Role, Run, StatisticsAllocation, StatisticsInventory, plan::State};
use crate::{OperationError, PrivateAllocationError as Error, statistics::Matrix};
use nrz_scalar::{ArithmeticContext, Finite, Scalar};
use std::rc::Rc;

impl Run {
    /// Both channels must retain the original exact role, rectangle and initialized length.
    pub(crate) fn matches<P: Scalar>(&self, matrix: &Matrix<P>, role: Role, index: usize) -> bool {
        let Ok(center) = self.slot(role, index, Channel::Centers) else {
            return false;
        };
        let Ok(truth) = self.slot(role, index, Channel::Truth) else {
            return false;
        };
        matrix.extent == self.0.specifications[center].extent
            && matrix.centers.len() == matrix.extent.entries()
            && matrix.truth.len() == matrix.extent.entries()
            && matrix.centers.matches_statistics(&self.0, center)
            && matrix.truth.matches_statistics(&self.0, truth)
    }
    /// Borrow the original covered owner and validate its exact parameter role before copying.
    pub(crate) fn copies<P: Scalar + 'static>(
        matrix: &Matrix<P>,
        parameter: usize,
        context: &ArithmeticContext,
    ) -> Result<[crate::private::Buffer<P>; 2], OperationError> {
        let run = Self(Rc::clone(
            matrix
                .centers
                .statistics_plan()
                .ok_or(Error::UnticketedBuffer)?,
        ));
        let (source, published) = match parameter {
            0 => (Role::Covariance, Role::PublishedCovariance),
            1 => (Role::Sensitivity, Role::PublishedSensitivity),
            2 => (Role::Jacobian, Role::PublishedJacobian),
            3 => (Role::Mean, Role::PublishedMean),
            _ => return Err(Error::UnplannedMeaning.into()),
        };
        let slot = run
            .0
            .specifications
            .iter()
            .find(|entry| {
                entry.role == source
                    && entry.channel == Channel::Centers
                    && run.matches(matrix, source, entry.index)
            })
            .ok_or(Error::ForeignInventory)?
            .index;
        let mut centers = run.buffer(published, slot, Channel::Centers)?;
        let mut errors = run.buffer(published, slot, Channel::Errors)?;
        for (&center, &truth) in matrix.centers.iter().zip(&matrix.truth) {
            centers.push(center)?;
            errors.push(
                truth
                    .displacement_from(Finite::new(center)?, context)?
                    .get(),
            )?;
        }
        Ok([centers, errors])
    }
    /// Required requests must be complete; every still-live owner must match the actual payload.
    pub(crate) fn finish(
        &self,
        token: Option<u64>,
        live: impl Fn(Role, usize) -> bool,
    ) -> Result<Rc<StatisticsInventory>, OperationError> {
        if self.0.candidate.get() != token {
            return Err(Error::ForeignInventory.into());
        }
        let mut allocations = crate::binding::buffer(self.0.specifications.len())?;
        for (slot, entry) in self.0.specifications.iter().enumerate() {
            match entry.state.get() {
                State::Planned if entry.optional => continue,
                State::Released(len) if len == entry.extent.entries() => {}
                State::Live | State::Candidate if live(entry.role, entry.index) => {}
                _ => return Err(Error::IncompleteCoverage.into()),
            }
            allocations.push(StatisticsAllocation {
                plan: Rc::clone(&self.0),
                slot,
            });
        }
        Ok(Rc::new(StatisticsInventory {
            plan: Rc::clone(&self.0),
            allocations,
            peak: self.0.peak.get(),
            metadata: self.0.metadata,
        }))
    }
    /// Input captures and operating points remain live in the binding through preparation.
    fn captured<P: Scalar>(
        &self,
        plan: &crate::statistics::Plan<P>,
        role: Role,
        index: usize,
    ) -> bool {
        match role {
            Role::InputSensitivity => plan
                .inputs
                .get(index)
                .is_some_and(|input| self.matches(&input.sensitivity, role, index)),
            Role::InputMeans => plan
                .inputs
                .get(index)
                .and_then(|input| input.means.as_ref())
                .is_some_and(|matrix| self.matches(matrix, role, index)),
            Role::OperatingPoint => plan
                .points
                .get(index)
                .is_some_and(|matrix| self.matches(matrix, role, index)),
            _ => false,
        }
    }
    /// Captured numerical rows retain the exact original model binding and optional mean declaration.
    fn validate_inputs<P: Scalar>(
        &self,
        plan: &crate::statistics::Plan<P>,
    ) -> Result<(), OperationError> {
        if plan.inputs.len() != self.0.bindings.len() || plan.points.len() != plan.inputs.len() {
            return Err(Error::IncompleteCoverage.into());
        }
        for (index, (input, binding)) in plan.inputs.iter().zip(&self.0.bindings).enumerate() {
            if !Rc::ptr_eq(&input.lineage, &binding.lineage)
                || !self.captured(plan, Role::InputSensitivity, index)
                || !self.captured(plan, Role::OperatingPoint, index)
                || input.means.is_some() != binding.means
            {
                return Err(Error::ForeignInventory.into());
            }
            if input.means.is_some() && !self.captured(plan, Role::InputMeans, index) {
                return Err(Error::ForeignInventory.into());
            }
        }
        Ok(())
    }
    /// Complete generic output owners and input captures refer to the exact bound inventory.
    pub(crate) fn complete<P: Scalar, Q: Scalar>(
        &self,
        plan: &crate::statistics::Plan<P>,
        candidate: &crate::Candidate<Q>,
    ) -> Result<Rc<StatisticsInventory>, OperationError> {
        let solve = self
            .0
            .operation
            .as_ref()
            .is_some_and(|op| matches!(op.recipe, crate::binding::NativeRecipe::Solve(_)));
        self.validate_inputs(plan)?;
        self.validate_lineage(plan.lineage.as_ref().ok_or(Error::IncompleteCoverage)?)?;
        if !solve {
            for (index, payload) in candidate.statistics.iter().enumerate() {
                for (role, matrix) in [
                    (Role::Sensitivity, &payload.sensitivity),
                    (Role::Covariance, &payload.covariance),
                    (Role::Jacobian, &payload.jacobian),
                ] {
                    if !self.matches(matrix, role, index) {
                        return Err(Error::ForeignInventory.into());
                    }
                }
                if payload
                    .means
                    .as_ref()
                    .is_some_and(|matrix| !self.matches(matrix, Role::Mean, index))
                {
                    return Err(Error::ForeignInventory.into());
                }
            }
        }
        self.finish((!solve).then_some(candidate.token), |role, index| {
            if self.captured(plan, role, index) {
                return true;
            }
            let Some(payload) = candidate.statistics.get(index) else {
                return false;
            };
            let matrix = match role {
                Role::Sensitivity => Some(&payload.sensitivity),
                Role::Covariance => Some(&payload.covariance),
                Role::Jacobian => Some(&payload.jacobian),
                Role::Mean => payload.means.as_ref(),
                _ => None,
            };
            matrix.is_some_and(|matrix| self.matches(matrix, role, index))
        })
    }
}
