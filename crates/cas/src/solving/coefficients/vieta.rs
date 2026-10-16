// SPDX-License-Identifier: AGPL-3.0-only
//! Complete existential Vieta systems over complex formal roots, without a computational adjunction.

use super::forms::{Form, Variable};
use super::normalize::Interpretation;
use super::proof::fault;
use super::storage::{AT, invalid, polynomial, vector};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::Handle;
use crate::model::{AtomicSort, Field, Operator, Scope};
use crate::proof_model::{ScopedConditions, Statement};
use crate::solving::{
    CertificateCoefficientIdentity, RootRelationData, ScopedCoefficientRelation, SolveDraft,
    SolveStatement,
};

/// The declared interpretation is a permutation of the entire innermost complex binder.
fn roots(
    interpreter: &Interpretation<'_>,
    evidence: &ScopedCoefficientRelation<Handle>,
    scope: &Scope,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let frame = scope.last().ok_or_else(invalid)?;
    if frame.len() != evidence.roots.len()
        || frame
            .iter()
            .any(|sort| *sort != AtomicSort::Scalar(Field::Complex))
    {
        return Err(invalid());
    }
    let mut seen = vector(frame.len(), budget)?;
    for handle in &evidence.roots {
        let expression = interpreter
            .cas
            .state
            .expression(interpreter.cas.checked(handle)?)?;
        let Operator::Bound { depth: 0, slot } = expression.operator else {
            return Err(invalid());
        };
        budget.work(crate::graph::size(seen.len(), AT)?.max(1), AT)?;
        if expression.scope != *scope
            || usize::try_from(slot).map_err(|_| invalid())? >= frame.len()
            || seen.contains(&slot)
        {
            return Err(invalid());
        }
        seen.push(slot);
    }
    Ok(())
}

/// Root existence supplies exactly the proved degree and defined nonzero leading coefficient.
fn existence(
    interpreter: &Interpretation<'_>,
    context: &Handle,
    anchor: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for premise in interpreter.premises {
        budget.work(1, AT)?;
        if let Statement::Solve(statement) = &premise.judgment.statement
            && let SolveStatement::RootExistence(existence) = statement.as_ref()
            && (&existence.relation, &existence.region) == (anchor, interpreter.region)
            && premise
                .conditions
                .hypotheses
                .iter()
                .all(|hypothesis| hypothesis.context != *context || hypothesis.slot == 0)
        {
            return Ok(());
        }
    }
    Err(invalid())
}

/// Build all elementary symmetric forms by the degree-generic product recurrence.
fn elementary(
    interpreter: &Interpretation<'_>,
    count: usize,
    budget: &mut Budget,
) -> Result<Vec<Form>, Fault> {
    let engine = &interpreter.engine;
    let mut result = vector(count.checked_add(1).ok_or_else(invalid)?, budget)?;
    result.push(
        Form::constant(
            engine
                .constant(true, budget)
                .map_err(|error| fault(&error))?,
            engine,
            budget,
        )
        .map_err(|error| fault(&error))?,
    );
    for _ in 0..count {
        result.push(Form { terms: Vec::new() });
    }
    for index in 0..count {
        let variable = Form::variable(
            Variable::Slot(u32::try_from(index).map_err(|_| invalid())?),
            engine,
            budget,
        )
        .map_err(|error| fault(&error))?;
        for degree in (1..=index + 1).rev() {
            let product = result
                .get(degree - 1)
                .ok_or_else(invalid)?
                .mul(&variable, engine, budget)
                .map_err(|error| fault(&error))?;
            let current = result.get_mut(degree).ok_or_else(invalid)?;
            *current = current
                .add(&product, engine, budget)
                .map_err(|error| fault(&error))?;
        }
    }
    Ok(result)
}

/// Normalize the exact signed coefficient ratio by multiplication with the independently proved unit.
fn coefficient(
    interpreter: &Interpretation<'_>,
    relation: &RootRelationData<Handle>,
    degree: usize,
    right: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let stored = polynomial(&interpreter.cas.state, &relation.polynomial)?;
    let leading = stored.terms.first().ok_or_else(invalid)?;
    let order = *leading.exponents.first().ok_or_else(invalid)?;
    let exponent = order
        .checked_sub(u32::try_from(degree).map_err(|_| invalid())?)
        .ok_or_else(invalid)?;
    let leading_form = interpreter
        .endpoint(&leading.coefficient, budget)
        .map_err(|error| fault(&error))?;
    let right_form = interpreter
        .endpoint(right, budget)
        .map_err(|error| fault(&error))?;
    let product = right_form
        .mul(&leading_form, &interpreter.engine, budget)
        .map_err(|error| fault(&error))?;
    let mut expected = Form { terms: Vec::new() };
    for term in &stored.terms {
        budget.work(1, AT)?;
        if term.exponents.as_slice() == [exponent] {
            expected = interpreter
                .endpoint(&term.coefficient, budget)
                .map_err(|error| fault(&error))?;
        }
    }
    if !degree.is_multiple_of(2) {
        expected = expected
            .neg(&interpreter.engine, budget)
            .map_err(|error| fault(&error))?;
    }
    if !super::scoped::equal(interpreter, &product, &expected, budget)? {
        return Err(invalid());
    }
    Ok(())
}

/// Every local predicate and every elementary degree occurs exactly once in the supplied system.
fn system(
    interpreter: &Interpretation<'_>,
    context: &Handle,
    certificate: &CertificateCoefficientIdentity,
    evidence: &ScopedCoefficientRelation<Handle>,
    relation: &RootRelationData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let frame = interpreter
        .cas
        .state
        .context(interpreter.cas.checked(context)?)?;
    let count = evidence.roots.len();
    if frame.parent.is_none()
        || frame.frame.len() != count
        || certificate.combinations.len() != count
    {
        return Err(invalid());
    }
    let expected = elementary(interpreter, count, budget)?;
    let mut degrees = vector(count, budget)?;
    let mut predicates = vector(count, budget)?;
    for multiple in &certificate.combinations {
        let row = multiple.hypothesis.as_ref().ok_or_else(invalid)?;
        if (&row.vieta, &row.roots) != (&evidence.vieta, &evidence.roots) {
            return Err(invalid());
        }
        let premise = interpreter
            .premises
            .get(usize::try_from(row.premise).map_err(|_| invalid())?)
            .ok_or_else(invalid)?;
        let Statement::PredicateFact { predicate, .. } = &premise.judgment.statement else {
            return Err(invalid());
        };
        budget.work(crate::graph::size(count, AT)?, AT)?;
        if predicates.contains(predicate)
            || !frame
                .frame
                .iter()
                .any(|(_, id)| *id == predicate.logical_id())
        {
            return Err(invalid());
        }
        predicates.push(predicate.clone());
        let degree = elementary_degree(interpreter, &row.left, &expected, budget)?;
        if degrees.contains(&degree) {
            return Err(invalid());
        }
        degrees.push(degree);
        coefficient(interpreter, relation, degree, &row.right, budget)?;
    }
    Ok(())
}

/// Identify each elementary degree by independent canonical polynomial comparison.
fn elementary_degree(
    interpreter: &Interpretation<'_>,
    handle: &Handle,
    expected: &[Form],
    budget: &mut Budget,
) -> Result<usize, Fault> {
    let left = interpreter
        .endpoint(handle, budget)
        .map_err(|error| fault(&error))?;
    for (degree, candidate) in expected.iter().enumerate().skip(1) {
        if super::scoped::equal(interpreter, &left, candidate, budget)? {
            return Ok(degree);
        }
    }
    Err(invalid())
}

/// A conclusion may retain its declared frame but no occurrence of its formal root variables.
fn conclusion(
    interpreter: &Interpretation<'_>,
    endpoint: &Handle,
    scope: &Scope,
    budget: &mut Budget,
) -> Result<(), Fault> {
    super::independent::object(interpreter.cas, endpoint, scope, budget)?;
    let form = interpreter
        .endpoint(endpoint, budget)
        .map_err(|error| fault(&error))?;
    for term in &form.terms {
        budget.work(crate::graph::size(term.powers.len(), AT)?, AT)?;
        if term
            .powers
            .iter()
            .any(|(variable, _)| matches!(variable, Variable::Slot(_)))
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Discharge only an explicitly anchored complete satisfiable system; retain all other conditions.
pub(crate) fn discharge(
    interpreter: &Interpretation<'_>,
    context: &Handle,
    certificate: &CertificateCoefficientIdentity,
    inherited: &mut ScopedConditions,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Some(evidence) = certificate
        .combinations
        .iter()
        .filter_map(|row| row.hypothesis.as_ref())
        .find(|row| row.vieta.is_some())
    else {
        return Ok(());
    };
    let anchor = evidence.vieta.as_ref().ok_or_else(invalid)?;
    let SolveDraft::RootRelation(relation) =
        crate::solving::graph::object(&interpreter.cas.state, interpreter.cas.checked(anchor)?)?
    else {
        return Err(invalid());
    };
    if (&relation.map, &relation.region, &relation.ambient)
        != (interpreter.map, interpreter.region, &Field::Complex)
    {
        return Err(invalid());
    }
    let stored = polynomial(&interpreter.cas.state, &relation.polynomial)?;
    let leading = stored.terms.first().ok_or_else(invalid)?;
    if stored.variables.len() != 1
        || leading.exponents.as_slice()
            != [u32::try_from(evidence.roots.len()).map_err(|_| invalid())?]
        || evidence.roots.is_empty()
    {
        return Err(invalid());
    }
    roots(interpreter, evidence, &relation.scope, budget)?;
    existence(interpreter, context, anchor, budget)?;
    for handle in [&certificate.left, &certificate.right] {
        conclusion(interpreter, handle, &relation.scope, budget)?;
    }
    for handle in [interpreter.map, interpreter.region] {
        super::independent::object(interpreter.cas, handle, &relation.scope, budget)?;
    }
    super::independent::surroundings(interpreter.cas, context, &relation.scope, budget)?;
    super::independent::conditions(interpreter.cas, inherited, context, &relation.scope, budget)?;
    system(
        interpreter,
        context,
        certificate,
        evidence,
        relation,
        budget,
    )?;
    inherited
        .hypotheses
        .retain(|reference| reference.context != *context || reference.slot == 0);
    Ok(())
}
