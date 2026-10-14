// SPDX-License-Identifier: AGPL-3.0-only
//! Untrusted derivative recipes consume checked branch derivatives and actual owned points.

use crate::boundary_model::{
    BoundaryCertificate, CaseDerivative, NonexistenceReason, PointDerivative, PointNonexistence,
};
use crate::boundary_producer::{Runner, pending};
use crate::case_syntax::invalid;
use crate::domain_model::Side;
use crate::error::Fault;
use crate::graph::Handle;
use crate::proof_model::{Ownership, Statement};
use crate::rule::Primitive;

/// Exact original function and requested calculus semantics.
pub(crate) struct DerivativeJob {
    /// Original input.
    pub(crate) function: Handle,
    /// Requested real domain.
    pub(crate) domain: Handle,
    /// Explicit coordinate.
    pub(crate) coordinate: String,
}

impl Runner<'_> {
    /// Obtain one existing scalar derivative proof and account for every expanded transition.
    fn scalar_derivative(
        &mut self,
        polynomial: &Handle,
        coordinate: &str,
    ) -> Result<(Handle, Handle), Fault> {
        let mut deriver = crate::calculus::Deriver::default();
        let (value, root) = deriver.derive(
            self.cas,
            self.request.context.logical_id(),
            polynomial.logical_id(),
            coordinate,
            self.generation,
        )?;
        let selected = deriver.plan.selected(root, self.generation)?;
        self.plan
            .account_scalar(&selected, self.cas, self.generation)?;
        let witness = selected.verify(self.cas, self.request.context.logical_id(), self.checker)?;
        Ok((self.cas.handle(value), witness.proof))
    }

    /// Transport a branch derivative on a nonempty open cell.
    pub(crate) fn interior(
        &mut self,
        job: &DerivativeJob,
        current: &Handle,
        region: &Handle,
        order: u32,
        prefix: Option<&Handle>,
    ) -> Result<Handle, Fault> {
        let (polynomial, agreement) = self.germ(current, region)?;
        let (derivative, branch) = self.scalar_derivative(&polynomial, &job.coordinate)?;
        let mut premises: Vec<_> = prefix.into_iter().cloned().collect();
        premises.extend([agreement, branch]);
        let proof = self.push(
            Primitive::CaseInterior,
            Statement::CaseDerivative(Box::new(CaseDerivative {
                function: job.function.clone(),
                derivative,
                coordinate: job.coordinate.clone(),
                order,
                domain: region.clone(),
            })),
            &premises,
            Some(BoundaryCertificate::Interior),
        )?;
        self.retain(&proof)?;
        Ok(proof)
    }

    /// Generate both genuine incident jets before proposing a point derivative or obstruction.
    pub(crate) fn point(
        &mut self,
        job: &DerivativeJob,
        current: &Handle,
        point: &Handle,
        order: u32,
        prefix: Option<&Handle>,
    ) -> Result<Handle, Fault> {
        let owner = self.ownership(current, point)?;
        let mut premises: Vec<_> = prefix.into_iter().cloned().collect();
        premises.push(owner.clone());
        let mut jets = Vec::new();
        for side in [Side::Below, Side::Above] {
            let region = self.incident(current, &job.domain, point, side, &job.coordinate)?;
            let proof = self.jet(current, &region, point, side, 1, &job.coordinate)?;
            jets.push(proof.clone());
            premises.push(proof);
        }
        let (slope, reason) = self.point_proposal(&owner, &jets)?;
        let claim = reason.map_or_else(
            || {
                Statement::DerivativeAt(Box::new(PointDerivative {
                    function: job.function.clone(),
                    boundary: point.clone(),
                    coordinate: job.coordinate.clone(),
                    order,
                    value: slope,
                }))
            },
            |reason| {
                Statement::DerivativeDoesNotExist(Box::new(PointNonexistence {
                    function: job.function.clone(),
                    boundary: point.clone(),
                    coordinate: job.coordinate.clone(),
                    order,
                    reason,
                }))
            },
        );
        let proof = self.push(
            Primitive::CaseBoundary,
            claim,
            &premises,
            Some(BoundaryCertificate::Point),
        )?;
        self.retain(&proof)?;
        Ok(proof)
    }

    /// Suggest the finite obstruction; the primitive independently checks the complete evidence.
    fn point_proposal(
        &mut self,
        owner: &Handle,
        jets: &[Handle],
    ) -> Result<(Handle, Option<NonexistenceReason>), Fault> {
        let Statement::Ownership {
            value: Ownership::Present(actual),
            ..
        } = self.statement(owner)?
        else {
            return Err(pending());
        };
        let mut values = Vec::new();
        for jet in jets {
            let Statement::Jet {
                values: entries, ..
            } = self.statement(jet)?
            else {
                return Err(invalid());
            };
            values.push((
                entries.first().ok_or_else(invalid)?.clone(),
                entries.get(1).ok_or_else(invalid)?.clone(),
            ));
        }
        let [(left, left_slope), (right, right_slope)] = values.as_slice() else {
            return Err(invalid());
        };
        let actual = crate::boundary_check::rational(self.cas, &actual)?;
        let reason = if actual != crate::boundary_check::rational(self.cas, left)? {
            Some(NonexistenceReason::Discontinuous(Side::Below))
        } else if actual != crate::boundary_check::rational(self.cas, right)? {
            Some(NonexistenceReason::Discontinuous(Side::Above))
        } else if crate::boundary_check::rational(self.cas, left_slope)?
            != crate::boundary_check::rational(self.cas, right_slope)?
        {
            Some(NonexistenceReason::UnequalSlopes)
        } else {
            None
        };
        Ok((left_slope.clone(), reason))
    }

    /// Expand the derived jump recipe into projections, exact subtraction and introduction.
    pub(crate) fn jump(
        &mut self,
        job: &DerivativeJob,
        point: &Handle,
        order: u32,
    ) -> Result<Handle, Fault> {
        let mut projections = Vec::new();
        let mut values = Vec::new();
        for side in [Side::Below, Side::Above] {
            let region = self.incident(&job.function, &job.domain, point, side, &job.coordinate)?;
            let jet = self.jet(&job.function, &region, point, side, order, &job.coordinate)?;
            let (proof, value) = self.projection(&jet, order)?;
            projections.push(proof);
            values.push(value);
        }
        let [left, right] = values.as_slice() else {
            return Err(invalid());
        };
        let difference = crate::boundary_check::rational(self.cas, right)?.sub(
            crate::boundary_check::rational(self.cas, left)?,
            self.generation,
        )?;
        let value = crate::boundary_regions::term(
            self.cas,
            &job.domain,
            crate::model::Operator::Rational(difference),
            &[],
            self.generation,
        )?;
        let subtraction = crate::boundary_regions::term(
            self.cas,
            &job.domain,
            crate::model::Operator::Sub,
            &[right.clone(), left.clone()],
            self.generation,
        )?;
        let domain = self.cas.handle(
            self.cas
                .state
                .context(self.request.context.logical_id())?
                .admissible,
        );
        let exact = self.push(
            Primitive::Exact,
            Statement::Equality {
                left: subtraction,
                right: value.clone(),
                domain,
                domain_contract: crate::proof_model::DomainContract::DefinedAndEqual,
            },
            &[],
            None,
        )?;
        projections.push(exact);
        let proof = self.push(
            Primitive::JetPolynomial,
            Statement::Jump {
                function: job.function.clone(),
                boundary: point.clone(),
                order,
                value,
            },
            &projections,
            Some(BoundaryCertificate::Jump),
        )?;
        self.retain(&proof)?;
        Ok(proof)
    }
}
