// SPDX-License-Identifier: AGPL-3.0-only
//! Family recipes publish only existing region, context and case applications.

use super::{AT, body, invalid, native};
use crate::budget::{Budget, BudgetKind};
use crate::case_producer::Plan;
use crate::domain_model::{RegionDraft, RegionExpr};
use crate::error::Fault;
use crate::graph::{Cas, Draft, Handle, LogicalId, Reference};
use crate::proof_model::{Certificate, DomainContract, Statement, VerifiedWitness};
use crate::rule::Primitive;
use crate::search::SearchPolicy;
use crate::solving::radicals::construction::admit;

impl Cas {
    /// Prove an identity on the exact current family piece by component arithmetic.
    ///
    /// The exported proof contains ordinary projection subsets and case congruence.
    ///
    /// # Errors
    /// Refuses missing projection, definedness, reciprocal or selection evidence, or an exhausted budget.
    pub fn family_identity(
        &mut self,
        context: &Handle,
        left: &Handle,
        right: &Handle,
        generation: &mut Budget,
        checker: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        self.transaction(generation, |cas, generation| {
            let domain = cas.handle(cas.state.context(cas.checked(context)?)?.admissible);
            let conclusion = Statement::Equality {
                left: left.clone(),
                right: right.clone(),
                domain,
                domain_contract: DomainContract::DefinedAndEqual,
            };
            identity(cas, context, &conclusion, generation, checker)
        })
    }

    /// Prove overlap compatibility of two original native family associations.
    ///
    /// # Errors
    /// Refuses an unproved overlap, mismatched family, partial value or failed component identity.
    pub fn family_agreement(
        &mut self,
        context: &Handle,
        left: &Handle,
        right: &Handle,
        generation: &mut Budget,
        checker: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        self.transaction(generation, |cas, generation| {
            let region = cas.handle(cas.state.context(cas.checked(context)?)?.admissible);
            identity(
                cas,
                context,
                &Statement::Compatibility {
                    left: left.clone(),
                    right: right.clone(),
                    region,
                },
                generation,
                checker,
            )
        })
    }

    /// Restrict a family identity or closed base-local fact with explicit subset evidence.
    ///
    /// # Errors
    /// Refuses widening, unrelated contexts or families, or an exhausted ledger.
    pub fn family_restrict(
        &mut self,
        context: &Handle,
        source: &VerifiedWitness,
        generation: &mut Budget,
        checker: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        self.transaction(generation, |cas, generation| {
            let statement = &source.judgment().statement;
            let original = super::restriction::domain(statement)
                .ok_or_else(invalid)?
                .clone();
            let target = cas.handle(cas.state.context(cas.checked(context)?)?.admissible);
            super::region::family(cas, target.logical_id(), generation)?;
            let original = if cas.state.region(original.logical_id())?.ambient
                == cas.state.region(target.logical_id())?.ambient
            {
                original
            } else {
                pullback_from(cas, &target, &original, generation)?
            };
            let policy = SearchPolicy::default();
            let mut plan = Plan::new(&policy);
            let subset = plan.push(
                cas,
                context,
                Primitive::RegionConstruct,
                Statement::Subset {
                    subset: target.clone(),
                    superset: original,
                },
                &[],
                Certificate::None,
                generation,
                checker,
            )?;
            let proof = plan.push(
                cas,
                context,
                Primitive::ContextWeaken,
                super::restriction::restricted(statement, &target),
                &[source.proof().clone(), subset],
                Certificate::None,
                generation,
                checker,
            )?;
            Ok(plan.finish(cas, &proof, Vec::new(), checker)?.witness)
        })
    }
}

/// Local agreement is a finite expansion, not a family theorem manifest.
fn identity(
    cas: &mut Cas,
    context: &Handle,
    conclusion: &Statement,
    generation: &mut Budget,
    checker: &mut Budget,
) -> Result<VerifiedWitness, Fault> {
    if !native::handles(cas, conclusion)? {
        return Err(invalid());
    }
    let policy = SearchPolicy::default();
    let mut plan = Plan::new(&policy);
    let proof = push(
        &mut plan,
        cas,
        context,
        Primitive::CaseCongruence,
        conclusion,
        &[],
        generation,
        checker,
    )?;
    Ok(plan.finish(cas, &proof, Vec::new(), checker)?.witness)
}

/// Retain all original piece domains reached through local bodies or scalar operands.
pub(crate) fn regions(
    cas: &Cas,
    statement: &Statement,
    budget: &mut Budget,
) -> Result<Vec<Handle>, Fault> {
    let mut pending = match statement {
        Statement::Equality { left, right, .. } => vec![left.logical_id(), right.logical_id()],
        Statement::Compatibility { left, right, .. } => vec![
            cas.state.arm(left.logical_id())?.value,
            cas.state.arm(right.logical_id())?.value,
        ],
        Statement::DefinedDomain { function, .. } => vec![function.logical_id()],
        Statement::PredicateFact { predicate, .. } => {
            super::native::predicate(cas, predicate.logical_id())?
                .1
                .to_vec()
        }
        _ => return Err(invalid()),
    };
    let mut result = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    while let Some(id) = pending.pop() {
        budget.work(crate::graph::size(seen.len().saturating_add(1), AT)?, AT)?;
        if !seen.insert(id) {
            continue;
        }
        if let Some(value) = body(cas, id)? {
            if !result.contains(&value.region) {
                result.push(value.region.clone());
            }
            pending.extend(
                value
                    .arguments
                    .iter()
                    .map(|argument| argument.value.logical_id()),
            );
        } else {
            let expression = cas.state.expression(id)?;
            pending.extend(&expression.operands);
            for arm in &expression.arms {
                pending.push(cas.state.arm(*arm)?.value);
            }
        }
    }
    Ok(result)
}

/// Materialize the exact preimage syntax before any subset theorem is requested.
pub(crate) fn pullback(
    cas: &mut Cas,
    total: &Handle,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let over = cas
        .state
        .region(cas.checked(total)?)?
        .lies_over
        .as_ref()
        .ok_or_else(invalid)?;
    let source = cas.handle(over.stratum);
    pullback_from(cas, total, &source, budget)
}

/// A source theorem's base region is copied explicitly into the total coordinate product.
fn pullback_from(
    cas: &mut Cas,
    total: &Handle,
    source: &Handle,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let region = cas.state.region(cas.checked(total)?)?.clone();
    let family = super::region::family(cas, total.logical_id(), budget)?;
    let root = lift(cas, source.logical_id(), &region.ambient, 1, budget)?;
    let copied = cas.state.region(root.logical_id())?.clone();
    admit(
        cas,
        &Draft::FamilyRegion {
            region: RegionDraft {
                header: Reference::Existing(cas.handle(copied.header)),
                scope: copied.scope,
                ambient: copied.ambient,
                expression: references(cas, &copied.expression),
            },
            lies_over: crate::solving::LiesOver {
                family: Reference::Existing(cas.handle(family)),
                stratum: Reference::Existing(source.clone()),
            },
        },
        budget,
    )
}

/// Copy a finite base predicate tree into the exact total coordinate product.
fn lift(
    cas: &mut Cas,
    root: LogicalId,
    ambient: &crate::domain_model::AmbientDomain,
    depth: u64,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    budget.work(1, AT)?;
    budget.check(BudgetKind::Depth, depth, AT)?;
    let source = cas.state.region(root)?.clone();
    let next = depth.saturating_add(1);
    let expression = match source.expression {
        RegionExpr::Ambient => RegionExpr::Ambient,
        RegionExpr::Empty => RegionExpr::Empty,
        RegionExpr::GuardTrue { base, predicate } => RegionExpr::GuardTrue {
            base: Reference::Existing(lift(cas, base, ambient, next, budget)?),
            predicate: Reference::Existing(cas.handle(predicate)),
        },
        RegionExpr::GuardFalse { base, predicate } => RegionExpr::GuardFalse {
            base: Reference::Existing(lift(cas, base, ambient, next, budget)?),
            predicate: Reference::Existing(cas.handle(predicate)),
        },
        RegionExpr::Meet(parts) => {
            RegionExpr::Meet(lift_parts(cas, &parts, ambient, next, budget)?)
        }
        RegionExpr::Union(parts) => {
            RegionExpr::Union(lift_parts(cas, &parts, ambient, next, budget)?)
        }
        RegionExpr::Difference { base, excluded } => RegionExpr::Difference {
            base: Reference::Existing(lift(cas, base, ambient, next, budget)?),
            excluded: Reference::Existing(lift(cas, excluded, ambient, next, budget)?),
        },
    };
    admit(
        cas,
        &Draft::Region(RegionDraft {
            header: Reference::Existing(cas.handle(source.header)),
            scope: source.scope,
            ambient: ambient.clone(),
            expression,
        }),
        budget,
    )
}

/// A region's finite child list shares the same depth and request ledger.
fn lift_parts(
    cas: &mut Cas,
    parts: &[LogicalId],
    ambient: &crate::domain_model::AmbientDomain,
    depth: u64,
    budget: &mut Budget,
) -> Result<Vec<Reference>, Fault> {
    let mut result = crate::solving::coefficients::storage::vector(parts.len(), budget)?;
    for part in parts {
        result.push(Reference::Existing(lift(
            cas, *part, ambient, depth, budget,
        )?));
    }
    Ok(result)
}

/// Change only reference representation when attaching the final explicit projection.
fn references(cas: &Cas, expression: &RegionExpr<LogicalId>) -> RegionExpr {
    let handle = |id| Reference::Existing(cas.handle(id));
    match expression {
        RegionExpr::Ambient => RegionExpr::Ambient,
        RegionExpr::Empty => RegionExpr::Empty,
        RegionExpr::GuardTrue { base, predicate } => RegionExpr::GuardTrue {
            base: handle(*base),
            predicate: handle(*predicate),
        },
        RegionExpr::GuardFalse { base, predicate } => RegionExpr::GuardFalse {
            base: handle(*base),
            predicate: handle(*predicate),
        },
        RegionExpr::Meet(parts) => RegionExpr::Meet(parts.iter().copied().map(handle).collect()),
        RegionExpr::Union(parts) => RegionExpr::Union(parts.iter().copied().map(handle).collect()),
        RegionExpr::Difference { base, excluded } => RegionExpr::Difference {
            base: handle(*base),
            excluded: handle(*excluded),
        },
    }
}

/// Native services share the request's one plan and both cumulative ledgers.
#[expect(
    clippy::too_many_arguments,
    reason = "the existing case plan and independent ledgers remain explicit"
)]
pub(crate) fn push(
    plan: &mut Plan<'_>,
    cas: &mut Cas,
    context: &Handle,
    rule: Primitive,
    conclusion: &Statement,
    supplied: &[Handle],
    generation: &mut Budget,
    checker: &mut Budget,
) -> Result<Handle, Fault> {
    let regions = regions(cas, conclusion, generation)?;
    let mut premises = crate::solving::coefficients::storage::copied(supplied, generation)?;
    for region in regions {
        let lift = pullback(cas, &region, generation)?;
        premises.push(plan.push(
            cas,
            context,
            Primitive::RegionConstruct,
            Statement::Subset {
                subset: region,
                superset: lift,
            },
            &[],
            Certificate::None,
            generation,
            checker,
        )?);
    }
    plan.push(
        cas,
        context,
        rule,
        conclusion.clone(),
        &premises,
        Certificate::None,
        generation,
        checker,
    )
}
