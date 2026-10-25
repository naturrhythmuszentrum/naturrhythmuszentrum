// SPDX-License-Identifier: AGPL-3.0-only
//! Checked nominal membership and its retained introduction root.

use crate::budget::Budget;
use crate::decision::{CoordinateInterval, RationalEnclosure};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, LogicalId, State, Vertex};
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, DomainContract, ScopedConditions, Statement,
};

/// Exact certificate validation location.
const AT: Location = Location::new("decision introduction");

/// An introduction or use does not match its retained evidence.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::EndpointMismatch, AT)
}

/// Replay all prerequisites before returning a membership conclusion and source assumptions.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the native proof and wire dispatchers share this checker"
)]
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let Certificate::Decision(evidence) = &draft.certificate else {
        return Err(invalid());
    };
    let frame = cas.state.context(cas.checked(&draft.context)?)?;
    let function = cas
        .state
        .expression(cas.checked(&evidence.partition.function)?)?;
    let Vertex::CaseArm(arm) = cas.state.node(cas.checked(&evidence.cut.association)?)? else {
        return Err(invalid());
    };
    let expected = Statement::Subset {
        subset: cas.handle(frame.admissible),
        superset: cas.handle(arm.region),
    };
    let dimensions = evidence.cut.bounds.len();
    if draft.conclusion != expected
        || !draft.substitutions.is_empty()
        || premises.len() != dimensions
        || evidence.enclosures.len() != dimensions
    {
        return Err(invalid());
    }
    // The admitted partition proves this exact source region nonempty.
    if function.domain != Some(frame.admissible) {
        return Err(Fault::invalid(InvalidCode::UnsupportedCertificate, AT));
    }
    let partition = cas.check_case_partition(&evidence.partition, budget)?;
    cas.check_partition_cut(&partition, &evidence.cut, budget)?;
    let mut conditions = ScopedConditions::default();
    budget.with_retained_payload(partition.retained_bytes(), AT, |budget| {
        for ((bound, enclosure), premise) in evidence
            .cut
            .bounds
            .iter()
            .zip(&evidence.enclosures)
            .zip(premises)
        {
            budget.work(1, AT)?;
            coordinate(cas, draft, bound, enclosure, premise)?;
            cas.check_rational_enclosure(enclosure, budget)?;
            crate::statement::merge(cas, &mut conditions, &premise.conditions, budget)?;
        }
        Ok(())
    })?;
    Ok(conditions)
}

/// Each equality holds on the entire admitted family, not a smaller convenient region.
fn coordinate(
    cas: &Cas,
    draft: &ApplicationDraft,
    bound: &CoordinateInterval,
    enclosure: &RationalEnclosure,
    premise: &CheckedApplication,
) -> Result<(), Fault> {
    let family = cas.handle(cas.state.context(cas.checked(&draft.context)?)?.admissible);
    if enclosure.context != draft.context
        || enclosure.input_family != family
        || enclosure.lower != bound.lower
        || enclosure.upper != bound.upper
        || premise.judgment.context != draft.context
        || !premise.conditions.residuals.is_empty()
    {
        return Err(invalid());
    }
    match &premise.judgment.statement {
        Statement::Equality {
            left,
            right,
            domain,
            domain_contract: DomainContract::DefinedAndEqual,
        } if *left == bound.coordinate && *right == enclosure.target && *domain == family => Ok(()),
        Statement::PredicateFact { region, predicate } if *region == family => {
            let Vertex::Condition(predicate) = cas.state.node(cas.checked(predicate)?)? else {
                return Err(invalid());
            };
            if predicate.kind != crate::model::PredicateKind::Equal
                || predicate.arguments
                    != [
                        cas.checked(&bound.coordinate)?,
                        cas.checked(&enclosure.target)?,
                    ]
            {
                return Err(invalid());
            }
            Ok(())
        }
        _ => Err(invalid()),
    }
}

/// Only the complete checked introduction can authorize a certified frame.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the native proof and wire dispatchers share this checker"
)]
pub(crate) fn frame(
    state: &State,
    parent: LogicalId,
    region: LogicalId,
    introduction: LogicalId,
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(4, AT)?;
    let Vertex::Proof(application) = state.node(introduction)? else {
        return Err(invalid());
    };
    let Vertex::ScopedApplication(proof) = state.node(*application)? else {
        return Err(invalid());
    };
    let Certificate::Decision(_) = &proof.certificate else {
        return Err(invalid());
    };
    let Statement::Subset { subset, superset } = &proof.judgment.statement else {
        return Err(invalid());
    };
    if proof.rule != crate::rule::Primitive::RegionDecide
        || proof.judgment.context.logical_id() != parent
        || subset.logical_id() != state.context(parent)?.admissible
        || superset.logical_id() != region
        || !proof.conditions.residuals.is_empty()
    {
        return Err(invalid());
    }
    Ok(())
}

/// Retain the partition, option and every coordinate/target as actual graph dependencies.
#[expect(
    clippy::redundant_pub_crate,
    reason = "native publication shares the certificate dependency inventory"
)]
pub(crate) fn dependencies(
    cas: &Cas,
    value: &crate::decision::DecisionIntroduction,
    edges: &mut Vec<(LogicalId, crate::graph::Edge)>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let count = value
        .partition
        .coordinates
        .len()
        .saturating_add(value.enclosures.len())
        .saturating_add(2);
    let total = crate::graph::size(edges.len().saturating_add(count), AT)?.saturating_mul(32);
    budget.check(crate::budget::BudgetKind::StagedPayloadBytes, total, AT)?;
    budget.work(crate::graph::size(count, AT)?, AT)?;
    edges
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for object in [&value.partition.function, &value.cut.association]
        .into_iter()
        .chain(&value.partition.coordinates)
        .chain(value.enclosures.iter().map(|enclosure| &enclosure.target))
    {
        edges.push((cas.checked(object)?, crate::graph::Edge::DependsOn));
    }
    Ok(())
}
