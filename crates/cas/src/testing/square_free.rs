// SPDX-License-Identifier: AGPL-3.0-only
//! Independent univariate inputs and untrusted multiplicity-certificate proposals.
//!
//! Compiled only in the public test target.

use crate::test_api::{Cas, Handle, Rational};

use crate::test_api::proof::{ApplicationDraft, Certificate, Primitive, SquareFreeCertificate};
use crate::test_api::request::{Outcome, Request, SearchResult, Task};
use crate::test_api::syntax::{AtomicSort, Field, Operator, Sort};

use super::budgets::derivation_budget as budget;
use super::builders::{self};
use super::common::TestResult;

/// Convert an independent integer coefficient fixture to canonical rationals.
pub(crate) fn rationals(values: &[i64]) -> TestResult<Vec<Rational>> {
    let mut result = Vec::new();
    for value in values {
        result.push(Rational::from_ratio(*value, 1, &mut budget()?)?);
    }
    Ok(result)
}

/// A generic field fixture with named coordinates x and y.
#[derive(Debug)]
pub(crate) struct Fixture {
    /// Store under test.
    pub(crate) cas: Cas,
    /// Immutable context.
    pub(crate) context: Handle,
    /// Scalar sort matching the declared field.
    pub(crate) sort: Sort,
}

impl Fixture {
    /// Declare coordinates without asserting any polynomial identities.
    pub(crate) fn new(field: Field) -> TestResult<Self> {
        let sort = Sort::Value(AtomicSort::Scalar(field.clone()));
        let mut cas = Cas::new(budget()?.limits().clone())?;
        let context = builders::core_context(
            &mut cas,
            builders::environment(field, &["x", "y"]),
            Vec::new(),
            &mut budget()?,
        )?;
        Ok(Self { cas, context, sort })
    }

    /// Construct ordered syntax without invoking a polynomial producer.
    pub(crate) fn term(&mut self, operator: Operator, operands: &[Handle]) -> TestResult<Handle> {
        builders::expression(
            &mut self.cas,
            &self.context,
            operator,
            self.sort.clone(),
            operands,
            &mut budget()?,
        )
    }

    /// Exact independent literal construction.
    pub(crate) fn number(&mut self, text: &str) -> TestResult<Handle> {
        builders::number(
            &mut self.cas,
            &self.context,
            self.sort.clone(),
            text,
            budget,
        )
    }

    /// Spell a coefficient fixture by Horner's identity, independently of the output formatter.
    pub(crate) fn polynomial(&mut self, coefficients: &[i64]) -> TestResult<Handle> {
        let coordinate = self.term(Operator::Free("x".into()), &[])?;
        let mut result = self.number("0")?;
        for coefficient in coefficients.iter().rev() {
            let product = self.term(Operator::Mul, &[result, coordinate.clone()])?;
            let constant = self.term(
                Operator::Rational(Rational::from_ratio(*coefficient, 1, &mut budget()?)?),
                &[],
            )?;
            result = self.term(Operator::Add, &[product, constant])?;
        }
        Ok(result)
    }

    /// Each input is covered completely before requesting multiplicities.
    pub(crate) fn request(&mut self, expression: &Handle) -> TestResult<Request> {
        builders::request(
            &mut self.cas,
            &self.context,
            Task::SquareFree("x".into()),
            std::slice::from_ref(expression),
        )
    }

    /// Only a discharged, complete generated certificate satisfies these polynomial fixtures.
    pub(crate) fn decompose(&mut self, input: &Handle) -> TestResult<SearchResult> {
        let request = self.request(input)?;
        match self.cas.run(&request) {
            Outcome::Proved { result, .. } => Ok(result),
            other => Err(format!("expected square-free proof, got {other:?}").into()),
        }
    }
}

/// Copy a checked theorem's endpoints into a new untrusted proposal for adversarial replay.
pub(crate) fn proposal(
    result: &SearchResult,
    certificate: SquareFreeCertificate,
) -> ApplicationDraft {
    ApplicationDraft {
        rule: Primitive::SquareFree,
        context: result.witness.judgment().context.clone(),
        conclusion: result.witness.judgment().statement.clone(),
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::SquareFree(Box::new(certificate)),
    }
}
