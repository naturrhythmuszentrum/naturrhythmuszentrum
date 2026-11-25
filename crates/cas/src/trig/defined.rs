// SPDX-License-Identifier: AGPL-3.0-only
//! Registered totality of the three closed Real trigonometric operators.

use super::{definedness, invalid};
use crate::budget::Budget;
use crate::error::{Fault, Location};
use crate::graph::{Cas, Draft, Handle, Reference, Vertex, size};
use crate::model::{AtomicSort, Field, Operator, PredicateKind, Sort};
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, ScopedConditions, Statement, VerifiedWitness,
};
use crate::rule::Primitive;

/// Registered totality checks and their temporary predicate construction.
const AT: Location = Location::new("trigonometric totality");

/// Inspect the closed discriminator before copying any input-controlled metadata.
pub(super) fn operand(cas: &Cas, function: &Handle, budget: &mut Budget) -> Result<Handle, Fault> {
    budget.work(1, AT)?;
    let value = cas
        .state
        .expression(cas.checked(function, budget)?, budget)?;
    if !matches!(
        value.operator,
        Operator::Sine | Operator::Cosine | Operator::Arctangent
    ) || value.sort != Sort::Value(AtomicSort::Scalar(Field::Real))
        || value.space.is_some()
    {
        return Err(invalid());
    }
    let [child] = value.operands.as_slice() else {
        return Err(invalid());
    };
    let original = cas.state.expression(*child, budget)?;
    crate::rule_check::prepay_shape(original, budget)?;
    crate::rule_check::prepay_shape(value, budget)?;
    if original.sort != value.sort
        || original.scope != value.scope
        || original.ambient != value.ambient
    {
        return Err(invalid());
    }
    Ok(cas.handle(*child))
}

/// The sole premise is exact original definedness, including all child obligations.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    if draft.certificate != Certificate::None || !draft.substitutions.is_empty() {
        return Err(invalid());
    }
    let [guard] = premises else {
        return Err(invalid());
    };
    let Statement::PredicateFact { region, predicate } = &draft.conclusion else {
        return Err(invalid());
    };
    let Vertex::Condition(condition) = cas.state.node(cas.checked(predicate, budget)?, budget)?
    else {
        return Err(invalid());
    };
    if condition.kind != PredicateKind::Defined {
        return Err(invalid());
    }
    let [function] = condition.arguments.as_slice() else {
        return Err(invalid());
    };
    let original = operand(cas, &cas.handle(*function), budget)?;
    if guard.judgment.context != draft.context {
        return Err(invalid());
    }
    definedness(cas, &original, region, &guard.judgment.statement, budget)?;
    let mut result = ScopedConditions::default();
    crate::statement::merge(cas, &mut result, &guard.conditions, budget)?;
    Ok(result)
}

/// Construct a Defined predicate with scope copying prepaid and retained through admission.
pub(crate) fn predicate(
    cas: &mut Cas,
    function: &Handle,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let value = cas
        .state
        .expression(cas.checked(function, budget)?, budget)?;
    let header = cas.handle(value.header);
    let bytes = if value.scope.is_empty() {
        0
    } else {
        crate::validation::scope_limit(&value.scope, budget)?;
        let mut out = crate::encoding::Encoder::new(budget, AT);
        crate::encoding::scope(&value.scope, &mut out)?;
        let bytes = size(out.finish().len(), AT)?;
        budget.work(bytes, AT)?;
        bytes
    };
    budget.with_retained_payload(bytes, AT, |budget| {
        let source = &cas
            .state
            .expression(cas.checked(function, budget)?, budget)?
            .scope;
        let mut scope = Vec::new();
        scope
            .try_reserve_exact(source.len())
            .map_err(|_| Fault::allocation(AT))?;
        for frame in source {
            let mut copied = Vec::new();
            copied
                .try_reserve_exact(frame.len())
                .map_err(|_| Fault::allocation(AT))?;
            copied.extend(frame.iter().cloned());
            scope.push(copied);
        }
        cas.admit(
            &[Draft::Condition {
                header: Reference::Existing(header),
                scope,
                kind: PredicateKind::Defined,
                arguments: vec![Reference::Existing(function.clone())],
            }],
            budget,
        )?
        .pop()
        .ok_or_else(invalid)
    })
}

impl Cas {
    /// Prove totality of a closed Real sine, cosine or arctangent from its original operand guard.
    ///
    /// The exact domain, hypotheses and residual conditions of the guard remain attached.
    /// # Errors
    /// Refuses other operators, malformed premise joins and exhausted resources.
    pub fn trig_defined(
        &mut self,
        function: &Handle,
        guard: &Handle,
        budget: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        self.proof_limits(budget, |cas, budget| {
            cas.transaction(budget, |cas, budget| {
                let original = operand(cas, function, budget)?;
                let proof =
                    crate::proof_view::application(cas, cas.checked(guard, budget)?, budget)?;
                let region = match &proof.judgment.statement {
                    Statement::PredicateFact { region, .. } => region.clone(),
                    Statement::DefinedDomain { domain, .. } => domain.clone(),
                    _ => return Err(invalid()),
                };
                definedness(cas, &original, &region, &proof.judgment.statement, budget)?;
                let conclusion = Statement::PredicateFact {
                    region,
                    predicate: predicate(cas, function, budget)?,
                };
                let draft = ApplicationDraft {
                    rule: Primitive::TrigDefined,
                    context: proof.judgment.context.clone(),
                    conclusion,
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
