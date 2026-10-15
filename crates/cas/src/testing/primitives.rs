// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit step and constant distributions for uniqueness tests.

use super::budgets::derivation_budget as budget;
use super::common::TestResult;
use super::distributions::Fixture;
use crate::test_api::domain::Layer;
use crate::test_api::proof::{DistributionStatement, Judgment, Statement};
use crate::test_api::syntax::{CaseName, NamedCase, Operator};
use crate::test_api::{Handle, Rational};

/// Hand-written `T_step` and `T_step + T_7`, with no differentiation in the fixture.
pub(crate) fn pair(fixture: &mut Fixture) -> TestResult<(Handle, Handle)> {
    let step = fixture.base.cas.named_case(
        &fixture.base.domain,
        &NamedCase {
            name: CaseName::Step,
            coordinate: "x".into(),
            parameter: Some(Rational::zero()),
        },
        &mut budget()?,
    )?;
    let left = fixture.node(Operator::RegularEmbed, &[step])?;
    let seven = fixture.base.number("7")?;
    let constant = fixture.node(Operator::RegularEmbed, &[seven])?;
    let right = fixture.node(Operator::DistributionAdd, &[left.clone(), constant])?;
    Ok((left, right))
}

/// Expected theorem asserts existence of a constant difference, independently of the producer.
pub(crate) fn expected(fixture: &Fixture, left: Handle, right: Handle) -> Judgment {
    Judgment {
        context: fixture.base.context.clone(),
        layer: Layer::Distribution,
        space: Some(fixture.space.clone()),
        residuals: Vec::new(),
        statement: Statement::Distribution(Box::new(DistributionStatement::PrimitiveUnique {
            left,
            right,
            space: fixture.space.clone(),
        })),
    }
}
