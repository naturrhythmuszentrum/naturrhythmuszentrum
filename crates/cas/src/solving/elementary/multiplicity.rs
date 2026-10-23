// SPDX-License-Identifier: AGPL-3.0-only
//! Characteristic-zero derivative multiplicities on exact existing selections.

use super::constraints::Constraints;
use super::evaluate::Evaluation;
use super::proof::{AT, invalid, premise, problem};
use super::value::Value;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::proof_model::{ApplicationDraft, CheckedApplication, Statement};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::forms::{Form, Variable};
use crate::solving::coefficients::proof::fault;
use crate::solving::{CertificateMultiplicityTransfer, MultiplicityMode, SolveStatement};

/// Formal differentiation is independently reconstructed from the original sparse equation.
fn derivative(
    evaluation: &Evaluation<'_>,
    input: &Value,
    variable: Variable,
    budget: &mut Budget,
) -> Result<Value, Fault> {
    let mut result = Value::integer(0, &evaluation.engine, budget)?;
    for term in &input.numerator.terms {
        let Some((_, exponent)) = term.powers.iter().find(|(key, _)| *key == variable) else {
            continue;
        };
        let coefficient = evaluation
            .engine
            .copy(&term.coefficient, budget)
            .map_err(|e| fault(&e))?;
        let mut product = Value::polynomial(
            Form::constant(coefficient, &evaluation.engine, budget).map_err(|e| fault(&e))?,
            &evaluation.engine,
            budget,
        )?;
        product = product.mul(
            &Value::integer(i64::from(*exponent), &evaluation.engine, budget)?,
            &evaluation.engine,
            budget,
        )?;
        for (key, power) in &term.powers {
            let degree = if *key == variable {
                power.checked_sub(1).ok_or_else(invalid)?
            } else {
                *power
            };
            product = product.mul(
                &Value::variable(*key, &evaluation.engine, budget)?.pow(
                    degree,
                    &evaluation.engine,
                    budget,
                )?,
                &evaluation.engine,
                budget,
            )?;
        }
        result = result.add(&product, &evaluation.engine, budget)?;
    }
    Ok(result)
}

/// One root relation proves the zero derivatives; the first nonzero value is checked independently.
fn at_root(
    evaluation: &Evaluation<'_>,
    relation: &Handle,
    multiplicity: u32,
    original: &Value,
    unknown: &Value,
    values: &[Handle],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let [term] = unknown.numerator.terms.as_slice() else {
        return Err(invalid());
    };
    let [(variable, 1)] = term.powers.as_slice() else {
        return Err(invalid());
    };
    let mut source = Constraints::new();
    source.zeros.push(
        evaluation
            .root_equation(
                unknown,
                super::selected::relation(evaluation.cas, relation)?,
                budget,
            )?
            .numerator,
    );
    let mut current = original.copy(&evaluation.engine, budget)?;
    for (order, handle) in values.iter().enumerate() {
        let supplied = evaluation.expression(handle.logical_id(), true, budget)?;
        source.zero(
            evaluation,
            &current.sub(&supplied, &evaluation.engine, budget)?,
            budget,
        )?;
        if u32::try_from(order).map_err(|_| invalid())? < multiplicity {
            evaluation.zero(&supplied, budget)?;
        } else {
            evaluation.nonzero(&supplied, budget)?;
        }
        current = derivative(evaluation, &current, *variable, budget)?;
    }
    Ok(())
}

/// A product theorem and exact selection existence never supply multiplicity without derivatives.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateMultiplicityTransfer,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    if certificate.mode != MultiplicityMode::Derivatives {
        return Err(invalid());
    }
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    let SolveStatement::Multiplicity(claim) = statement.as_ref() else {
        return Err(invalid());
    };
    if (
        &claim.problem,
        &claim.selection,
        &claim.multiplicities,
        &claim.region,
    ) != (
        &certificate.problem,
        &certificate.selection,
        &certificate.multiplicities,
        &certificate.region,
    ) {
        return Err(invalid());
    }
    let original = problem(cas, &certificate.problem)?;
    super::proof::region(cas, &draft.context, original, &certificate.region)?;
    let Statement::Solve(product) = &premise(premises, certificate.product)?.judgment.statement
    else {
        return Err(invalid());
    };
    if !matches!(product.as_ref(), SolveStatement::Coverage(value) if (&value.problem, &value.region) == (&certificate.problem, &certificate.region))
    {
        return Err(invalid());
    }
    let selected = super::selected::selection(cas, &certificate.selection)?;
    if selected.relations.len() != certificate.multiplicities.len() {
        return Err(invalid());
    }
    for slot in &certificate.nonzero {
        premise(premises, *slot)?;
    }
    let evaluation = Evaluation {
        exclusions: &[],
        cas,
        engine: Engine {
            state: &cas.state,
            domain: &original.coefficient_domain,
        },
        region: &certificate.region,
        premises,
        free: &[],
        bound: &[],
    };
    evaluation.existence(certificate.selection.logical_id(), budget)?;
    let input = super::transport::unknowns(&evaluation, original, budget)?;
    let [unknown] = input.as_slice() else {
        return Err(invalid());
    };
    let equation = super::product::equation(&evaluation, original, budget)?;
    let one = Value::integer(1, &evaluation.engine, budget)?;
    if !Value::polynomial(
        equation
            .denominator
            .copy(&evaluation.engine, budget)
            .map_err(|e| fault(&e))?,
        &evaluation.engine,
        budget,
    )?
    .equals(&one, &evaluation.engine, budget)?
    {
        return Err(invalid());
    }
    derivatives(
        &evaluation,
        certificate,
        selected,
        &equation,
        unknown,
        budget,
    )
}

/// The ordered vector carries every derivative through the first nonzero order for every slot.
fn derivatives(
    evaluation: &Evaluation<'_>,
    certificate: &CertificateMultiplicityTransfer,
    selected: &crate::solving::RootSelectionData<Handle>,
    equation: &Value,
    unknown: &Value,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut cursor = 0_usize;
    for (relation, multiplicity) in selected.relations.iter().zip(&certificate.multiplicities) {
        if *multiplicity == 0 {
            return Err(invalid());
        }
        budget.check(BudgetKind::PolynomialDegree, u64::from(*multiplicity), AT)?;
        let count = usize::try_from(*multiplicity)
            .map_err(|_| invalid())?
            .checked_add(1)
            .ok_or_else(invalid)?;
        let end = cursor.checked_add(count).ok_or_else(invalid)?;
        at_root(
            evaluation,
            relation,
            *multiplicity,
            equation,
            unknown,
            certificate.values.get(cursor..end).ok_or_else(invalid)?,
            budget,
        )?;
        cursor = end;
    }
    if cursor != certificate.values.len() {
        return Err(invalid());
    }
    Ok(())
}
