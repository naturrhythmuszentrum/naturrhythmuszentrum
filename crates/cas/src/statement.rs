// SPDX-License-Identifier: AGPL-3.0-only
//! Typed statement references and exact local condition keys.

use std::collections::BTreeMap;

use crate::budget::{Budget, BudgetKind};
use crate::domain_model::{HypothesisRef, Layer};
use crate::encoding::{self, Encoder};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Edge, Handle, encode_id, size};
use crate::proof_model::{
    Certificate, CheckedApplication, GuardedObligation, Judgment, ScopedConditions, Statement,
};

/// Scoped key and validation location.
const AT: Location = Location::new("typed statement");

/// Encode one already validated local mathematical reference.
pub(crate) fn handle(value: &Handle, out: &mut Encoder<'_>) -> Result<(), Fault> {
    encode_id(value.logical_id(), out)
}

/// Encode the complete guarded tuple; no component may be dropped during union.
pub(crate) fn guard(
    value: &GuardedObligation,
    out: &mut Encoder<'_>,
    mut write: impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    write(&value.home, out)?;
    encoding::scope(&value.scope, out)?;
    out.byte(value.layer.tag())?;
    out.byte(u8::from(value.space.is_some()))?;
    if let Some(space) = &value.space {
        write(space, out)?;
    }
    write(&value.region, out)?;
    write(&value.predicate, out)
}

/// Validate every reference in a guarded condition and its exact interpretation.
pub(crate) fn validate_guard(
    cas: &Cas,
    value: &GuardedObligation,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let home = cas.state.context(cas.checked(&value.home)?)?;
    let region = cas.state.region(cas.checked(&value.region)?)?;
    let (header, scope) =
        crate::domain_admission::predicate_context(&cas.state, cas.checked(&value.predicate)?)?;
    if scope != &value.scope
        || (!region.scope.is_empty() && region.scope != value.scope)
        || !crate::header::embeds(&cas.state, header, home.header_id, budget)?
        || !crate::header::embeds(&cas.state, region.header, home.header_id, budget)?
    {
        return Err(Fault::invalid(InvalidCode::Context, AT));
    }
    if let Some(space) = &value.space {
        cas.state.space(cas.checked(space)?)?;
    }
    if value.layer == Layer::Distribution && value.space.is_none() {
        return Err(Fault::invalid(InvalidCode::Sort, AT));
    }
    Ok(())
}

/// Canonical whole-tuple set union with an explicit comparison/storage preflight.
pub(crate) fn union(
    cas: &Cas,
    values: &[GuardedObligation],
    budget: &mut Budget,
) -> Result<Vec<GuardedObligation>, Fault> {
    budget.check(BudgetKind::ConditionAtoms, size(values.len(), AT)?, AT)?;
    let mut ordered = BTreeMap::new();
    let mut retained = 0_u64;
    for value in values {
        validate_guard(cas, value, budget)?;
        let mut out = Encoder::new(budget, AT);
        guard(value, &mut out, handle)?;
        let key = out.finish();
        let bytes = size(key.len(), AT)?;
        retained = retained
            .checked_add(bytes)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, retained, AT)?;
        budget.condition(
            bytes
                .checked_mul(
                    size(ordered.len(), AT)?
                        .checked_add(1)
                        .ok_or_else(|| Fault::capacity(BudgetKind::ConditionSteps, AT))?,
                )
                .ok_or_else(|| Fault::capacity(BudgetKind::ConditionSteps, AT))?,
            AT,
        )?;
        ordered.entry(key).or_insert_with(|| value.clone());
    }
    Ok(ordered.into_values().collect())
}

/// Preserve requirement/residual/hypothesis collections independently during composition.
pub(crate) fn merge(
    cas: &Cas,
    target: &mut ScopedConditions,
    source: &ScopedConditions,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for (destination, values) in [
        (&mut target.requirements, &source.requirements),
        (&mut target.residuals, &source.residuals),
    ] {
        destination
            .try_reserve(values.len())
            .map_err(|_| Fault::allocation(AT))?;
        destination.extend_from_slice(values);
        *destination = union(cas, destination, budget)?;
    }
    target
        .hypotheses
        .try_reserve(source.hypotheses.len())
        .map_err(|_| Fault::allocation(AT))?;
    target.hypotheses.extend_from_slice(&source.hypotheses);
    canonical_hypotheses(cas, &mut target.hypotheses, budget)
}

/// Sort exact qualified frame/slot references without collapsing names in sibling frames.
pub(crate) fn canonical_hypotheses(
    cas: &Cas,
    values: &mut Vec<HypothesisRef>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let count = size(values.len(), AT)?;
    budget.check(BudgetKind::ConditionAtoms, count, AT)?;
    budget.condition(
        count
            .checked_mul(count)
            .ok_or_else(|| Fault::capacity(BudgetKind::ConditionSteps, AT))?,
        AT,
    )?;
    for value in values.iter() {
        cas.state.context(cas.checked(&value.context)?)?;
    }
    values.sort_by_key(|value| (value.context.logical_id(), value.slot));
    values.dedup();
    Ok(())
}

/// Encode complete local judgment identity, including canonical residuals.
pub(crate) fn judgment(value: &Judgment, out: &mut Encoder<'_>) -> Result<(), Fault> {
    handle(&value.context, out)?;
    out.byte(value.layer.tag())?;
    out.byte(u8::from(value.space.is_some()))?;
    if let Some(space) = &value.space {
        handle(space, out)?;
    }
    encode(&value.statement, out, handle)?;
    out.count(value.residuals.len())?;
    for residual in &value.residuals {
        guard(residual, out, handle)?;
    }
    Ok(())
}

/// Encode a graph-owned application with every semantic and proof dependency retained.
pub(crate) fn application(value: &CheckedApplication, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.name(value.rule.specification().key)?;
    out.unsigned(u64::from(value.rule.specification().version))?;
    judgment(&value.judgment, out)?;
    out.count(value.premises.len())?;
    for premise in &value.premises {
        encode_id(*premise, out)?;
    }
    out.count(value.substitutions.len())?;
    for substitution in &value.substitutions {
        out.name(&substitution.key)?;
        handle(&substitution.replacement, out)?;
    }
    certificate(&value.certificate, out, handle)?;
    out.count(value.conditions.requirements.len())?;
    for required in &value.conditions.requirements {
        guard(required, out, handle)?;
    }
    out.count(value.conditions.hypotheses.len())?;
    for hypothesis in &value.conditions.hypotheses {
        handle(&hypothesis.context, out)?;
        out.unsigned(u64::from(hypothesis.slot))?;
    }
    Ok(())
}

/// Encode a closed auxiliary payload without interpreting it as proof.
pub(crate) fn certificate(
    value: &Certificate,
    out: &mut Encoder<'_>,
    mut write: impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    match value {
        Certificate::None => out.byte(0),
        Certificate::Region(value) => {
            out.byte(1)?;
            out.count(value.cuts.len())?;
            for cut in &value.cuts {
                out.number(cut)?;
            }
            out.count(value.membership.len())?;
            for row in &value.membership {
                out.count(row.len())?;
                for truth in row {
                    out.byte(u8::from(*truth))?;
                }
            }
            Ok(())
        }
        Certificate::Hypothesis(value) => {
            out.byte(2)?;
            write(&value.context, out)?;
            out.unsigned(u64::from(value.slot))
        }
        Certificate::Close { frame } => {
            out.byte(3)?;
            write(frame, out)
        }
        Certificate::Congruence(path) => {
            out.byte(4)?;
            out.count(path.len())?;
            for position in path {
                out.unsigned(u64::from(*position))?;
            }
            Ok(())
        }
        Certificate::SquareFree(value) => {
            out.byte(5)?;
            value.encode(out)
        }
        Certificate::Cases(value) => {
            out.byte(6)?;
            for family in [&value.regions, &value.arms, &value.frames] {
                out.count(family.len())?;
                for handle in family {
                    write(handle, out)?;
                }
            }
            out.count(value.cells.len())?;
            for cells in &value.cells {
                case_cells(cells, out)?;
            }
            Ok(())
        }
    }
}

/// Encode exact case partitions without copying caller-owned rational components.
fn case_cells(
    value: &crate::region::RegionCertificate,
    out: &mut Encoder<'_>,
) -> Result<(), Fault> {
    out.count(value.cuts.len())?;
    for cut in &value.cuts {
        out.number(cut)?;
    }
    out.count(value.membership.len())?;
    for row in &value.membership {
        out.count(row.len())?;
        for truth in row {
            out.byte(u8::from(*truth))?;
        }
    }
    Ok(())
}

/// Encode a typed proposition using caller-selected local or portable references.
pub(crate) fn encode(
    value: &Statement,
    out: &mut Encoder<'_>,
    mut write: impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    match value {
        Statement::Equality {
            left,
            right,
            domain,
            domain_contract,
        } => {
            out.byte(0)?;
            write(left, out)?;
            write(right, out)?;
            write(domain, out)?;
            out.byte(match domain_contract {
                crate::proof_model::DomainContract::DefinedAndEqual => 0,
                crate::proof_model::DomainContract::SameDefinedDomain => 1,
            })?;
        }
        Statement::PredicateFact { region, predicate } => {
            out.byte(1)?;
            write(region, out)?;
            write(predicate, out)?;
        }
        Statement::Subset { subset, superset } => {
            out.byte(2)?;
            write(subset, out)?;
            write(superset, out)?;
        }
        Statement::Disjoint { left, right } => {
            out.byte(3)?;
            write(left, out)?;
            write(right, out)?;
        }
        Statement::Cover { domain, members } => {
            out.byte(4)?;
            write(domain, out)?;
            out.count(members.len())?;
            for item in members {
                write(item, out)?;
            }
        }
        Statement::Empty { region } => {
            out.byte(5)?;
            write(region, out)?;
        }
        Statement::Constructor { region, law } => {
            out.byte(6)?;
            write(region, out)?;
            out.byte(match law {
                crate::proof_model::ConstructorLaw::Membership => 0,
            })?;
        }
        Statement::DefinedDomain { function, domain } => {
            out.byte(7)?;
            write(function, out)?;
            write(domain, out)?;
        }
        Statement::Compatibility {
            left,
            right,
            region,
        } => {
            out.byte(8)?;
            write(left, out)?;
            write(right, out)?;
            write(region, out)?;
        }
        Statement::Elaboration {
            source,
            result,
            domain,
        } => {
            out.byte(9)?;
            write(source, out)?;
            write(result, out)?;
            write(domain, out)?;
        }
        _ => return encode_values(value, out, write),
    }
    Ok(())
}

/// Encode scalar derivative, Jacobian and multiplicity propositions in their fixed field order.
fn encode_values(
    value: &Statement,
    out: &mut Encoder<'_>,
    mut write: impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    match value {
        Statement::DerivativeOn {
            function,
            derivative,
            coordinate,
            domain,
        } => {
            out.byte(12)?;
            write(function, out)?;
            write(derivative, out)?;
            out.name(coordinate)?;
            write(domain, out)?;
        }
        Statement::Jacobian {
            input,
            output,
            variables,
            domain,
        } => {
            out.byte(13)?;
            write(input, out)?;
            write(output, out)?;
            out.count(variables.len())?;
            for item in variables {
                out.name(item)?;
            }
            write(domain, out)?;
        }
        Statement::SquareFree {
            input,
            output,
            variable,
            field,
            domain,
        } => {
            out.byte(14)?;
            write(input, out)?;
            write(output, out)?;
            out.name(variable)?;
            encoding::field(field, out)?;
            write(domain, out)?;
        }
        _ => return encode_boundary(value, out, write),
    }
    Ok(())
}

/// Encode boundary, ownership and jet propositions without changing their wire tags or fields.
fn encode_boundary(
    value: &Statement,
    out: &mut Encoder<'_>,
    mut write: impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    match value {
        Statement::Incidence {
            region,
            boundary,
            side,
            neighborhood,
        } => {
            out.byte(10)?;
            write(region, out)?;
            write(boundary, out)?;
            out.byte(match side {
                crate::domain_model::Side::Below => 0,
                crate::domain_model::Side::Above => 1,
            })?;
            write(neighborhood, out)?;
        }
        Statement::Ownership {
            function,
            boundary,
            value,
        } => {
            out.byte(11)?;
            write(function, out)?;
            write(boundary, out)?;
            match value {
                crate::proof_model::Ownership::Present(item) => {
                    out.byte(0)?;
                    write(item, out)?;
                }
                crate::proof_model::Ownership::Absent => out.byte(1)?,
            }
        }
        Statement::Jet {
            function,
            region,
            boundary,
            side,
            values,
        } => {
            out.byte(15)?;
            write(function, out)?;
            write(region, out)?;
            write(boundary, out)?;
            out.byte(match side {
                crate::domain_model::Side::Below => 0,
                crate::domain_model::Side::Above => 1,
            })?;
            out.count(values.len())?;
            for item in values {
                write(item, out)?;
            }
        }
        Statement::Jump {
            function,
            boundary,
            order,
            value,
        } => {
            out.byte(16)?;
            write(function, out)?;
            write(boundary, out)?;
            out.unsigned(u64::from(*order))?;
            write(value, out)?;
        }
        _ => return Err(Fault::invalid(InvalidCode::Rule, AT)),
    }
    Ok(())
}

/// Enumerate every mathematical object referenced by a typed statement.
pub(crate) fn objects(value: &Statement) -> Vec<&Handle> {
    match value {
        Statement::Equality { .. }
        | Statement::DerivativeOn { .. }
        | Statement::Jacobian { .. }
        | Statement::SquareFree { .. } => scalar_objects(value),
        Statement::PredicateFact { .. }
        | Statement::Subset { .. }
        | Statement::Disjoint { .. }
        | Statement::Cover { .. }
        | Statement::Empty { .. }
        | Statement::Constructor { .. }
        | Statement::DefinedDomain { .. }
        | Statement::Compatibility { .. }
        | Statement::Elaboration { .. } => region_objects(value),
        Statement::Incidence { .. }
        | Statement::Ownership { .. }
        | Statement::Jet { .. }
        | Statement::Jump { .. } => boundary_objects(value),
    }
}

/// List region and case references in their original semantic order.
/// Other statement groups use the exhaustive dispatcher.
fn region_objects(value: &Statement) -> Vec<&Handle> {
    match value {
        Statement::PredicateFact {
            region, predicate, ..
        } => {
            vec![region, predicate]
        }
        Statement::Subset {
            subset, superset, ..
        } => {
            vec![subset, superset]
        }
        Statement::Disjoint { left, right, .. } => {
            vec![left, right]
        }
        Statement::Cover {
            domain, members, ..
        } => {
            let mut result = vec![domain];
            result.extend(members);
            result
        }
        Statement::Empty { region, .. } | Statement::Constructor { region, .. } => {
            vec![region]
        }
        Statement::DefinedDomain {
            function, domain, ..
        } => {
            vec![function, domain]
        }
        Statement::Compatibility {
            left,
            right,
            region,
            ..
        } => {
            vec![left, right, region]
        }
        Statement::Elaboration {
            source,
            result,
            domain,
            ..
        } => {
            vec![source, result, domain]
        }
        _ => objects(value),
    }
}

/// List scalar endpoints followed by their exact claimed domain.
/// Other statement groups use the exhaustive dispatcher.
fn scalar_objects(value: &Statement) -> Vec<&Handle> {
    match value {
        Statement::Equality {
            left,
            right,
            domain,
            ..
        } => {
            vec![left, right, domain]
        }
        Statement::DerivativeOn {
            function,
            derivative,
            domain,
            ..
        } => {
            vec![function, derivative, domain]
        }
        Statement::Jacobian {
            input,
            output,
            domain,
            ..
        }
        | Statement::SquareFree {
            input,
            output,
            domain,
            ..
        } => {
            vec![input, output, domain]
        }
        _ => objects(value),
    }
}

/// List boundary references and ordered jet values without omitting optional ownership.
/// Other statement groups use the exhaustive dispatcher.
fn boundary_objects(value: &Statement) -> Vec<&Handle> {
    match value {
        Statement::Incidence {
            region,
            boundary,
            neighborhood,
            ..
        } => {
            vec![region, boundary, neighborhood]
        }
        Statement::Ownership {
            function,
            boundary,
            value,
            ..
        } => {
            let mut result = vec![function, boundary];
            if let crate::proof_model::Ownership::Present(item) = value {
                result.push(item);
            }
            result
        }
        Statement::Jet {
            function,
            region,
            boundary,
            values,
            ..
        } => {
            let mut result = vec![function, region, boundary];
            result.extend(values);
            result
        }
        Statement::Jump {
            function,
            boundary,
            value,
            ..
        } => {
            vec![function, boundary, value]
        }
        _ => objects(value),
    }
}

/// Reconstruct semantic roles from positions in a validated statement's object sequence.
pub(crate) const fn edge_role(value: &Statement, position: u32) -> Edge {
    match (value, position) {
        (Statement::Subset { .. } | Statement::Disjoint { .. }, _) => {
            Edge::ComparedRegion(position)
        }
        (
            Statement::Cover { .. }
            | Statement::Empty { .. }
            | Statement::Constructor { .. }
            | Statement::PredicateFact { .. },
            0,
        )
        | (Statement::DefinedDomain { .. }, 1)
        | (
            Statement::Equality { .. }
            | Statement::Compatibility { .. }
            | Statement::Elaboration { .. }
            | Statement::DerivativeOn { .. }
            | Statement::Jacobian { .. }
            | Statement::SquareFree { .. },
            2,
        ) => Edge::ArgumentRegion(0),
        (Statement::Cover { .. }, _) => Edge::CoverMember(position.saturating_sub(1)),
        (Statement::Incidence { .. }, 0) => Edge::IncidentRegion,
        (Statement::Incidence { .. } | Statement::Ownership { .. } | Statement::Jump { .. }, 1)
        | (Statement::Jet { .. }, 2) => Edge::AtBoundary,
        (Statement::Incidence { side, .. }, 2) | (Statement::Jet { side, .. }, 1) => {
            Edge::SideRegion(*side)
        }
        (
            Statement::Ownership { .. }
            | Statement::DefinedDomain { .. }
            | Statement::Jet { .. }
            | Statement::Jump { .. }
            | Statement::DerivativeOn { .. },
            0,
        ) => Edge::OfFunction,
        (Statement::Jet { .. }, _) => Edge::JetValue(position.saturating_sub(3)),
        _ => Edge::ConclusionObject(position),
    }
}

/// Mathematical layer implied by a typed statement.
pub(crate) const fn layer(value: &Statement) -> Layer {
    match value {
        Statement::PredicateFact { .. }
        | Statement::Subset { .. }
        | Statement::Disjoint { .. }
        | Statement::Cover { .. }
        | Statement::Empty { .. }
        | Statement::Constructor { .. }
        | Statement::Incidence { .. } => Layer::Logic,
        _ => Layer::Pointwise,
    }
}
