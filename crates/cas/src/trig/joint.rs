// SPDX-License-Identifier: AGPL-3.0-only
//! One original Real argument binds the exact circular equality.

use super::{definedness, invalid, value_term};
use crate::budget::Budget;
use crate::error::{Fault, Location};
use crate::graph::{Cas, Handle};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, DomainContract, ScopedConditions, Statement,
    VerifiedWitness,
};
use crate::rule::Primitive;

/// Location for the joint law's budget and refusal events.
const AT: Location = Location::new("joint trigonometric law");

/// Read one exact square and its closed sine or cosine child.
fn square(
    cas: &Cas,
    term: crate::graph::LogicalId,
    operator: &Operator,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let closed = crate::rule_check::unary(&cas.state, term, &Operator::IntegerPower(2), budget)?;
    let argument = crate::rule_check::unary(&cas.state, closed, operator, budget)?;
    let square = cas.state.expression(term, budget)?;
    let node = cas.state.expression(closed, budget)?;
    if square.domain.is_some() || node.domain.is_some() {
        return Err(invalid());
    }
    Ok(cas.handle(argument))
}

/// Validate the exact equality shape and recover the shared original argument.
fn roles(cas: &Cas, left: &Handle, right: &Handle, budget: &mut Budget) -> Result<Handle, Fault> {
    budget.work(1, AT)?;
    let (sine_square, cosine_square) = crate::rule_check::binary(
        &cas.state,
        cas.checked(left, budget)?,
        &Operator::Add,
        budget,
    )?;
    let sine_argument = square(cas, sine_square, &Operator::Sine, budget)?;
    let cosine_argument = square(cas, cosine_square, &Operator::Cosine, budget)?;
    if sine_argument != cosine_argument {
        return Err(invalid());
    }
    let original = cas
        .state
        .expression(cas.checked(&sine_argument, budget)?, budget)?;
    let sum = cas.state.expression(cas.checked(left, budget)?, budget)?;
    let one = cas.state.expression(cas.checked(right, budget)?, budget)?;
    let real = Sort::Value(AtomicSort::Scalar(Field::Real));
    for node in [original, sum, one] {
        crate::rule_check::prepay_shape(node, budget)?;
        if node.sort != real
            || node.scope != original.scope
            || node.ambient != original.ambient
            || node.header != original.header
            || node.space.is_some()
        {
            return Err(invalid());
        }
    }
    if sum.domain.is_some()
        || one.domain.is_some()
        || one.operator != Operator::Rational(crate::number::Rational::one())
        || !one.operands.is_empty()
    {
        return Err(invalid());
    }
    Ok(sine_argument)
}

/// The registered analytic identity consumes one exact original definedness guard.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let Statement::Equality {
        left,
        right,
        domain,
        domain_contract: DomainContract::DefinedAndEqual,
    } = &draft.conclusion
    else {
        return Err(invalid());
    };
    let [guard] = premises else {
        return Err(invalid());
    };
    if draft.certificate != Certificate::None
        || !draft.substitutions.is_empty()
        || guard.judgment.context != draft.context
    {
        return Err(invalid());
    }
    let argument = roles(cas, left, right, budget)?;
    definedness(cas, &argument, domain, &guard.judgment.statement, budget)?;
    // Sine, cosine, positive integer powers and addition are total on their
    // defined Real operands; all native case-domain fields above are empty.
    let mut conditions = ScopedConditions::default();
    crate::statement::merge(cas, &mut conditions, &guard.conditions, budget)?;
    Ok(conditions)
}

impl Cas {
    /// Check the original sine/cosine circular equality on the original domain.
    ///
    /// The result is an Equality consumable as `EqualOnDomain`. Derivative laws
    /// belong to their separate checked primitives.
    /// # Errors
    /// Refuses a foreign or malformed guard, non-Real argument, or exhausted resources.
    pub fn trig_joint(
        &mut self,
        argument: &Handle,
        guard: &Handle,
        budget: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        self.proof_limits(budget, |cas, budget| {
            cas.transaction(budget, |cas, budget| {
                budget.work(1, AT)?;
                let original = cas
                    .state
                    .expression(cas.checked(argument, budget)?, budget)?;
                if original.sort != Sort::Value(AtomicSort::Scalar(Field::Real))
                    || original.space.is_some()
                {
                    return Err(invalid());
                }
                let proof =
                    crate::proof_view::application(cas, cas.checked(guard, budget)?, budget)?;
                let domain = match &proof.judgment.statement {
                    Statement::PredicateFact { region, .. } => region.clone(),
                    Statement::DefinedDomain { domain, .. } => domain.clone(),
                    _ => return Err(invalid()),
                };
                definedness(cas, argument, &domain, &proof.judgment.statement, budget)?;
                let sine = value_term(
                    cas,
                    argument,
                    Operator::Sine,
                    vec![argument.clone()],
                    budget,
                )?;
                let cosine = value_term(
                    cas,
                    argument,
                    Operator::Cosine,
                    vec![argument.clone()],
                    budget,
                )?;
                let sine_square =
                    value_term(cas, argument, Operator::IntegerPower(2), vec![sine], budget)?;
                let cosine_square = value_term(
                    cas,
                    argument,
                    Operator::IntegerPower(2),
                    vec![cosine],
                    budget,
                )?;
                let sum = value_term(
                    cas,
                    argument,
                    Operator::Add,
                    vec![sine_square, cosine_square],
                    budget,
                )?;
                let one = value_term(
                    cas,
                    argument,
                    Operator::Rational(crate::number::Rational::one()),
                    Vec::new(),
                    budget,
                )?;
                let draft = ApplicationDraft {
                    rule: Primitive::TrigJoint,
                    context: proof.judgment.context.clone(),
                    conclusion: Statement::Equality {
                        left: sum,
                        right: one,
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
