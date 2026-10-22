// SPDX-License-Identifier: AGPL-3.0-only
//! Untrusted finite sign recipes; replay reconstructs every ordered-field inference.

use super::leaf::{Leaf, Signed};
use crate::error::Fault;
use crate::graph::{Draft, Handle, Reference};
use crate::model::{Operator, PredicateKind};
use crate::proof_model::{Certificate, DomainContract, Statement};
use crate::solving::coefficients::storage::{invalid, vector};
use crate::solving::{
    CertificateRealSign, RealSignConclusion as Sign, RealSignRule as Rule, RealSignStep,
    SolveCertificate, SolveSchema, SolveStatement, StatementRealSign,
};

impl Leaf<'_> {
    /// Ordinary scalar syntax keeps every operand and its Real sort visible.
    pub(crate) fn expression(
        &mut self,
        operator: Operator,
        operands: &[Handle],
    ) -> Result<Handle, Fault> {
        self.frame
            .expression(self.cas, operator, operands, false, self.budget)
    }

    /// Small exact constants are graph literals, never producer sign bytes.
    pub(crate) fn number(&mut self, numerator: i64, denominator: u64) -> Result<Handle, Fault> {
        let value = crate::number::Rational::from_ratio(numerator, denominator, self.budget)?;
        self.expression(Operator::Rational(value), &[])
    }

    /// Import exact prior subjects before applying one final finite rule.
    pub(crate) fn derive(
        &mut self,
        rule: Rule,
        subject: &Handle,
        sign: Sign,
        arguments: &[&Signed],
        premises: Vec<u32>,
        exponent: u32,
    ) -> Result<Signed, Fault> {
        let mut steps = vector(arguments.len().saturating_add(1), self.budget)?;
        let mut indices = vector(arguments.len(), self.budget)?;
        for argument in arguments {
            indices.push(u32::try_from(steps.len()).map_err(|_| invalid())?);
            steps.push(RealSignStep {
                rule: Rule::Hypothesis,
                subject: argument.subject.clone(),
                conclusion: argument.sign,
                arguments: Vec::new(),
                premises: vec![argument.slot],
                exponent: 0,
            });
        }
        let root = u32::try_from(steps.len()).map_err(|_| invalid())?;
        steps.push(RealSignStep {
            rule,
            subject: subject.clone(),
            conclusion: sign,
            arguments: indices,
            premises,
            exponent,
        });
        let region = self.frame.region.clone();
        let slot = self.frame.push(
            SolveSchema::RealSign,
            SolveStatement::RealSign(StatementRealSign {
                subject: subject.clone(),
                region: region.clone(),
                conclusion: sign,
            }),
            SolveCertificate::RealSign(CertificateRealSign {
                subject: subject.clone(),
                region,
                steps,
                root,
            }),
            self.budget,
        )?;
        Ok(Signed {
            subject: subject.clone(),
            sign,
            slot,
        })
    }

    /// Exact reconstruction supplies a defined equality, including selected-root equations.
    pub(crate) fn equality(&mut self, left: &Handle, right: &Handle) -> Result<u32, Fault> {
        let difference = self.expression(Operator::Sub, &[left.clone(), right.clone()])?;
        let proof = self.derive(
            Rule::Identity,
            &difference,
            Sign::Zero,
            &[],
            vec![self.unit],
            0,
        )?;
        let region = self.frame.region.clone();
        self.frame.steps.last_mut().ok_or_else(invalid)?.conclusion = Statement::Equality {
            left: left.clone(),
            right: right.clone(),
            domain: region,
            domain_contract: DomainContract::DefinedAndEqual,
        };
        Ok(proof.slot)
    }

    /// Sign transport consumes exactly the checked equality joining both scalar subjects.
    pub(crate) fn transport(&mut self, source: &Signed, target: &Handle) -> Result<Signed, Fault> {
        let equality = self.equality(&source.subject, target)?;
        self.derive(
            Rule::Identity,
            target,
            source.sign,
            &[source],
            vec![equality],
            0,
        )
    }

    /// Expose a strict sign as an ordinary fact needed by existing division or square-root checkers.
    pub(crate) fn predicate(&mut self, source: &Signed, kind: PredicateKind) -> Result<u32, Fault> {
        let predicate = super::construction::admit(
            self.cas,
            &Draft::Condition {
                header: Reference::Existing(self.frame.header.clone()),
                scope: Vec::new(),
                kind,
                arguments: vec![Reference::Existing(source.subject.clone())],
            },
            self.budget,
        )?;
        let region = self.frame.region.clone();
        let certificate = CertificateRealSign {
            subject: source.subject.clone(),
            region: region.clone(),
            root: 0,
            steps: vec![RealSignStep {
                rule: Rule::Hypothesis,
                subject: source.subject.clone(),
                conclusion: source.sign,
                arguments: Vec::new(),
                premises: vec![source.slot],
                exponent: 0,
            }],
        };
        self.frame.append(
            crate::rule::Primitive::Solve(SolveSchema::RealSign),
            Statement::PredicateFact { predicate, region },
            Certificate::Solve(Box::new(SolveCertificate::RealSign(certificate))),
            self.budget,
        )
    }
}
