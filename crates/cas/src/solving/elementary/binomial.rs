// SPDX-License-Identifier: AGPL-3.0-only
//! Degree-generic roots of unity and complete binomial products.

use super::evaluate::Evaluation;
use super::proof::{AT, invalid, problem};
use super::value::Value;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, ExpressionAttachment, Handle};
use crate::model::Operator;
use crate::proof_model::{ApplicationDraft, CheckedApplication, Statement};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::model::EvaluationScope;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{polynomial, vector};
use crate::solving::towers::dense::Dense;
use crate::solving::{
    BinomialMode, CertificateBinomialCoverage, PrimePower, SolveStatement, StatementCoverage,
};

/// Prime powers are increasing, positive and reconstruct the full exact order.
fn primes(order: u32, factors: &[PrimePower], budget: &mut Budget) -> Result<(), Fault> {
    if order == 0 {
        return Err(invalid());
    }
    budget.check(BudgetKind::PolynomialDegree, u64::from(order), AT)?;
    let mut product = 1_u32;
    let mut previous = 1;
    for factor in factors {
        if factor.prime <= previous || factor.exponent == 0 {
            return Err(invalid());
        }
        let mut divisor = 2_u32;
        while divisor <= factor.prime / divisor {
            budget.work(1, AT)?;
            if factor.prime.is_multiple_of(divisor) {
                return Err(invalid());
            }
            divisor = divisor.checked_add(1).ok_or_else(invalid)?;
        }
        for _ in 0..factor.exponent {
            budget.work(1, AT)?;
            product = product.checked_mul(factor.prime).ok_or_else(invalid)?;
        }
        previous = factor.prime;
    }
    if product != order {
        return Err(invalid());
    }
    Ok(())
}

/// Construct Xⁿ-1 without any degree-specific constants.
fn unity_polynomial(engine: &Engine<'_>, order: u32, budget: &mut Budget) -> Result<Dense, Fault> {
    let length = usize::try_from(order)
        .map_err(|_| invalid())?
        .checked_add(1)
        .ok_or_else(invalid)?;
    let mut coefficients = vector(length, budget)?;
    for index in 0..length {
        let coefficient = engine
            .constant(index == 0 || index == length - 1, budget)
            .map_err(|e| fault(&e))?;
        coefficients.push(if index == 0 {
            engine.neg(&coefficient, budget).map_err(|e| fault(&e))?
        } else {
            coefficient
        });
    }
    Dense::new(engine, coefficients, budget).map_err(|e| fault(&e))
}

/// Exact divisibility and Bezout coprimality exclude every prime-divisor order.
fn order_identities(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateBinomialCoverage,
    relation: &crate::solving::RootRelationData<Handle>,
    region: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let stored = polynomial(&cas.state, &relation.polynomial)?;
    let engine = Engine {
        state: &cas.state,
        domain: &stored.domain,
    };
    let defining = Dense::read(&cas.state, &relation.polynomial, budget).map_err(|e| fault(&e))?;
    let scope = EvaluationScope {
        context: &draft.context,
        region,
        premises: &[],
        localization: &[],
    };
    let total = unity_polynomial(&engine, certificate.order, budget)?;
    let (_, remainder) =
        crate::solving::towers::euclid::divide(&total, &defining, &cas.state, &scope, budget)
            .map_err(|e| fault(&e))?
            .map_err(|_| invalid())?;
    if !remainder.coefficients.is_empty() {
        return Err(invalid());
    }
    for prime in &certificate.factors {
        let excluded = unity_polynomial(&engine, certificate.order / prime.prime, budget)?;
        let bezout =
            crate::solving::towers::euclid::gcd(&defining, &excluded, &cas.state, &scope, budget)
                .map_err(|e| fault(&e))?
                .map_err(|_| invalid())?;
        let one = Dense::one(&engine, budget).map_err(|e| fault(&e))?;
        let reconstructed = defining
            .mul(&bezout.first, &cas.state, budget)
            .map_err(|e| fault(&e))?
            .sum(
                &excluded
                    .mul(&bezout.second, &cas.state, budget)
                    .map_err(|e| fault(&e))?,
                false,
                &cas.state,
                budget,
            )
            .map_err(|e| fault(&e))?;
        if !bezout
            .gcd
            .equals(&one, &cas.state, budget)
            .map_err(|e| fault(&e))?
            || !reconstructed
                .equals(&one, &cas.state, budget)
                .map_err(|e| fault(&e))?
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/// The exact root atom and its existing selection are the subject of the order theorem.
fn order(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateBinomialCoverage,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    let SolveStatement::RootOrder(order) = statement.as_ref() else {
        return Err(invalid());
    };
    if certificate.order != order.order
        || certificate.selection.is_some()
        || certificate.product.is_some()
        || !certificate.exclusions.is_empty()
    {
        return Err(invalid());
    }
    let unity = certificate.unity.as_ref().ok_or_else(invalid)?;
    let expression = cas.state.expression(cas.checked(&order.value)?)?;
    if !matches!((&expression.operator, &expression.attachment), (Operator::RootValue { slot: 0 }, Some(ExpressionAttachment::SelectedRoot(id))) if *id == unity.logical_id())
    {
        return Err(invalid());
    }
    let selected = super::selected::selection(cas, unity)?;
    let [handle] = selected.relations.as_slice() else {
        return Err(invalid());
    };
    let relation = super::selected::relation(cas, handle)?;
    if relation.region != order.region {
        return Err(invalid());
    }
    super::total::check(
        cas,
        order.value.logical_id(),
        &order.region,
        premises,
        budget,
    )?;
    order_identities(cas, draft, certificate, relation, &order.region, budget)
}

/// Order evidence is taken from an earlier theorem about this exact unity selection.
fn unity(
    evaluation: &Evaluation<'_>,
    selected: &Handle,
    order: u32,
    budget: &mut Budget,
) -> Result<Value, Fault> {
    for premise in evaluation.premises {
        budget.work(1, AT)?;
        if let Statement::Solve(statement) = &premise.judgment.statement
            && let SolveStatement::RootOrder(value) = statement.as_ref()
            && (&value.region, value.order) == (evaluation.region, order)
        {
            let expression = evaluation.cas.state.expression(value.value.logical_id())?;
            if matches!(&expression.attachment, Some(ExpressionAttachment::SelectedRoot(id)) if *id == selected.logical_id())
            {
                return evaluation.expression(value.value.logical_id(), true, budget);
            }
        }
    }
    Err(invalid())
}

/// Every listed tuple is one of the independently constructed cyclic products, and vice versa.
fn cyclic(
    evaluation: &Evaluation<'_>,
    certificate: &CertificateBinomialCoverage,
    entries: &[Handle],
    unknown: &Value,
    budget: &mut Budget,
) -> Result<Value, Fault> {
    let selected = certificate.selection.as_ref().ok_or_else(invalid)?;
    let atom = super::selected::occurrence(evaluation.cas, entries, selected, budget)?;
    let root = evaluation.expression(atom, true, budget)?;
    let unity = unity(
        evaluation,
        certificate.unity.as_ref().ok_or_else(invalid)?,
        certificate.order,
        budget,
    )?;
    let mut product = Value::integer(1, &evaluation.engine, budget)?;
    for power in 0..certificate.order {
        let value =
            unity
                .pow(power, &evaluation.engine, budget)?
                .mul(&root, &evaluation.engine, budget)?;
        let mut present = false;
        for entry in entries {
            let actual = evaluation.expression(entry.logical_id(), true, budget)?;
            if super::product::decides(
                evaluation.zero(&actual.sub(&value, &evaluation.engine, budget)?, budget),
            )? {
                present = true;
            }
        }
        if !present {
            return Err(invalid());
        }
        product = product.mul(
            &unknown.sub(&value, &evaluation.engine, budget)?,
            &evaluation.engine,
            budget,
        )?;
    }
    Ok(product)
}

/// The original equation is exactly the monic binomial relation of the selected root.
fn coverage(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateBinomialCoverage,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    let SolveStatement::Coverage(coverage) = statement.as_ref() else {
        return Err(invalid());
    };
    let original = problem(cas, &coverage.problem)?;
    super::proof::region(cas, &draft.context, original, &coverage.region)?;
    super::member::contained(cas, &coverage.region, &original.region, budget)?;
    let evaluation = Evaluation {
        exclusions: &[],
        cas,
        engine: Engine {
            state: &cas.state,
            domain: &original.coefficient_domain,
        },
        region: &coverage.region,
        premises,
        free: &[],
        bound: &[],
    };
    let selected =
        super::selected::selection(cas, certificate.selection.as_ref().ok_or_else(invalid)?)?;
    let [handle] = selected.relations.as_slice() else {
        return Err(invalid());
    };
    let relation = super::selected::relation(cas, handle)?;
    let stored = polynomial(&cas.state, &relation.polynomial)?;
    if stored
        .terms
        .first()
        .is_none_or(|term| term.exponents != [certificate.order])
        || stored
            .terms
            .iter()
            .any(|term| term.exponents != [0] && term.exponents != [certificate.order])
    {
        return Err(invalid());
    }
    let input = super::transport::unknowns(&evaluation, original, budget)?;
    let [unknown] = input.as_slice() else {
        return Err(invalid());
    };
    let equation = super::product::equation(&evaluation, original, budget)?;
    if !equation.equals(
        &evaluation.root_equation(unknown, relation, budget)?,
        &evaluation.engine,
        budget,
    )? {
        return Err(invalid());
    }
    let entries = super::product::entries(cas, coverage, budget)?;
    super::product::membership(&evaluation, original, &entries, budget)?;
    let product = if certificate.mode == BinomialMode::ZeroRadicand {
        zero(
            &evaluation,
            certificate,
            coverage,
            unknown,
            &entries,
            budget,
        )?
    } else {
        cyclic(&evaluation, certificate, &entries, unknown, budget)?
    };
    super::product::identity(
        &evaluation,
        certificate.product.ok_or_else(invalid)?,
        &equation,
        &product,
        budget,
    )
}

/// Zero has one set member and its full polynomial factor power, without a unity choice.
fn zero(
    evaluation: &Evaluation<'_>,
    certificate: &CertificateBinomialCoverage,
    coverage: &StatementCoverage,
    unknown: &Value,
    entries: &[Handle],
    budget: &mut Budget,
) -> Result<Value, Fault> {
    if certificate.unity.is_some()
        || !certificate.factors.is_empty()
        || !certificate.exclusions.is_empty()
        || entries.len() != 1
    {
        return Err(invalid());
    }
    let value = evaluation.expression(
        entries.first().ok_or_else(invalid)?.logical_id(),
        true,
        budget,
    )?;
    evaluation.zero(&value, budget)?;
    let original = problem(evaluation.cas, &coverage.problem)?;
    let expected = unknown.pow(certificate.order, &evaluation.engine, budget)?;
    if !super::product::equation(evaluation, original, budget)?.equals(
        &expected,
        &evaluation.engine,
        budget,
    )? {
        return Err(invalid());
    }
    Ok(expected)
}

/// Each closed mode has its exact required and forbidden optional fields.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateBinomialCoverage,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    if certificate.order == 0 || !certificate.exclusions.is_empty() {
        return Err(invalid());
    }
    budget.check(
        BudgetKind::PolynomialDegree,
        u64::from(certificate.order),
        AT,
    )?;
    if certificate.mode != BinomialMode::ZeroRadicand {
        primes(certificate.order, &certificate.factors, budget)?;
    }
    match certificate.mode {
        BinomialMode::Order => order(cas, draft, certificate, premises, budget),
        BinomialMode::Coverage | BinomialMode::ZeroRadicand => {
            coverage(cas, draft, certificate, premises, budget)
        }
    }
}
