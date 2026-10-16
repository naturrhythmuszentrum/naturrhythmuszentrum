// SPDX-License-Identifier: AGPL-3.0-only
//! Close only an independently discharged Vieta identity into its exact unscoped parent.

use super::formal_shape;
use super::storage::{invalid, vector};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::Scope;
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, ScopedConditions, Statement,
};
use crate::rule::Primitive;
use crate::solving::{
    CertificateCoefficientIdentity, IdentityMode, SolveCertificate, SolveDraft, SolveSchema,
    SolveStatement, StatementCoefficientEquality,
};

/// Admission retains the exact original parent edge, rather than inventing an independent root context.
pub(crate) use super::formal_shape::introduction;

/// A scope-changing close has exactly one scoped identity premise and no substitutions.
pub(crate) fn boundary(
    cas: &Cas,
    draft: &ApplicationDraft,
    earlier: &[CheckedApplication],
) -> Result<bool, Fault> {
    if draft.rule != Primitive::ContextClose {
        return Ok(false);
    }
    let Certificate::Close { frame } = &draft.certificate else {
        return Ok(false);
    };
    let local = cas.state.context(cas.checked(frame)?)?;
    let scope = &cas.state.region(local.admissible)?.scope;
    let target = cas.state.context(cas.checked(&draft.context)?)?;
    if scope.is_empty() || !cas.state.region(target.admissible)?.scope.is_empty() {
        return Ok(false);
    }
    formal_shape::frame(scope)?;
    if local.parent != Some(cas.checked(&draft.context)?)
        || earlier.len() != 1
        || earlier
            .first()
            .is_none_or(|source| source.judgment.context != *frame)
        || !draft.substitutions.is_empty()
    {
        return Err(invalid());
    }
    Ok(true)
}

/// The checked source must be the anchored identity itself, never a caller's discharge assertion.
fn certificate(source: &CheckedApplication) -> Result<&CertificateCoefficientIdentity, Fault> {
    if source.rule != Primitive::Solve(SolveSchema::CoefficientIdentity) {
        return Err(invalid());
    }
    let Certificate::Solve(certificate) = &source.certificate else {
        return Err(invalid());
    };
    let SolveCertificate::CoefficientIdentity(identity) = certificate.as_ref() else {
        return Err(invalid());
    };
    if identity.mode != IdentityMode::RelationCombination || identity.combinations.is_empty() {
        return Err(invalid());
    }
    Ok(identity)
}

/// Independently check that the referenced existence premise belongs to this exact local frame.
fn existence(
    cas: &Cas,
    source: &CheckedApplication,
    anchor: &Handle,
    region: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for proof in &source.premises {
        budget.work(1, formal_shape::AT)?;
        let premise = crate::proof_view::application(cas, *proof, budget)?;
        if premise.judgment.context != source.judgment.context {
            continue;
        }
        if let Statement::Solve(statement) = &premise.judgment.statement
            && let SolveStatement::RootExistence(value) = statement.as_ref()
            && (&value.relation, &value.region) == (anchor, region)
            && premise
                .conditions
                .hypotheses
                .iter()
                .all(|used| used.context != source.judgment.context)
        {
            return Ok(());
        }
    }
    Err(invalid())
}

/// The complete identical root interpretation and anchor remain actual dependencies of the proof.
fn anchored(
    cas: &Cas,
    source: &CheckedApplication,
    scope: &Scope,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let identity = certificate(source)?;
    let first = identity
        .combinations
        .first()
        .and_then(|row| row.hypothesis.as_ref())
        .ok_or_else(invalid)?;
    let anchor = first.vieta.as_ref().ok_or_else(invalid)?;
    let SolveDraft::RootRelation(relation) =
        crate::solving::graph::object(&cas.state, cas.checked(anchor)?)?
    else {
        return Err(invalid());
    };
    if (
        &relation.scope,
        &relation.map,
        &relation.region,
        &relation.ambient,
    ) != (
        scope,
        &identity.map,
        &identity.region,
        &crate::model::Field::Complex,
    ) {
        return Err(invalid());
    }
    for multiple in &identity.combinations {
        budget.work(
            crate::graph::size(first.roots.len(), formal_shape::AT)?.max(1),
            formal_shape::AT,
        )?;
        let row = multiple.hypothesis.as_ref().ok_or_else(invalid)?;
        if (&row.vieta, &row.roots) != (&first.vieta, &first.roots) {
            return Err(invalid());
        }
    }
    existence(cas, source, anchor, &identity.region, budget)
}

/// Extract ordinary coefficient endpoints; no other proposition is transported by this subcase.
fn equality(statement: &Statement) -> Result<&StatementCoefficientEquality, Fault> {
    let Statement::Solve(statement) = statement else {
        return Err(invalid());
    };
    let SolveStatement::CoefficientEquality(value) = statement.as_ref() else {
        return Err(invalid());
    };
    Ok(value)
}

/// Every escaped condition keeps its original kind and exact ancestry, with no local discharge here.
fn conditions(
    cas: &mut Cas,
    inherited: &ScopedConditions,
    source: &Handle,
    target: &Handle,
    scope: &Scope,
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let mut result = inherited.clone();
    for used in &result.hypotheses {
        if used.context == *source
            || !crate::context_check::ancestor(
                cas,
                cas.checked(&used.context)?,
                cas.checked(target)?,
                budget,
            )?
        {
            return Err(invalid());
        }
        let frame = cas.state.context(cas.checked(&used.context)?)?;
        let handle = if used.slot == 0 && frame.parent.is_some() {
            cas.handle(frame.admissible)
        } else {
            crate::context_check::predicate(cas, used)?
        };
        super::independent::object(cas, &handle, scope, budget)?;
    }
    for values in [&mut result.requirements, &mut result.residuals] {
        for value in values.iter_mut() {
            if value.home != *source
                && !crate::context_check::ancestor(
                    cas,
                    cas.checked(&value.home)?,
                    cas.checked(target)?,
                    budget,
                )?
            {
                return Err(invalid());
            }
            value.predicate = formal_shape::erase(cas, &value.predicate, scope, budget)?;
            value.region = formal_shape::erase(cas, &value.region, scope, budget)?;
            if value.scope == *scope {
                value.scope.clear();
            }
            if !value.scope.is_empty() {
                return Err(invalid());
            }
            if value.home == *source {
                value.home.clone_from(target);
            }
        }
        *values = crate::statement::union(cas, values, budget)?;
    }
    Ok(result)
}

/// A one-premise close retains the entire scoped derivation in canonical export and fresh replay.
pub(crate) fn close(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    earlier: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    if !boundary(cas, draft, earlier)? {
        return Err(invalid());
    }
    let source = earlier.first().ok_or_else(invalid)?;
    let local = cas.state.context(cas.checked(&source.judgment.context)?)?;
    let region = local.admissible;
    let scope = cas.state.region(region)?.scope.clone();
    let parent = cas.state.context(cas.checked(&draft.context)?)?;
    if local.header_id != parent.header_id || local.restriction != Some(region) {
        return Err(invalid());
    }
    let target_region = parent.admissible;
    formal_shape::paired(&cas.state, region, target_region, &scope, budget)?;
    anchored(cas, source, &scope, budget)?;
    let original = equality(&source.judgment.statement)?;
    let target = equality(&draft.conclusion)?;
    if original.region.logical_id() != region || target.region.logical_id() != target_region {
        return Err(invalid());
    }
    let mut pairs = vector(4, budget)?;
    pairs.extend([
        (&original.left, &target.left),
        (&original.right, &target.right),
        (&original.map, &target.map),
        (&original.region, &target.region),
    ]);
    for (left, right) in pairs {
        super::independent::object(cas, left, &scope, budget)?;
        formal_shape::paired(
            &cas.state,
            cas.checked(left)?,
            cas.checked(right)?,
            &scope,
            budget,
        )?;
    }
    super::independent::surroundings(cas, &source.judgment.context, &scope, budget)?;
    conditions(
        cas,
        &source.conditions,
        &source.judgment.context,
        &draft.context,
        &scope,
        budget,
    )
}

#[cfg(test)]
mod tests {
    use super::conditions;
    use crate::budget::Budget;
    use crate::domain_model::{ContextDraft, RegionDraft, RegionExpr};
    use crate::graph::{Draft, ExpressionDraft, Reference};
    use crate::limits::Limits;
    use crate::model::{AtomicSort, Field, Operator, PredicateKind, Sort};
    use crate::proof_model::{GuardedObligation, ScopedConditions};
    use crate::testing::common::TestResult;
    use crate::testing::solve_graph::admit;

    /// Escaped obligations preserve their kind, while any root occurrence refuses before admission.
    /// ´test:unit:formal-close-retained-conditions´
    #[test]
    fn formal_close_retained_conditions() -> TestResult {
        for dependent in [false, true] {
            let mut fixture = crate::testing::solve_coefficients::Fixture::new()?;
            let scope = vec![vec![AtomicSort::Scalar(Field::Complex); 3]];
            let ambient = fixture
                .cas
                .state
                .region(fixture.region.logical_id())?
                .ambient
                .clone();
            let region = admit(
                &mut fixture.cas,
                &Draft::Region(RegionDraft {
                    header: Reference::Existing(fixture.header.clone()),
                    scope: scope.clone(),
                    ambient,
                    expression: RegionExpr::Ambient,
                }),
            )?;
            let source = admit(
                &mut fixture.cas,
                &Draft::Context(ContextDraft::Local {
                    parent: Reference::Existing(fixture.context.clone()),
                    region: Reference::Existing(region.clone()),
                    hypotheses: Vec::new(),
                }),
            )?;
            let operator = if dependent {
                Operator::Bound { depth: 0, slot: 2 }
            } else {
                Operator::Rational(crate::number::Rational::zero())
            };
            let scalar = admit(
                &mut fixture.cas,
                &Draft::Expression(ExpressionDraft {
                    header: Reference::Existing(fixture.header.clone()),
                    scope: scope.clone(),
                    operator,
                    sort: Sort::Value(AtomicSort::Scalar(Field::Complex)),
                    operands: Vec::new(),
                    space: None,
                    binding: None,
                }),
            )?;
            let predicate = admit(
                &mut fixture.cas,
                &Draft::Condition {
                    header: Reference::Existing(fixture.header.clone()),
                    scope: scope.clone(),
                    kind: PredicateKind::Equal,
                    arguments: vec![
                        Reference::Existing(scalar.clone()),
                        Reference::Existing(scalar),
                    ],
                },
            )?;
            let value = GuardedObligation {
                home: source.clone(),
                scope: scope.clone(),
                layer: crate::domain_model::Layer::Logic,
                space: None,
                region,
                predicate,
            };
            let inherited = ScopedConditions {
                requirements: vec![value.clone()],
                residuals: vec![value],
                hypotheses: Vec::new(),
            };
            let result = conditions(
                &mut fixture.cas,
                &inherited,
                &source,
                &fixture.context,
                &scope,
                &mut Budget::new(Limits::blind_cubic())?,
            );
            if dependent {
                assert!(result.is_err());
            } else {
                let result = result?;
                assert_eq!(result.requirements, result.residuals);
                assert_eq!(result.requirements.len(), 1);
                let retained = result.requirements.first().ok_or("lost requirement")?;
                assert_eq!(retained.home, fixture.context);
                assert_eq!(retained.scope, crate::model::Scope::new());
                assert_eq!(retained.region, fixture.region);
            }
        }
        Ok(())
    }
}
