// SPDX-License-Identifier: AGPL-3.0-only
//! Proof contexts, hand-selected scalar leaves and conditional cancellation setup.
//!
//! Compiled only in the public test target.

use crate::test_api::domain::AssumptionDraft;
use crate::test_api::{Cas, Handle, Rational, Reference};

use crate::test_api::proof::{ApplicationDraft, Primitive};
use crate::test_api::syntax::{Draft, Environment, Field, Operator, PredicateKind};

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
    /// Exact domain used in every scalar proposal.
    pub(crate) domain: Handle,
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
        let mut cas = Cas::new(budget()?.limits().clone())?;
        let context =
            builders::core_context(&mut cas, environment.clone(), Vec::new(), &mut budget()?)?;
        let domain = cas.admissible_region(&context)?;
        let header = cas.context_header(&context)?;
        let mut drafts = Vec::new();
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
                Reference::Existing(header.clone()),
                Vec::new(),
                None,
                &[],
            ));
        }
        let handles = cas.admit(&drafts, &mut budget()?)?;
        Ok(Self {
            cas,
            context,
            domain,
            environment,
            x: at(&handles, 0)?,
            y: at(&handles, 1)?,
            zero: at(&handles, 2)?,
            one: at(&handles, 3)?,
            two: at(&handles, 4)?,
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
        let header = self.cas.context_header(&self.context)?;
        let predicate = builders::admit(
            &mut self.cas,
            Draft::Condition {
                kind,
                header: Reference::Existing(header),
                scope: Vec::new(),
                arguments: vec![Reference::Existing(term.clone())],
            },
            &mut budget()?,
        )?;
        self.context = builders::core_context(
            &mut self.cas,
            self.environment.clone(),
            vec![AssumptionDraft {
                key: key.into(),
                condition: Reference::Existing(predicate),
            }],
            &mut budget()?,
        )?;
        self.domain = self.cas.admissible_region(&self.context)?;
        Ok(())
    }
}

/// A cancellation fixture whose independently known domain is exactly x nonzero.
pub(crate) fn cancellation() -> TestResult<(Fixture, Handle, ApplicationDraft)> {
    let mut fixture = Fixture::new()?;
    let quotient = fixture.term(Operator::Div, &[fixture.x.clone(), fixture.x.clone()])?;
    let step = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::Cancel,
        &quotient,
        &fixture.one,
    );
    Ok((fixture, quotient, step))
}
