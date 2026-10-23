// SPDX-License-Identifier: AGPL-3.0-only
//! Independent original LC declarations and whole-family exact coefficient formulas.

use super::readings::{Fixture, TestResult, ledger, real};
use nrz_cas::{
    Handle, Rational, Reference,
    domain::{AssumptionDraft, ContextDraft},
    readings::ExactAffineMap,
    syntax::{Draft, ExpressionDraft, Operator, PredicateKind},
};

/// The declared family requires positive inductance and capacitance before reciprocal normalization.
pub(crate) fn family() -> TestResult<Fixture> {
    let mut case = Fixture::new(&[("L", real()), ("C", real())])?;
    let header = case.cas.context_header(&case.context)?;
    let domain = case.cas.admissible_region(&case.context)?;
    let mut assumptions = Vec::new();
    for key in ["L", "C"] {
        let term = case.symbol(key)?;
        let condition = case
            .cas
            .admit(
                &[Draft::Condition {
                    kind: PredicateKind::Positive,
                    header: Reference::Existing(header.clone()),
                    scope: Vec::new(),
                    arguments: vec![Reference::Existing(term)],
                }],
                &mut ledger()?,
            )?
            .pop()
            .ok_or("condition")?;
        assumptions.push(AssumptionDraft {
            key: key.into(),
            condition: Reference::Existing(condition),
        });
    }
    case.context = case
        .cas
        .admit(
            &[Draft::Context(ContextDraft::Root {
                header: Reference::Existing(header),
                admissible: Reference::Existing(domain),
                assumptions,
            })],
            &mut ledger()?,
        )?
        .pop()
        .ok_or("context")?;
    Ok(case)
}

/// Admit an original expression under the retained family header without sampled numerical values.
pub(crate) fn term(
    case: &mut Fixture,
    operator: Operator,
    operands: &[Handle],
) -> TestResult<Handle> {
    let header = case.cas.context_header(&case.context)?;
    Ok(case
        .cas
        .admit(
            &[Draft::Expression(ExpressionDraft {
                operator,
                sort: real(),
                header: Reference::Existing(header),
                scope: Vec::new(),
                binding: None,
                space: None,
                operands: operands.iter().cloned().map(Reference::Existing).collect(),
            })],
            &mut ledger()?,
        )?
        .pop()
        .ok_or("term")?)
}

/// Independently written off-diagonal formula, without inspecting the composed matrix or its proof.
pub(crate) fn operator(case: &mut Fixture, upper: &str, lower: &str) -> TestResult<ExactAffineMap> {
    let zero = term(case, Operator::Rational(Rational::zero()), &[])?;
    let mut coefficients = vec![zero.clone()];
    for (name, text) in [("C", upper), ("L", lower)] {
        let numerator = term(
            case,
            Operator::Rational(Rational::parse(text, &mut ledger()?)?),
            &[],
        )?;
        let denominator = case.symbol(name)?;
        coefficients.push(term(case, Operator::Div, &[numerator, denominator])?);
    }
    coefficients.push(zero.clone());
    Ok(case.cas.exact_reading_map(
        &case.context,
        (2, 2),
        &coefficients,
        &[zero.clone(), zero],
        &mut ledger()?,
    )?)
}
