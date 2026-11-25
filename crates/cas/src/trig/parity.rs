// SPDX-License-Identifier: AGPL-3.0-only
//! Closed parity schemas retain one original Real operand and its exact definedness guard.

use super::{defined, definedness, invalid, value_term};
use crate::budget::Budget;
use crate::error::{Fault, Location};
use crate::graph::{Cas, Handle};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, DomainContract, ScopedConditions, Statement,
    VerifiedWitness,
};
use crate::rule::Primitive;

/// Parity reconstruction and bounded output construction share this location.
const AT: Location = Location::new("trigonometric parity");

/// A closed discriminator is copied without cloning an arbitrary admitted operator payload.
const fn schema(operator: &Operator) -> Result<(Primitive, Operator), Fault> {
    Ok(match operator {
        Operator::Sine => (Primitive::SinOdd, Operator::Sine),
        Operator::Cosine => (Primitive::CosEven, Operator::Cosine),
        Operator::Arctangent => (Primitive::AtanOdd, Operator::Arctangent),
        _ => return Err(invalid()),
    })
}

/// Negation is the existing total scalar constructor; its exact child and frame stay original.
fn negation(
    cas: &Cas,
    negated: &Handle,
    original: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let child = crate::rule_check::unary(
        &cas.state,
        cas.checked(negated, budget)?,
        &Operator::Neg,
        budget,
    )?;
    if child != cas.checked(original, budget)? {
        return Err(invalid());
    }
    frame(cas, negated, original, budget)
}

/// Prepay complete metadata comparisons and retain the exact declaration frame, not printed names.
fn frame(cas: &Cas, value: &Handle, original: &Handle, budget: &mut Budget) -> Result<(), Fault> {
    let value = cas.state.expression(cas.checked(value, budget)?, budget)?;
    let original = cas
        .state
        .expression(cas.checked(original, budget)?, budget)?;
    crate::rule_check::prepay_shape(value, budget)?;
    crate::rule_check::prepay_shape(original, budget)?;
    if value.sort != Sort::Value(AtomicSort::Scalar(Field::Real))
        || value.sort != original.sort
        || value.header != original.header
        || value.scope != original.scope
        || value.ambient != original.ambient
        || value.space.is_some()
        || original.space.is_some()
    {
        return Err(invalid());
    }
    Ok(())
}

/// Both closed applications use the registered totality law's unary domain reconstruction.
/// Checked negation preserves its child's domain; the sole original guard supplies that child.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let [guard] = premises else {
        return Err(invalid());
    };
    let Statement::Equality {
        left,
        right,
        domain,
        domain_contract: DomainContract::DefinedAndEqual,
    } = &draft.conclusion
    else {
        return Err(invalid());
    };
    if draft.certificate != Certificate::None
        || !draft.substitutions.is_empty()
        || guard.judgment.context != draft.context
    {
        return Err(invalid());
    }
    let operator = match draft.rule {
        Primitive::SinOdd => Operator::Sine,
        Primitive::CosEven => Operator::Cosine,
        Primitive::AtanOdd => Operator::Arctangent,
        _ => return Err(invalid()),
    };
    let left_node = cas.state.expression(cas.checked(left, budget)?, budget)?;
    if left_node.operator != operator {
        return Err(invalid());
    }
    let negative = defined::operand(cas, left, budget)?;
    let original = cas.handle(crate::rule_check::unary(
        &cas.state,
        negative.logical_id(),
        &Operator::Neg,
        budget,
    )?);
    negation(cas, &negative, &original, budget)?;
    let function = if draft.rule == Primitive::CosEven {
        right.clone()
    } else {
        cas.handle(crate::rule_check::unary(
            &cas.state,
            cas.checked(right, budget)?,
            &Operator::Neg,
            budget,
        )?)
    };
    if cas
        .state
        .expression(function.logical_id(), budget)?
        .operator
        != operator
    {
        return Err(invalid());
    }
    let repeated = defined::operand(cas, &function, budget)?;
    if repeated != original {
        return Err(invalid());
    }
    for endpoint in [left, &function, right] {
        frame(cas, endpoint, &original, budget)?;
    }
    if draft.rule != Primitive::CosEven {
        negation(cas, right, &function, budget)?;
    }
    definedness(cas, &original, domain, &guard.judgment.statement, budget)?;
    let mut conditions = ScopedConditions::default();
    crate::statement::merge(cas, &mut conditions, &guard.conditions, budget)?;
    Ok(conditions)
}

impl Cas {
    /// Prove parity of an original closed Real function `f(u)` from its checked operand guard.
    ///
    /// The result is `f(-u) = -f(u)` for sine and arctangent, or `f(-u) = f(u)` for cosine.
    /// Original domains, hypotheses and residuals remain attached to the equality.
    /// # Errors
    /// Refuses unsupported operators, malformed original guards and exhausted resources.
    pub fn trig_parity(
        &mut self,
        function: &Handle,
        guard: &Handle,
        budget: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        self.proof_limits(budget, |cas, budget| {
            cas.transaction(budget, |cas, budget| {
                budget.work(1, AT)?;
                let (rule, operator) = schema(
                    &cas.state
                        .expression(cas.checked(function, budget)?, budget)?
                        .operator,
                )?;
                let original = defined::operand(cas, function, budget)?;
                let proof =
                    crate::proof_view::application(cas, cas.checked(guard, budget)?, budget)?;
                let domain = match &proof.judgment.statement {
                    Statement::PredicateFact { region, .. } => region.clone(),
                    Statement::DefinedDomain { domain, .. } => domain.clone(),
                    _ => return Err(invalid()),
                };
                definedness(cas, &original, &domain, &proof.judgment.statement, budget)?;
                let negative = value_term(cas, function, Operator::Neg, vec![original], budget)?;
                let left = value_term(cas, function, operator, vec![negative], budget)?;
                let right = if rule == Primitive::CosEven {
                    function.clone()
                } else {
                    value_term(cas, function, Operator::Neg, vec![function.clone()], budget)?
                };
                let draft = ApplicationDraft {
                    rule,
                    context: proof.judgment.context.clone(),
                    conclusion: Statement::Equality {
                        left,
                        right,
                        domain,
                        domain_contract: DomainContract::DefinedAndEqual,
                    },
                    premises: Vec::new(),
                    substitutions: Vec::new(),
                    certificate: Certificate::None,
                };
                let mut checked =
                    crate::scoped_check::check_application(cas, &draft, &[proof], budget)?;
                checked.premises.push(guard.logical_id());
                let result = cas.publish_scoped(checked, budget)?;
                cas.seal_scoped(&result, budget)
            })
        })
    }
}
