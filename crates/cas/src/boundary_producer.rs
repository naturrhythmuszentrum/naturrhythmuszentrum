// SPDX-License-Identifier: AGPL-3.0-only
//! Untrusted boundary recipes with separate generation and checking ledgers.

use crate::boundary_model::{BoundaryCertificate, TraceValue};
use crate::boundary_regions as syntax;
use crate::budget::Budget;
use crate::case_producer::Plan;
use crate::case_syntax::invalid;
use crate::domain_model::Side;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle};
use crate::model::Operator;
use crate::number::Rational;
use crate::proof_model::{Certificate, DomainContract, Statement, VerifiedWitness};
use crate::request_model::Request;
use crate::rule::Primitive;

/// One request shares its positive cost ledger and every monotone resource counter.
pub(crate) struct Runner<'a> {
    /// Unpublished candidate graph.
    pub(crate) cas: &'a mut Cas,
    /// Exact request identity and context.
    pub(crate) request: &'a Request,
    /// Expanded checked primitive path.
    pub(crate) plan: Plan<'a>,
    /// Producer resource ledger.
    pub(crate) generation: &'a mut Budget,
    /// Independent checker and fresh replay ledger.
    pub(crate) checker: &'a mut Budget,
    /// Only fully replayed facts survive a later pending obligation.
    pub(crate) facts: Vec<VerifiedWitness>,
    /// Checked facts awaiting replay through the enclosing derivative level.
    pub(crate) deferred: Option<Vec<Handle>>,
}

impl Runner<'_> {
    /// Check one proposed schema without granting authority to its producer.
    pub(crate) fn push(
        &mut self,
        rule: Primitive,
        claim: Statement,
        premises: &[Handle],
        mode: Option<BoundaryCertificate>,
    ) -> Result<Handle, Fault> {
        for object in crate::statement::objects(&claim) {
            if let Ok(value) = self.cas.state.expression(object.logical_id()) {
                self.plan
                    .expansion(value.unfolded, value.unfolded, self.generation)?;
            }
        }
        if let Statement::CaseDerivative(value) = &claim {
            let before = self
                .cas
                .state
                .expression(value.function.logical_id())?
                .unfolded;
            let after = self
                .cas
                .state
                .expression(value.derivative.logical_id())?
                .unfolded;
            self.plan.expansion(before, after, self.generation)?;
        }
        let certificate = mode.map_or(Certificate::None, |value| {
            Certificate::Boundary(Box::new(value))
        });
        self.plan.push(
            self.cas,
            &self.request.context,
            rule,
            claim,
            premises,
            certificate,
            self.generation,
            self.checker,
        )
    }

    /// Retain a fact immediately or queue it for replay through its derivative-level roots.
    pub(crate) fn retain(&mut self, proof: &Handle) -> Result<(), Fault> {
        if let Some(pending) = &mut self.deferred {
            let count = crate::graph::size(pending.len().saturating_add(1), AT)?;
            self.generation
                .check(crate::budget::BudgetKind::ProofNodes, count, AT)?;
            self.generation.check(
                crate::budget::BudgetKind::StagedPayloadBytes,
                count.saturating_mul(64),
                AT,
            )?;
            pending.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
            pending.push(proof.clone());
            return Ok(());
        }
        let witness = self.cas.seal_scoped(proof, self.checker)?;
        crate::case_producer::replay(self.cas, &witness, self.checker)?;
        self.keep(witness)
    }

    /// Publish a sealed fact only after its entire proof dependency closure has been replayed.
    pub(crate) fn keep(&mut self, witness: VerifiedWitness) -> Result<(), Fault> {
        let bytes = self.facts.iter().try_fold(
            crate::graph::size(witness.bytes.len(), AT)?,
            |total, fact| {
                total
                    .checked_add(crate::graph::size(fact.bytes.len(), AT)?)
                    .ok_or_else(invalid)
            },
        )?;
        self.generation
            .check(crate::budget::BudgetKind::StagedPayloadBytes, bytes, AT)?;
        self.facts
            .try_reserve(1)
            .map_err(|_| Fault::allocation(AT))?;
        self.facts.push(witness);
        Ok(())
    }

    /// Read the already checked theorem corresponding to an exact proof handle.
    pub(crate) fn statement(&mut self, proof: &Handle) -> Result<Statement, Fault> {
        Ok(
            crate::proof_view::application(self.cas, proof.logical_id(), self.generation)?
                .judgment
                .statement,
        )
    }

    /// Propose a branch germ, then certify total agreement over the entire neighborhood.
    pub(crate) fn germ(
        &mut self,
        function: &Handle,
        region: &Handle,
    ) -> Result<(Handle, Handle), Fault> {
        let analysis = crate::case_cells::analyze(
            self.cas,
            function.logical_id(),
            function.logical_id(),
            region.logical_id(),
            self.generation,
        )?;
        let value = analysis.germ(
            self.cas,
            function.logical_id(),
            region.logical_id(),
            self.generation,
        )?;
        let polynomial = self.cas.handle(value);
        let mut premises = Vec::new();
        let expression = self.cas.state.expression(function.logical_id())?.clone();
        if matches!(expression.operator, Operator::NamedCase(_)) {
            let definition = self
                .cas
                .handle(*expression.operands.first().ok_or_else(invalid)?);
            premises.push(self.push(
                Primitive::CaseCongruence,
                Statement::Equality {
                    left: function.clone(),
                    right: definition,
                    domain: region.clone(),
                    domain_contract: DomainContract::DefinedAndEqual,
                },
                &[],
                None,
            )?);
        }
        let proof = self.push(
            Primitive::CaseCongruence,
            Statement::Equality {
                left: function.clone(),
                right: polynomial.clone(),
                domain: region.clone(),
                domain_contract: DomainContract::DefinedAndEqual,
            },
            &premises,
            None,
        )?;
        Ok((polynomial, proof))
    }

    /// Produce jets by repeated differentiation and Horner evaluation.
    pub(crate) fn jet(
        &mut self,
        function: &Handle,
        region: &Handle,
        boundary: &Handle,
        side: Side,
        order: u32,
        coordinate: &str,
    ) -> Result<Handle, Fault> {
        let count = usize::try_from(order)
            .map_err(|_| invalid())?
            .checked_add(1)
            .ok_or_else(invalid)?;
        crate::jet_polynomial::measure(count, self.generation)?;
        let (polynomial, equality) = self.germ(function, region)?;
        let incidence = self.push(
            Primitive::RegionDecide,
            Statement::Incidence {
                region: region.clone(),
                boundary: boundary.clone(),
                side,
                neighborhood: region.clone(),
            },
            &[],
            None,
        )?;
        let coefficients = crate::polynomial::univariate(
            &self.cas.state,
            polynomial.logical_id(),
            coordinate,
            self.generation,
        )?;
        let point =
            crate::region::boundary_point(&self.cas.state, boundary.logical_id(), self.generation)?;
        let values = self.jet_values(region, &coefficients, &point, count)?;
        let proof = self.push(
            Primitive::JetPolynomial,
            Statement::Jet {
                function: function.clone(),
                region: region.clone(),
                boundary: boundary.clone(),
                side,
                values,
            },
            &[equality, incidence],
            Some(BoundaryCertificate::Polynomial {
                polynomial,
                coefficients,
            }),
        )?;
        self.retain(&proof)?;
        Ok(proof)
    }

    /// The producer's recurrence is deliberately different from the checker coefficient sum.
    fn jet_values(
        &mut self,
        region: &Handle,
        coefficients: &[Rational],
        point: &Rational,
        count: usize,
    ) -> Result<Vec<Handle>, Fault> {
        let mut polynomial = crate::univariate::Polynomial::new(coefficients, self.generation)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| Fault::allocation(AT))?;
        for _ in 0..count {
            let mut value = Rational::zero();
            for coefficient in polynomial.coefficients.iter().rev() {
                value = value
                    .mul(point, self.generation)?
                    .add(coefficient, self.generation)?;
            }
            result.push(syntax::term(
                self.cas,
                region,
                Operator::Rational(value),
                &[],
                self.generation,
            )?);
            polynomial = polynomial.derivative(self.generation)?;
        }
        Ok(result)
    }

    /// Function-specific ownership is derived from the existing exact case domain/select recipe.
    pub(crate) fn ownership(&mut self, function: &Handle, point: &Handle) -> Result<Handle, Fault> {
        let proof = crate::case_recipes::ownership(
            &mut self.plan,
            self.cas,
            &self.request.context,
            function,
            point,
            self.generation,
            self.checker,
        )?;
        self.retain(&proof)?;
        Ok(proof)
    }

    /// Choose an exact incident cell; an empty side is a pending premise.
    pub(crate) fn incident(
        &mut self,
        function: &Handle,
        domain: &Handle,
        point: &Handle,
        side: Side,
        coordinate: &str,
    ) -> Result<Handle, Fault> {
        let regions =
            crate::boundary_partition::inventory(self.cas, function, domain, self.generation)?;
        let roots: Vec<_> = regions.iter().map(Handle::logical_id).collect();
        let value =
            crate::region::boundary_point(&self.cas.state, point.logical_id(), self.generation)?;
        let cells =
            crate::region_cache::partition_at(self.cas, &roots, Some(&value), self.generation)?;
        let position = cells
            .cuts
            .iter()
            .position(|cut| *cut == value)
            .ok_or_else(invalid)?;
        let index = position
            .checked_mul(2)
            .and_then(|n| n.checked_add(usize::from(side == Side::Above) * 2))
            .ok_or_else(invalid)?;
        let region = syntax::cell(
            self.cas,
            domain,
            coordinate,
            &cells.cuts,
            index,
            self.generation,
        )?;
        if crate::case_obligations::empty(self.cas, region.logical_id(), self.generation)? {
            return Err(pending());
        }
        Ok(region)
    }

    /// A projection preserves all exact jet indexing information.
    pub(crate) fn projection(
        &mut self,
        jet: &Handle,
        order: u32,
    ) -> Result<(Handle, Handle), Fault> {
        let Statement::Jet {
            function,
            boundary,
            side,
            values,
            ..
        } = self.statement(jet)?
        else {
            return Err(invalid());
        };
        let index = usize::try_from(order).map_err(|_| invalid())?;
        let value = values.get(index).ok_or_else(invalid)?.clone();
        let proof = self.push(
            Primitive::JetPolynomial,
            Statement::TraceValue(Box::new(TraceValue {
                function,
                boundary,
                side,
                order,
                value: value.clone(),
            })),
            std::slice::from_ref(jet),
            Some(BoundaryCertificate::Projection),
        )?;
        Ok((proof, value))
    }
}

/// Boundary recipe location.
const AT: Location = Location::new("boundary calculus recipe");

/// Missing analytic evidence never asserts mathematical nonexistence.
pub(crate) const fn pending() -> Fault {
    Fault::invalid(InvalidCode::PendingPremise, AT)
}
