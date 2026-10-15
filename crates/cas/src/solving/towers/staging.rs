// SPDX-License-Identifier: AGPL-3.0-only
//! Atomic component syntax and parent-first rebuilding, with no proof authority.

use super::dense::Dense;
use super::materialize;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, Handle, Reference};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{AT, domain, invalid, vector};
use crate::solving::{
    CoefficientDomainData, CoefficientMapData, ComponentSplitData, ExtensionLevelData, MapMode,
    MapTarget, SolveDraft,
};

/// Admitted component syntax still requires independent product, square-free and selection replay.
#[derive(Debug)]
pub(crate) struct StagedSplit {
    /// Full two-component syntax.
    pub(crate) split: Handle,
    /// Ordered child algebras.
    pub(crate) children: [Handle; 2],
    /// Ordered proper restriction maps.
    pub(crate) maps: [Handle; 2],
    /// Independently replayable coprimality multiplier polynomials.
    pub(crate) bezout: [Handle; 2],
    /// Images of each explicitly requested dependent object, in the same order on each child.
    pub(crate) dependents: [Vec<Handle>; 2],
}

/// One branch's graph objects, always sharing all unchanged parent nodes.
#[derive(Debug)]
struct Child {
    /// Canonical proper factor over the unchanged parent.
    factor: Handle,
    /// Quotient by this exact factor.
    domain: Handle,
    /// Generator projection retaining the finite localization.
    map: Handle,
}

/// Build one child and its actual reduced generator projection in the private candidate.
fn child(
    cas: &mut Cas,
    source: &Handle,
    proposal: &super::SplitCertificate,
    factor: &Dense,
    region: &Handle,
    localization: &[Handle],
    budget: &mut Budget,
) -> Result<Child, Fault> {
    let SolveDraft::ExtensionLevel(extension) =
        crate::solving::graph::object(&cas.state, proposal.level.logical_id())?
    else {
        return Err(invalid());
    };
    let variables = materialize::fields(
        &crate::solving::coefficients::storage::polynomial(&cas.state, &extension.polynomial)?
            .variables,
        budget,
    )?;
    let generator = materialize::generator(&extension.generator, budget)?;
    let square_free = extension.square_free;
    let factor = materialize::polynomial(cas, factor, &variables, budget)?;
    let parent = crate::solving::coefficients::storage::polynomial(&cas.state, &factor)?
        .domain
        .clone();
    let level = materialize::admit(
        cas,
        SolveDraft::ExtensionLevel(ExtensionLevelData {
            parent: Reference::Existing(parent.clone()),
            polynomial: Reference::Existing(factor.clone()),
            generator,
            square_free,
        }),
        budget,
    )?;
    let domain = materialize::admit(
        cas,
        SolveDraft::CoefficientDomain(CoefficientDomainData::Quotient {
            level: Reference::Existing(level),
        }),
        budget,
    )?;
    let image = generator_image(cas, &parent, &domain, budget)?;
    let mut images = vector(1, budget)?;
    images.push(Reference::Existing(image));
    let mut denominators = vector(localization.len(), budget)?;
    for handle in localization {
        denominators.push(Reference::Existing(handle.clone()));
    }
    let map = materialize::admit(
        cas,
        SolveDraft::CoefficientMap(CoefficientMapData {
            source: Reference::Existing(source.clone()),
            target: MapTarget::CoefficientDomain {
                domain: Reference::Existing(domain.clone()),
            },
            mode: MapMode::Specialization,
            region: Reference::Existing(region.clone()),
            images,
            denominators,
        }),
        budget,
    )?;
    Ok(Child {
        factor,
        domain,
        map,
    })
}

/// Reduce X even for a linear child, whose generator is a parent constant.
fn generator_image(
    cas: &mut Cas,
    parent: &Handle,
    target: &Handle,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let engine = Engine {
        state: &cas.state,
        domain: parent,
    };
    let mut coefficients = vector(2, budget)?;
    coefficients.push(
        engine
            .constant(false, budget)
            .map_err(|error| fault(&error))?,
    );
    coefficients.push(
        engine
            .constant(true, budget)
            .map_err(|error| fault(&error))?,
    );
    let polynomial = Dense::new(&engine, coefficients, budget).map_err(|error| fault(&error))?;
    let child = Engine {
        state: &cas.state,
        domain: target,
    };
    let generator =
        super::arithmetic::reduced(&child, polynomial, budget).map_err(|error| fault(&error))?;
    materialize::value(cas, &generator, budget)
}

/// Rebuild all dependent objects under one projection before exposing any of their handles.
fn dependents(
    cas: &mut Cas,
    source: &Handle,
    target: &Handle,
    handles: &[Handle],
    budget: &mut Budget,
) -> Result<Vec<Handle>, Fault> {
    let mut builder = super::rebuild::Rebuilder::new(source, target, budget)?;
    let mut result = vector(handles.len(), budget)?;
    for handle in handles {
        result.push(builder.object(cas, handle, budget)?);
    }
    Ok(result)
}

/// Materialize both factors and all requested higher coefficient objects in one atomic transaction.
pub(crate) fn stage(
    cas: &mut Cas,
    source: &Handle,
    proposal: &super::SplitCertificate,
    handles: &[Handle],
    region: &Handle,
    localization: &[Handle],
    budget: &mut Budget,
) -> Result<StagedSplit, Fault> {
    budget.solving(BudgetKind::ComponentSplits, 1, AT)?;
    budget.solving(BudgetKind::SolutionBranches, 2, AT)?;
    budget.solving(
        BudgetKind::DenominatorFactors,
        crate::graph::size(localization.len(), AT)?,
        AT,
    )?;
    cas.checked(source)?;
    cas.checked(region)?;
    if !matches!(domain(&cas.state, source)?, CoefficientDomainData::Quotient { level } if *level == proposal.level)
    {
        return Err(invalid());
    }
    let mut candidate = cas.fork(budget)?;
    let left = child(
        &mut candidate,
        source,
        proposal,
        &proposal.left,
        region,
        localization,
        budget,
    )?;
    let right = child(
        &mut candidate,
        source,
        proposal,
        &proposal.right,
        region,
        localization,
        budget,
    )?;
    let result = finish(
        &mut candidate,
        source,
        proposal,
        handles,
        region,
        [left, right],
        budget,
    )?;
    cas.state = candidate.state;
    Ok(result)
}

/// Package only graph syntax; certificates and selected-root transport remain explicitly untrusted.
fn finish(
    cas: &mut Cas,
    source: &Handle,
    proposal: &super::SplitCertificate,
    handles: &[Handle],
    region: &Handle,
    children: [Child; 2],
    budget: &mut Budget,
) -> Result<StagedSplit, Fault> {
    let [left, right] = children;
    let split = materialize::admit(
        cas,
        SolveDraft::ComponentSplit(ComponentSplitData {
            parent: Reference::Existing(proposal.level.clone()),
            left_factor: Reference::Existing(left.factor),
            right_factor: Reference::Existing(right.factor),
            left_domain: Reference::Existing(left.domain.clone()),
            right_domain: Reference::Existing(right.domain.clone()),
            left_map: Reference::Existing(left.map.clone()),
            right_map: Reference::Existing(right.map.clone()),
            region: Reference::Existing(region.clone()),
        }),
        budget,
    )?;
    let SolveDraft::ExtensionLevel(extension) =
        crate::solving::graph::object(&cas.state, proposal.level.logical_id())?
    else {
        return Err(invalid());
    };
    let variables = materialize::fields(
        &crate::solving::coefficients::storage::polynomial(&cas.state, &extension.polynomial)?
            .variables,
        budget,
    )?;
    let first = materialize::polynomial(cas, &proposal.first, &variables, budget)?;
    let second = materialize::polynomial(cas, &proposal.second, &variables, budget)?;
    let first_images = dependents(cas, source, &left.domain, handles, budget)?;
    let second_images = dependents(cas, source, &right.domain, handles, budget)?;
    Ok(StagedSplit {
        split,
        children: [left.domain, right.domain],
        maps: [left.map, right.map],
        bezout: [first, second],
        dependents: [first_images, second_images],
    })
}
