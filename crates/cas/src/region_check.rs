// SPDX-License-Identifier: AGPL-3.0-only
//! Independent exact certificates and structural constructor laws for region judgments.

use crate::budget::{Budget, BudgetKind};
use crate::domain_model::RegionExpr;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, LogicalId, size};
use crate::proof_model::{Certificate, ConstructorLaw, Statement};
use crate::region::RegionCertificate;

/// Exact set theorem location.
const AT: Location = Location::new("region theorem");

/// A certificate or conclusion did not match its registered schema.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Rule, AT)
}

/// Ordered roots defining the exact rows of a region decision certificate.
pub(crate) fn roots(cas: &Cas, statement: &Statement) -> Result<Vec<LogicalId>, Fault> {
    let handles: Vec<&Handle> = match statement {
        Statement::Subset { subset, superset } => vec![subset, superset],
        Statement::Disjoint { left, right } => vec![left, right],
        Statement::Cover { domain, members } => {
            let mut result = vec![domain];
            result
                .try_reserve(members.len())
                .map_err(|_| Fault::allocation(AT))?;
            result.extend(members);
            result
        }
        Statement::Empty { region } | Statement::PredicateFact { region, .. } => vec![region],
        Statement::Incidence {
            region,
            neighborhood,
            ..
        } => vec![region, neighborhood],
        _ => return Err(invalid()),
    };
    let mut result = Vec::new();
    result
        .try_reserve_exact(handles.len())
        .map_err(|_| Fault::allocation(AT))?;
    for handle in handles {
        let id = cas.checked(handle)?;
        cas.state.region(id)?;
        result.push(id);
    }
    Ok(result)
}

/// Recompute cuts and every cell before checking the asserted set relation.
pub(crate) fn decide(
    cas: &mut Cas,
    statement: &Statement,
    certificate: &Certificate,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Certificate::Region(certificate) = certificate else {
        return Err(invalid());
    };
    measure(certificate, budget)?;
    let expected = evidence(cas, statement, budget)?;
    if *certificate != expected {
        return Err(invalid());
    }
    let first = expected.membership.first().ok_or_else(invalid)?;
    let valid = match statement {
        Statement::Empty { .. } => first.iter().all(|value| !value),
        Statement::PredicateFact { .. } => expected.membership.get(1).map_or_else(
            || first.iter().all(|value| !value),
            |truth| {
                first
                    .iter()
                    .zip(truth)
                    .all(|(reached, holds)| !reached || *holds)
            },
        ),
        Statement::Incidence { .. } => incidence(cas, statement, &expected, budget)?,
        Statement::Subset { .. } => {
            let second = expected.membership.get(1).ok_or_else(invalid)?;
            first
                .iter()
                .zip(second)
                .all(|(left, right)| !left || *right)
        }
        Statement::Disjoint { .. } => {
            let second = expected.membership.get(1).ok_or_else(invalid)?;
            first
                .iter()
                .zip(second)
                .all(|(left, right)| !(*left && *right))
        }
        Statement::Cover { .. } => covered(&expected, budget)?,
        _ => false,
    };
    if valid { Ok(()) } else { Err(invalid()) }
}

/// Every inhabited parent cell needs at least one explicit covering member.
fn covered(cells: &RegionCertificate, budget: &mut Budget) -> Result<bool, Fault> {
    let parent = cells.membership.first().ok_or_else(invalid)?;
    let mut valid = true;
    for (index, member) in parent.iter().enumerate() {
        budget.condition(1, AT)?;
        if *member
            && !cells
                .membership
                .iter()
                .skip(1)
                .any(|row| row.get(index) == Some(&true))
        {
            valid = false;
        }
    }
    Ok(valid)
}

/// Reconstruct predicate truth only after checking whether its region is empty.
pub(crate) fn evidence(
    cas: &mut Cas,
    statement: &Statement,
    budget: &mut Budget,
) -> Result<RegionCertificate, Fault> {
    let mut roots = roots(cas, statement)?;
    if let Statement::PredicateFact { region, predicate } = statement {
        let empty = crate::region::partition(&cas.state, &roots, budget)?;
        if empty
            .membership
            .first()
            .ok_or_else(invalid)?
            .iter()
            .all(|value| !value)
        {
            return Ok(empty);
        }
        let region = cas.checked(region)?;
        let predicate = cas.checked(predicate)?;
        let truth = crate::case_syntax::guarded(cas, region, predicate, budget)?;
        roots.push(truth);
    }
    let boundary = match statement {
        Statement::Incidence { boundary, .. } => Some(crate::region::boundary_point(
            &cas.state,
            cas.checked(boundary)?,
            budget,
        )?),
        _ => None,
    };
    crate::region::partition_at(&cas.state, &roots, boundary.as_ref(), budget)
}

/// Bound untrusted rows and exact numbers before comparison or copying.
pub(crate) fn measure(value: &RegionCertificate, budget: &mut Budget) -> Result<(), Fault> {
    let cells = size(value.cuts.len(), AT)?
        .checked_mul(2)
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| Fault::capacity(BudgetKind::RegionCells, AT))?;
    budget.specialized(BudgetKind::RegionCells, cells, false, AT)?;
    budget.check(
        BudgetKind::RegionNodes,
        size(value.membership.len(), AT)?,
        AT,
    )?;
    let bytes = cells
        .checked_mul(size(value.membership.len(), AT)?)
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(bytes, AT)?;
    for row in &value.membership {
        if size(row.len(), AT)? != cells {
            return Err(invalid());
        }
    }
    for cut in &value.cuts {
        cut.check(budget, AT)?;
    }
    Ok(())
}

/// Check only defining set identities; an undefined false branch is never a complement.
pub(crate) fn construct(
    cas: &Cas,
    statement: &Statement,
    certificate: &Certificate,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if *certificate != Certificate::None {
        return Err(invalid());
    }
    budget.condition(1, AT)?;
    match statement {
        Statement::Constructor {
            region,
            law: ConstructorLaw::Membership,
        } => {
            cas.state.region(cas.checked(region)?)?;
            Ok(())
        }
        Statement::Subset { subset, superset } => {
            let subset = cas.checked(subset)?;
            let superset = cas.checked(superset)?;
            let smaller = cas.state.region(subset)?;
            let larger = cas.state.region(superset)?;
            if smaller.ambient != larger.ambient || smaller.scope != larger.scope {
                return Err(invalid());
            }
            let valid = subset == superset
                || matches!(smaller.expression, RegionExpr::Empty)
                || matches!(larger.expression, RegionExpr::Ambient)
                || matches!(&smaller.expression, RegionExpr::Meet(members) if
                    members.contains(&superset))
                || matches!(&larger.expression, RegionExpr::Union(members) if
                    members.contains(&subset))
                || matches!(smaller.expression, RegionExpr::GuardTrue { base, .. }
                    | RegionExpr::GuardFalse { base, .. } | RegionExpr::Difference { base, .. } if
                        base == superset);
            if valid { Ok(()) } else { Err(invalid()) }
        }
        Statement::Empty { region } => {
            let expression = &cas.state.region(cas.checked(region)?)?.expression;
            if matches!(expression, RegionExpr::Empty)
                || matches!(expression, RegionExpr::Union(members) if members.is_empty())
            {
                Ok(())
            } else {
                Err(invalid())
            }
        }
        Statement::Cover { domain, members } => {
            let domain = cas.state.region(cas.checked(domain)?)?;
            let RegionExpr::Union(expected) = &domain.expression else {
                return Err(invalid());
            };
            let mut actual = Vec::new();
            actual
                .try_reserve_exact(members.len())
                .map_err(|_| Fault::allocation(AT))?;
            for member in members {
                actual.push(cas.checked(member)?);
            }
            actual.sort_unstable();
            actual.dedup();
            if actual == *expected {
                Ok(())
            } else {
                Err(invalid())
            }
        }
        Statement::Disjoint { left, right } => {
            let first = &cas.state.region(cas.checked(left)?)?.expression;
            let second = &cas.state.region(cas.checked(right)?)?.expression;
            if matches!(first, RegionExpr::Empty) || matches!(second, RegionExpr::Empty) {
                return Ok(());
            }
            let valid = match (first, second) {
                (
                    RegionExpr::GuardTrue { base, predicate },
                    RegionExpr::GuardFalse {
                        base: other,
                        predicate: guard,
                    },
                )
                | (
                    RegionExpr::GuardFalse { base, predicate },
                    RegionExpr::GuardTrue {
                        base: other,
                        predicate: guard,
                    },
                ) => base == other && predicate == guard,
                _ => false,
            };
            if valid { Ok(()) } else { Err(invalid()) }
        }
        _ => Err(invalid()),
    }
}

/// Establish a genuine nonempty one-sided neighborhood ending at the exact boundary.
fn incidence(
    cas: &Cas,
    statement: &Statement,
    value: &RegionCertificate,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let Statement::Incidence {
        region,
        boundary,
        side,
        neighborhood,
    } = statement
    else {
        return Err(invalid());
    };
    let region = cas.state.region(cas.checked(region)?)?;
    let boundary_id = cas.checked(boundary)?;
    let location = cas.state.boundary(boundary_id)?;
    if region.ambient != location.ambient || region.scope != location.scope {
        return Err(invalid());
    }
    crate::region::open_interval(&cas.state, cas.checked(neighborhood)?, budget)?;
    let point = crate::region::boundary_point(&cas.state, boundary_id, budget)?;
    let position = value
        .cuts
        .iter()
        .position(|cut| *cut == point)
        .ok_or_else(invalid)?;
    let point = position
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(invalid)?;
    let region = value.membership.first().ok_or_else(invalid)?;
    let near = value.membership.get(1).ok_or_else(invalid)?;
    if !near
        .iter()
        .zip(region)
        .all(|(near, region)| !near || *region)
    {
        return Ok(false);
    }
    Ok(match side {
        crate::domain_model::Side::Below => {
            near.iter().rposition(|value| *value) == point.checked_sub(1)
        }
        crate::domain_model::Side::Above => {
            near.iter().position(|value| *value) == point.checked_add(1)
        }
    })
}
