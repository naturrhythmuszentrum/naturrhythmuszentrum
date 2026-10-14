// SPDX-License-Identifier: AGPL-3.0-only
//! Untrusted distribution recipes with one generation ledger and one replay ledger.

use crate::budget::Budget;
use crate::case_producer::Plan;
use crate::distribution_model::{
    DistributionCertificate, DistributionStatement, RegularCertificate, RegularityKind,
};
use crate::distribution_syntax::invalid;
use crate::error::Fault;
use crate::graph::{Cas, Handle, LogicalId};
use crate::model::Operator;
use crate::proof_model::{Certificate, Statement};
use crate::rule::Primitive;
use std::collections::BTreeMap;

/// One request's typed proposals and reusable, independently checked facts.
pub(crate) struct Runner<'a> {
    /// Unpublished candidate store.
    pub(crate) cas: &'a mut Cas,
    /// Exact request context.
    pub(crate) context: Handle,
    /// Full semantic header, including declared tests.
    pub(crate) header: Handle,
    /// Exact distribution space.
    pub(crate) space: Handle,
    /// Positive logical accounting and independent application checking.
    pub(crate) plan: Plan<'a>,
    /// Shared generation ledger.
    pub(crate) generation: &'a mut Budget,
    /// Shared independent checker ledger.
    pub(crate) checker: &'a mut Budget,
    /// Exact regularity facts keyed by original representative.
    pub(crate) regular: BTreeMap<LogicalId, (Handle, RegularCertificate)>,
    /// First derivatives keyed by complete distribution syntax.
    pub(crate) derivatives: BTreeMap<LogicalId, (Handle, Handle)>,
    /// Explicit supplied or generated membership prerequisites.
    pub(crate) premises: Vec<Handle>,
}

impl Runner<'_> {
    /// Every proposal enters the native checker before it can become a proof dependency.
    pub(crate) fn push(
        &mut self,
        rule: Primitive,
        conclusion: DistributionStatement,
        premises: &[Handle],
        certificate: Certificate,
    ) -> Result<Handle, Fault> {
        self.plan.push(
            self.cas,
            &self.context,
            rule,
            Statement::Distribution(Box::new(conclusion)),
            premises,
            certificate,
            self.generation,
            self.checker,
        )
    }

    /// Certify complete finite polynomial regularity once per representative and request.
    pub(crate) fn regular(
        &mut self,
        function: &Handle,
    ) -> Result<(Handle, RegularCertificate), Fault> {
        if let Some(value) = self.regular.get(&function.logical_id()) {
            return Ok(value.clone());
        }
        let certificate = crate::distribution_regular::certificate(
            self.cas,
            function,
            &self.space,
            self.generation,
        )?;
        let proof = self.push(
            Primitive::DistRegular,
            DistributionStatement::Regularity {
                function: function.clone(),
                space: self.space.clone(),
                property: RegularityKind::PiecewisePolynomial,
            },
            &[],
            Certificate::Distribution(Box::new(DistributionCertificate::Regular(Box::new(
                certificate.clone(),
            )))),
        )?;
        self.regular
            .insert(function.logical_id(), (proof.clone(), certificate.clone()));
        Ok((proof, certificate))
    }

    /// Explicit embedding always retains its integrability proof.
    pub(crate) fn embed(&mut self, function: &Handle) -> Result<(Handle, Handle), Fault> {
        let (regular, _) = self.regular(function)?;
        let embedding = self.node(Operator::RegularEmbed, std::slice::from_ref(function))?;
        let proof = self.push(
            Primitive::DistRegular,
            DistributionStatement::Embedding {
                function: function.clone(),
                embedding: embedding.clone(),
                space: self.space.clone(),
            },
            &[regular],
            Certificate::None,
        )?;
        self.premises.push(proof.clone());
        Ok((embedding, proof))
    }

    /// Typed construction does not copy a scalar header from a different space.
    pub(crate) fn node(
        &mut self,
        operator: Operator,
        operands: &[Handle],
    ) -> Result<Handle, Fault> {
        crate::distribution_terms::node(
            self.cas,
            &self.header,
            &self.space,
            operator,
            operands,
            self.generation,
        )
    }

    /// Reconstruct all regular-embedding prerequisites before using a distribution expression.
    pub(crate) fn ensure(&mut self, input: &Handle) -> Result<(), Fault> {
        let mut pending = vec![input.clone()];
        let mut seen = std::collections::BTreeSet::new();
        while let Some(term) = pending.pop() {
            self.generation.work(1, crate::distribution_syntax::AT)?;
            if !seen.insert(term.logical_id()) {
                continue;
            }
            let value = self.cas.state.expression(term.logical_id())?.clone();
            match value.operator {
                Operator::RegularEmbed => {
                    let function = self
                        .cas
                        .handle(*value.operands.first().ok_or_else(invalid)?);
                    let (proof, _) = self.regular(&function)?;
                    self.premises.push(proof);
                }
                Operator::DistributionAdd | Operator::DistributionDerivative { .. } => {
                    pending.extend(value.operands.iter().map(|id| self.cas.handle(*id)));
                }
                Operator::DistributionScale | Operator::SmoothMultiply => {
                    pending.push(self.cas.handle(*value.operands.get(1).ok_or_else(invalid)?));
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Iterate actual first derivatives and retain the exact original input and order.
    pub(crate) fn differentiate(
        &mut self,
        input: &Handle,
        order: u32,
    ) -> Result<(Handle, Handle), Fault> {
        if order == 0 {
            return Err(invalid());
        }
        self.generation.check(
            crate::budget::BudgetKind::Depth,
            u64::from(order),
            crate::distribution_syntax::AT,
        )?;
        let (mut output, mut proof) = self.once(input)?;
        for level in 2..=order {
            let (next, step) = self.once(&output)?;
            proof = self.push(
                Primitive::DistDerivative,
                DistributionStatement::Derivative {
                    distribution: input.clone(),
                    derivative: next.clone(),
                    space: self.space.clone(),
                    order: level,
                },
                &[proof, step],
                Certificate::None,
            )?;
            output = next;
        }
        Ok((output, proof))
    }

    /// Reuse first derivative facts without re-exporting or replaying a partial root.
    pub(crate) fn once(&mut self, input: &Handle) -> Result<(Handle, Handle), Fault> {
        if let Some(value) = self.derivatives.get(&input.logical_id()) {
            return Ok(value.clone());
        }
        self.ensure(input)?;
        let result =
            if self.cas.state.expression(input.logical_id())?.operator == Operator::RegularEmbed {
                match self.classical_bridge(input)? {
                    Some(result) => result,
                    None => self.jump(input)?,
                }
            } else {
                self.linear_derivative(input)?
            };
        self.derivatives.insert(input.logical_id(), result.clone());
        Ok(result)
    }

    /// Expand one regular derivative with all interior impulses.
    fn jump(&mut self, input: &Handle) -> Result<(Handle, Handle), Fault> {
        let function = crate::distribution_ae::representative(self.cas, input)?;
        let (source_proof, certificate) = self.regular(&function)?;
        let scalar = crate::distribution_jump::regular_output(
            self.cas,
            &function,
            &self.space,
            &certificate,
            self.generation,
        )?;
        let (target_proof, target) = self.regular(&scalar)?;
        let mut output = if target.polynomials.iter().flatten().all(Vec::is_empty) {
            self.node(Operator::DistributionZero, &[])?
        } else {
            self.node(Operator::RegularEmbed, &[scalar])?
        };
        for (point, coefficient) in
            crate::distribution_jump::impulses(&certificate, self.generation)?
        {
            let point = crate::distribution_terms::rational(
                self.cas,
                &self.header,
                &point,
                self.generation,
            )?;
            let delta = self.node(Operator::Delta { order: 0 }, &[point])?;
            let factor = crate::distribution_terms::rational(
                self.cas,
                &self.header,
                &coefficient,
                self.generation,
            )?;
            let term = self.node(Operator::DistributionScale, &[factor, delta])?;
            output = self.node(Operator::DistributionAdd, &[output, term])?;
        }
        output = self.normalize(&output)?;
        let proof = self.push(
            Primitive::DistJump,
            DistributionStatement::Derivative {
                distribution: input.clone(),
                derivative: output.clone(),
                space: self.space.clone(),
                order: 1,
            },
            &[source_proof, target_proof],
            Certificate::None,
        )?;
        Ok((output, proof))
    }

    /// Differentiate complete structural terms, then check rational collection independently.
    fn linear_derivative(&mut self, input: &Handle) -> Result<(Handle, Handle), Fault> {
        let terms =
            crate::distribution_linear::terms(self.cas, input.logical_id(), &[], self.generation)?;
        let mut premises = self.premises.clone();
        for (term, _) in terms.values() {
            let term = term.as_ref().ok_or_else(invalid)?;
            if self.cas.state.expression(term.logical_id())?.operator == Operator::RegularEmbed {
                premises.push(self.once(term)?.1);
            }
        }
        let earlier = self.applications(&premises)?;
        let output = crate::distribution_derivative::expand(
            self.cas,
            input,
            &self.space,
            &earlier,
            self.generation,
        )?;
        let output = self.normalize(&output)?;
        let proof = self.push(
            Primitive::DistDerivative,
            DistributionStatement::Derivative {
                distribution: input.clone(),
                derivative: output.clone(),
                space: self.space.clone(),
                order: 1,
            },
            &premises,
            Certificate::None,
        )?;
        Ok((output, proof))
    }

    /// Canonical coefficient collection remains a proposal until its enclosing rule checks it.
    pub(crate) fn normalize(&mut self, input: &Handle) -> Result<Handle, Fault> {
        let terms =
            crate::distribution_linear::terms(self.cas, input.logical_id(), &[], self.generation)?;
        crate::distribution_terms::linear(
            self.cas,
            &self.header,
            &self.space,
            &terms,
            false,
            self.generation,
        )
    }

    /// Read exact earlier applications without converting their judgments to scalar claims.
    pub(crate) fn applications(
        &mut self,
        proofs: &[Handle],
    ) -> Result<Vec<crate::proof_model::CheckedApplication>, Fault> {
        proofs
            .iter()
            .map(|proof| {
                crate::proof_view::application(self.cas, proof.logical_id(), self.generation)
            })
            .collect()
    }
}
