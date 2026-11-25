// SPDX-License-Identifier: AGPL-3.0-only
//! Polynomial-ring interpretation, independent of strict scalar definedness.

use super::check::{AT, coefficient, compatible, equal, invalid, number, unsupported};
use super::{ResidualSubject, SeriesCertificate, SeriesObject, SeriesStatement};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, Handle, LogicalId, Vertex, size};
use crate::model::{AtomicSort, Field, Operator, PredicateKind, Sort};
use crate::number::Rational;
use crate::proof_model::{ApplicationDraft, CheckedApplication, ScopedConditions, Statement};
use crate::solving::{CoordinateRole, SolveDraft};

/// Original interpretation frame borrowed throughout preflight and arithmetic.
struct Frame<'a> {
    /// Owning store.
    cas: &'a Cas,
    /// Exact admitted candidate.
    input: &'a SeriesObject<Handle>,
    /// Original placeholder declaration.
    dependent: &'a Handle,
    /// Original differentiation declaration key.
    key: &'a str,
    /// Shared closed declaration header.
    header: LogicalId,
}

impl Frame<'_> {
    /// All syntax, including shorter derivative nodes, shares one closed Real header.
    fn expression(
        &self,
        id: LogicalId,
        budget: &mut crate::budget::Budget,
    ) -> Result<&crate::graph::Expression, Fault> {
        let value = self.cas.state.expression(id, budget)?;
        if value.header != self.header
            || !value.scope.is_empty()
            || value.space.is_some()
            || value.sort != Sort::Value(AtomicSort::Scalar(Field::Real))
        {
            return Err(unsupported());
        }
        Ok(value)
    }

    /// Derivatives mean jet coordinates only when the entire chain ends at y.
    fn derivative(&self, mut id: LogicalId, budget: &mut Budget) -> Result<u32, Fault> {
        let mut order = 0_u32;
        while id != self.dependent.logical_id() {
            budget.work(1, AT)?;
            let value = self.expression(id, budget)?;
            let Operator::Derivative(key) = &value.operator else {
                return Err(unsupported());
            };
            if key != self.key {
                return Err(unsupported());
            }
            let [child] = value.operands.as_slice() else {
                return Err(unsupported());
            };
            order = order.checked_add(1).ok_or_else(unsupported)?;
            id = *child;
        }
        Ok(order)
    }

    /// Count every occurrence and power round before allocating the trace workspace.
    fn schedule(
        &self,
        id: LogicalId,
        depth: u64,
        budget: &mut Budget,
    ) -> Result<(u32, u64), Fault> {
        budget.work(1, AT)?;
        budget.check(BudgetKind::Depth, depth, AT)?;
        let value = self.expression(id, budget)?;
        let mut derivative = 0;
        let mut rows = 1_u64;
        match &value.operator {
            Operator::Rational(value) => value.check(budget, AT)?,
            Operator::Free(_)
                if id == self.input.variable.logical_id() || id == self.dependent.logical_id() => {}
            Operator::Neg | Operator::Add | Operator::Sub | Operator::Mul => {}
            Operator::IntegerPower(q) if *q >= 0 => {
                let q = u64::try_from(*q).map_err(|_| unsupported())?;
                budget.check(BudgetKind::IntegerExponent, q, AT)?;
                rows = rows.checked_add(q).ok_or_else(invalid)?;
            }
            Operator::Derivative(_) => derivative = self.derivative(id, budget)?,
            _ => return Err(unsupported()),
        }
        for child in &value.operands {
            let (r, count) =
                self.schedule(*child, depth.checked_add(1).ok_or_else(invalid)?, budget)?;
            derivative = derivative.max(r);
            rows = rows.checked_add(count).ok_or_else(invalid)?;
        }
        Ok((derivative, rows))
    }
}

/// Validate the exact frame before proof payload ownership or power schedule allocation.
fn frame<'a>(
    cas: &'a Cas,
    subject: &'a ResidualSubject,
    dependent: &'a Handle,
    budget: &mut Budget,
) -> Result<Frame<'a>, Fault> {
    let input = cas.series_object(subject.output(), budget)?;
    input.preflight(budget)?;
    let context = cas
        .state
        .context(cas.checked(&input.context, budget)?, budget)?;
    if context.environment.field != Field::Real {
        return Err(unsupported());
    }
    let header = context.header_id;
    let variable = cas
        .state
        .expression(cas.checked(&input.variable, budget)?, budget)?;
    let Operator::Free(key) = &variable.operator else {
        return Err(unsupported());
    };
    let region = cas.state.region(context.admissible, budget)?;
    if !input.scope.is_empty() || variable.space.is_some() {
        return Err(unsupported());
    }
    let result = Frame {
        cas,
        input,
        dependent,
        key,
        header,
    };
    let y = result.expression(cas.checked(dependent, budget)?, budget)?;
    if dependent == &input.variable || !matches!(y.operator, Operator::Free(_)) {
        return Err(unsupported());
    }
    if let ResidualSubject::Source(source) = subject {
        if source.domain.logical_id() != context.admissible {
            return Err(invalid());
        }
        super::independent(
            &cas.state,
            cas.checked(&source.source, budget)?,
            cas.checked(dependent, budget)?,
            budget,
        )?;
    }
    let crate::domain_model::AmbientDomain::Product { coordinates } = &region.ambient else {
        return Err(unsupported());
    };
    let SolveDraft::CoordinateSpace(product) =
        crate::solving::graph::object(&cas.state, coordinates.logical_id(), budget)?
    else {
        return Err(unsupported());
    };
    let [coordinate] = product.coordinates.as_slice() else {
        return Err(unsupported());
    };
    if product.header.logical_id() != header
        || coordinate.key != *key
        || coordinate.field != Field::Real
        || coordinate.role != CoordinateRole::Coefficient
    {
        return Err(unsupported());
    }
    Ok(result)
}

/// Preflight is shared by public and replay entry routes, including dead power syntax.
pub(super) fn preflight(
    cas: &Cas,
    subject: &ResidualSubject,
    expression: &Handle,
    dependent: &Handle,
    budget: &mut Budget,
) -> Result<(u32, u32, u64), Fault> {
    let frame = frame(cas, subject, dependent, budget)?;
    let (r, rows) = frame.schedule(cas.checked(expression, budget)?, 1, budget)?;
    let m = frame
        .input
        .order
        .checked_sub(r)
        .filter(|m| *m > 0)
        .ok_or_else(unsupported)?;
    let entries = rows.checked_mul(u64::from(m)).ok_or_else(invalid)?;
    budget.check(BudgetKind::JetEntries, entries, AT)?;
    Ok((r, m, rows))
}

/// Reject malformed problem roles even on independently supplied expected judgments.
pub(super) fn roles(cas: &Cas, value: &SeriesStatement, budget: &mut Budget) -> Result<(), Fault> {
    let SeriesStatement::EvaluatedResidual {
        subject,
        expression,
        dependent,
        output,
        problem,
        derivative_order,
        order,
    } = value
    else {
        return Err(invalid());
    };
    let (r, m, _) = preflight(cas, subject, expression, dependent, budget)?;
    if (r, m) != (*derivative_order, *order) {
        return Err(invalid());
    }
    let input = cas.series_object(subject.output(), budget)?;
    let output = cas.series_object(output, budget)?;
    compatible(input, output, budget)?;
    if output.order != m {
        return Err(invalid());
    }
    problem_check(cas, input, output, problem, budget)
}

/// Exact syntax, complete row order and original product metadata determine the problem.
fn problem_check(
    cas: &Cas,
    input: &SeriesObject<Handle>,
    output: &SeriesObject<Handle>,
    problem: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let SolveDraft::SolveProblem(problem) =
        crate::solving::graph::object(&cas.state, cas.checked(problem, budget)?, budget)?
    else {
        return Err(invalid());
    };
    let context = cas.state.context(input.context.logical_id(), budget)?;
    let region = cas.state.region(context.admissible, budget)?;
    if problem.header.logical_id() != context.header_id
        || !problem.scope.is_empty()
        || problem.region.logical_id() != context.admissible
        || region.ambient
            != (crate::domain_model::AmbientDomain::Product {
                coordinates: problem.coefficients.clone(),
            })
        || problem.coefficient_domain != input.field
        || problem.ambient != Field::Real
        || !problem.denominators.is_empty()
    {
        return Err(invalid());
    }
    let SolveDraft::CoordinateSpace(unknowns) =
        crate::solving::graph::object(&cas.state, problem.unknowns.logical_id(), budget)?
    else {
        return Err(invalid());
    };
    if unknowns.header != problem.header || !unknowns.coordinates.is_empty() {
        return Err(invalid());
    }
    let domain = cas.state.region(problem.domain.logical_id(), budget)?;
    if domain.header != context.header_id
        || !domain.scope.is_empty()
        || domain.ambient
            != (crate::domain_model::AmbientDomain::Product {
                coordinates: problem.unknowns.clone(),
            })
        || !matches!(domain.expression, crate::domain_model::RegionExpr::Ambient)
    {
        return Err(invalid());
    }
    let Vertex::Condition(predicate) = cas.state.node(problem.predicate.logical_id(), budget)?
    else {
        return Err(invalid());
    };
    if predicate.header != context.header_id || !predicate.scope.is_empty() {
        return Err(invalid());
    }
    if output.order == 1 {
        if predicate.kind != PredicateKind::Equal {
            return Err(invalid());
        }
        equation(cas, &predicate.arguments, &output.coefficients[0], budget)?;
    } else {
        if predicate.kind != PredicateKind::And
            || predicate.arguments.len() != output.coefficients.len()
        {
            return Err(invalid());
        }
        for (id, coefficient) in predicate.arguments.iter().zip(&output.coefficients) {
            budget.work(1, AT)?;
            let expression = cas.state.expression(*id, budget)?;
            if expression.operator != Operator::Predicate(PredicateKind::Equal)
                || expression.header != context.header_id
                || !expression.scope.is_empty()
                || expression.space.is_some()
                || expression.binding.is_some()
                || expression.sort != Sort::Value(AtomicSort::Boolean)
                || expression.attachment.is_some()
                || expression.ambient.is_some()
                || expression.domain.is_some()
                || !expression.arms.is_empty()
                || expression.layer != crate::domain_model::Layer::Logic
            {
                return Err(invalid());
            }
            equation(cas, &expression.operands, coefficient, budget)?;
        }
    }
    Ok(())
}

/// Canonical literal equality retains zeros and duplicates rather than simplifying predicates.
fn equation(
    cas: &Cas,
    arguments: &[LogicalId],
    coefficient: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let [left, right] = arguments else {
        return Err(invalid());
    };
    if *left != coefficient.logical_id() {
        return Err(invalid());
    }
    let zero = cas.state.expression(*right, budget)?;
    let expected = cas.state.expression(coefficient.logical_id(), budget)?;
    if zero.header != expected.header
        || zero.scope != expected.scope
        || zero.space != expected.space
        || zero.binding != expected.binding
        || zero.sort != expected.sort
        || zero.ambient != expected.ambient
        || zero.attachment.is_some()
        || zero.layer != expected.layer
        || zero.domain != expected.domain
        || zero.arms != expected.arms
        || !zero.operands.is_empty()
    {
        return Err(invalid());
    }
    let Operator::Rational(value) = &zero.operator else {
        return Err(invalid());
    };
    equal(value, &Rational::zero(), budget)
}

/// A dense vector has reserved ownership and explicit charges for zero entries too.
fn vector(order: usize, budget: &mut Budget) -> Result<Vec<Rational>, Fault> {
    budget.specialized(BudgetKind::JetEntries, size(order, AT)?, true, AT)?;
    budget.work(size(order, AT)?, AT)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(order)
        .map_err(|_| Fault::allocation(AT))?;
    values.resize_with(order, Rational::zero);
    Ok(values)
}

/// Reconstruct every convolution pair, including pairs with zero operands.
fn multiply(
    left: &[Rational],
    right: &[Rational],
    budget: &mut Budget,
) -> Result<Vec<Rational>, Fault> {
    let mut result = vector(left.len(), budget)?;
    for (degree, value) in result.iter_mut().enumerate() {
        for (index, a) in left.iter().take(degree + 1).enumerate() {
            budget.work(1, AT)?;
            *value = value.add(&a.mul(&right[degree - index], budget)?, budget)?;
        }
    }
    Ok(result)
}

/// The checker consumes supplied rows only after independent arithmetic reconstruction.
struct Trace<'a> {
    /// Checked original interpretation.
    frame: Frame<'a>,
    /// Untrusted occurrence vectors.
    rows: &'a [super::ResidualRow],
    /// Next required ordinal.
    next: usize,
    /// Uniform positive coefficient count.
    order: usize,
}

impl Trace<'_> {
    /// Check complete shape and every exact rational before using the row downstream.
    fn compare(&mut self, value: &[Rational], budget: &mut Budget) -> Result<(), Fault> {
        let row = self.rows.get(self.next).ok_or_else(invalid)?;
        if u64::from(row.ordinal) != size(self.next, AT)? || row.coefficients.len() != self.order {
            return Err(invalid());
        }
        for (actual, claimed) in value.iter().zip(&row.coefficients) {
            budget.specialized(BudgetKind::JetEntries, 1, true, AT)?;
            equal(actual, claimed, budget)?;
        }
        self.next = self.next.checked_add(1).ok_or_else(invalid)?;
        Ok(())
    }

    /// Evaluate occurrences without DAG deduplication and preserve every power round.
    fn visit(&mut self, id: LogicalId, budget: &mut Budget) -> Result<Vec<Rational>, Fault> {
        budget.work(1, AT)?;
        let cas = self.frame.cas;
        let expression = cas.state.expression(id, budget)?;
        let operator = &expression.operator;
        let mut children = Vec::new();
        children
            .try_reserve_exact(expression.operands.len())
            .map_err(|_| Fault::allocation(AT))?;
        for child in &expression.operands {
            children.push(self.visit(*child, budget)?);
        }
        let mut value = vector(self.order, budget)?;
        match operator {
            Operator::Rational(number) => value[0] = super::produce::copied(number, budget)?,
            Operator::Free(_) if id == self.frame.input.variable.logical_id() => {
                value[0] = super::produce::copied(
                    number(self.frame.cas, &self.frame.input.centre, budget)?,
                    budget,
                )?;
                if let Some(linear) = value.get_mut(1) {
                    *linear = Rational::one();
                }
            }
            Operator::Free(_) | Operator::Derivative(_) => {
                let derivative = self.frame.derivative(id, budget)?;
                for (index, value) in value.iter_mut().enumerate() {
                    let index = u32::try_from(index).map_err(|_| invalid())?;
                    *value = super::produce::copied(
                        coefficient(self.frame.cas, self.frame.input, index + derivative, budget)?,
                        budget,
                    )?;
                    for step in 1..=derivative {
                        budget.work(1, AT)?;
                        *value = value.mul(
                            &Rational::from_ratio(i64::from(index + step), 1, budget)?,
                            budget,
                        )?;
                    }
                }
            }
            Operator::Neg => {
                let [child] = children.as_slice() else {
                    return Err(invalid());
                };
                for (value, child) in value.iter_mut().zip(child) {
                    *value = Rational::zero().sub(child, budget)?;
                }
            }
            Operator::Add | Operator::Sub => {
                let [left, right] = children.as_slice() else {
                    return Err(invalid());
                };
                for ((value, left), right) in value.iter_mut().zip(left).zip(right) {
                    *value = if *operator == Operator::Add {
                        left.add(right, budget)?
                    } else {
                        left.sub(right, budget)?
                    };
                }
            }
            Operator::Mul => {
                let [left, right] = children.as_slice() else {
                    return Err(invalid());
                };
                value = multiply(left, right, budget)?;
            }
            Operator::IntegerPower(q) => {
                let [child] = children.as_slice() else {
                    return Err(invalid());
                };
                value[0] = Rational::one();
                for _ in 0..*q {
                    budget.work(1, AT)?;
                    value = multiply(&value, child, budget)?;
                    self.compare(&value, budget)?;
                }
            }
            _ => return Err(unsupported()),
        }
        self.compare(&value, budget)?;
        Ok(value)
    }
}

/// Independently check the original subject, arithmetic, exact equations and inherited conditions.
pub(super) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let (Statement::Series(value), crate::proof_model::Certificate::Series(certificate)) =
        (&draft.conclusion, &draft.certificate)
    else {
        return Err(invalid());
    };
    let SeriesStatement::EvaluatedResidual {
        subject,
        expression,
        dependent,
        output,
        ..
    } = value.as_ref()
    else {
        return Err(invalid());
    };
    let SeriesCertificate::EvaluatedResidual {
        form: 0,
        stage: 0,
        rows,
    } = certificate.as_ref()
    else {
        return Err(unsupported());
    };
    let [premise] = premises else {
        return Err(invalid());
    };
    if draft.rule != crate::rule::Primitive::SeriesResidual
        || !draft.substitutions.is_empty()
        || premise.rule != subject.statement().primitive()
        || premise.judgment.statement != Statement::Series(Box::new(subject.statement()))
        || premise.judgment.context != draft.context
        || draft.context != cas.series_object(subject.output(), budget)?.context
    {
        return Err(invalid());
    }
    roles(cas, value, budget)?;
    let (_, order, count) = preflight(cas, subject, expression, dependent, budget)?;
    if size(rows.len(), AT)? != count {
        return Err(invalid());
    }
    let mut out = crate::encoding::Encoder::new(budget, AT);
    super::codec::certificate(certificate, &mut out)?;
    let encoded = size(out.finish().len(), AT)?;
    // A maliciously small proposed row cannot bound the independently reconstructed value.
    // Four complete schedules cover live children, output and convolution scratch at the
    // active rational ceiling, independently of whether any claimed coefficient is true.
    let coefficient_bytes = budget
        .limits()
        .component_bits
        .div_ceil(8)
        .checked_mul(2)
        .and_then(|n| n.checked_add(128))
        .ok_or_else(invalid)?;
    let workspace = encoded
        .checked_mul(3)
        .and_then(|n| {
            n.checked_add(
                count
                    .checked_mul(u64::from(order))?
                    .checked_mul(coefficient_bytes)?
                    .checked_mul(4)?,
            )
        })
        .ok_or_else(invalid)?;
    budget.with_retained_payload(workspace, AT, |budget| {
        let mut trace = Trace {
            frame: frame(cas, subject, dependent, budget)?,
            rows,
            next: 0,
            order: usize::try_from(order).map_err(|_| invalid())?,
        };
        let values = trace.visit(expression.logical_id(), budget)?;
        if trace.next != rows.len() {
            return Err(invalid());
        }
        for (index, value) in values.iter().enumerate() {
            equal(
                value,
                coefficient(
                    cas,
                    cas.series_object(output, budget)?,
                    u32::try_from(index).map_err(|_| invalid())?,
                    budget,
                )?,
                budget,
            )?;
        }
        Ok(())
    })?;
    let mut conditions = ScopedConditions::default();
    crate::statement::merge(cas, &mut conditions, &premise.conditions, budget)?;
    Ok(conditions)
}
