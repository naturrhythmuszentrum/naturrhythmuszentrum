// SPDX-License-Identifier: AGPL-3.0-only
//! Proof contexts, hand-selected scalar leaves and conditional cancellation setup.
//!
//! Compiled only in the public test target.

use crate::test_api::{
    AssumptionDraft, Cas, Draft, Environment, Field, Handle, Limits, Operator, PredicateKind,
    Primitive, ProofStep, Rational, Reference,
};

use super::budgets::proof_budget as budget;
use super::builders::{self, at};
use super::common::{TestResult, real};
use super::witness::equality;

/// Shared small symbolic universe used only by this new test suite.
#[derive(Debug)]
pub(crate) struct Fixture {
    /// Independent graph store.
    pub(crate) cas: Cas,
    /// Full context for every proof in the fixture.
    pub(crate) context: Handle,
    /// Original declaration header, used when explicitly adding hypotheses.
    pub(crate) environment: Environment,
    /// First free coordinate.
    pub(crate) x: Handle,
    /// Independent second coordinate.
    pub(crate) y: Handle,
    /// Exact zero.
    pub(crate) zero: Handle,
    /// Exact one.
    pub(crate) one: Handle,
    /// Exact two.
    pub(crate) two: Handle,
}

impl Fixture {
    /// Admit declarations and five leaf terms as one transaction.
    pub(crate) fn new() -> TestResult<Self> {
        let environment = builders::environment(Field::Real, &["x", "y"]);
        let mut cas = Cas::new(Limits::default())?;
        let mut drafts = vec![Draft::Context {
            environment: environment.clone(),
            assumptions: Vec::new(),
        }];
        for operator in [
            Operator::Free("x".into()),
            Operator::Free("y".into()),
            Operator::Rational(Rational::zero()),
            Operator::Rational(Rational::one()),
            Operator::Rational(Rational::from_ratio(2, 1, &mut budget()?)?),
        ] {
            drafts.push(builders::expression_draft(
                operator,
                real(),
                Reference::Local(0),
                Vec::new(),
                None,
                &[],
            ));
        }
        let handles = cas.admit(&drafts, &mut budget()?)?;
        Ok(Self {
            cas,
            context: at(&handles, 0)?,
            environment,
            x: at(&handles, 1)?,
            y: at(&handles, 2)?,
            zero: at(&handles, 3)?,
            one: at(&handles, 4)?,
            two: at(&handles, 5)?,
        })
    }

    /// Build exact ordered scalar syntax without normalization.
    pub(crate) fn term(&mut self, operator: Operator, operands: &[Handle]) -> TestResult<Handle> {
        builders::expression(
            &mut self.cas,
            &self.context,
            operator,
            real(),
            operands,
            &mut budget()?,
        )
    }

    /// Add one explicitly named hypothesis while preserving the semantic header.
    pub(crate) fn assume(&mut self, key: &str, kind: PredicateKind, term: &Handle) -> TestResult {
        let handles = self.cas.admit(
            &[
                Draft::Condition {
                    kind,
                    context: Reference::Existing(self.context.clone()),
                    scope: Vec::new(),
                    arguments: vec![Reference::Existing(term.clone())],
                },
                Draft::Context {
                    environment: self.environment.clone(),
                    assumptions: vec![AssumptionDraft {
                        key: key.into(),
                        condition: Reference::Local(0),
                    }],
                },
            ],
            &mut budget()?,
        )?;
        self.context = at(&handles, 1)?;
        Ok(())
    }
}

/// A cancellation fixture whose independently known domain is exactly x nonzero.
pub(crate) fn cancellation() -> TestResult<(Fixture, Handle, ProofStep)> {
    let mut fixture = Fixture::new()?;
    let quotient = fixture.term(Operator::Div, &[fixture.x.clone(), fixture.x.clone()])?;
    let step = equality(Primitive::Cancel, &quotient, &fixture.one);
    Ok((fixture, quotient, step))
}
