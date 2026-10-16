// SPDX-License-Identifier: AGPL-3.0-only
//! The degree-zero identity family retains the full original coordinate product.

use super::construction::{Construction, admit};
use crate::budget::Budget;
use crate::domain_model::{AmbientDomain, RegionDraft, RegionExpr};
use crate::error::Fault;
use crate::graph::{Cas, Draft, Handle, Reference};
use crate::model::{AtomicSort, Field, Operator};
use crate::solving::coefficients::storage::invalid;
use crate::solving::towers::materialize;
use crate::solving::{
    CertificateParameterImageCoverage, Coordinate, CoordinateRole, CoordinateSpaceData,
    FamilyMapData, SolutionBranchData, SolutionBranchKind, SolutionSetData, SolveCertificate,
    SolveDraft, SolveSchema, SolveStatement, StatementCoverage, StatementFamilyMembership,
    TupleData,
};

/// One identity map binds its declared source product and returns its target product exactly.
fn identity(
    cas: &mut Cas,
    construction: &Construction,
    coefficients: &Handle,
    source: &Handle,
    target: &Handle,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let scope = vec![vec![AtomicSort::Scalar(Field::Complex)]];
    let region = admit(
        cas,
        &Draft::Region(RegionDraft {
            header: Reference::Existing(construction.header.clone()),
            scope: scope.clone(),
            ambient: AmbientDomain::Product {
                coordinates: source.clone(),
            },
            expression: RegionExpr::Ambient,
        }),
        budget,
    )?;
    let bound = construction.expression(
        cas,
        Operator::Bound { depth: 0, slot: 0 },
        &[],
        true,
        budget,
    )?;
    let body = materialize::admit(
        cas,
        SolveDraft::Tuple(TupleData {
            header: Reference::Existing(construction.header.clone()),
            scope,
            coordinates: Reference::Existing(target.clone()),
            entries: vec![Reference::Existing(bound)],
        }),
        budget,
    )?;
    materialize::admit(
        cas,
        SolveDraft::FamilyMap(FamilyMapData {
            header: Reference::Existing(construction.header.clone()),
            scope: Vec::new(),
            coefficients: Reference::Existing(coefficients.clone()),
            family: Reference::Existing(source.clone()),
            region: Reference::Existing(region),
            target: Reference::Existing(target.clone()),
            body: Reference::Existing(body),
        }),
        budget,
    )
}

/// The family parameter uses the declared unknown's semantic key with an explicit Family role.
fn maps(
    cas: &mut Cas,
    construction: &Construction,
    problem: &Handle,
    budget: &mut Budget,
) -> Result<(Handle, Handle), Fault> {
    let original = crate::solving::elementary::proof::problem(cas, problem)?;
    let unknowns = crate::solving::elementary::proof::coordinates(cas, &original.unknowns)?;
    let [coordinate] = unknowns.coordinates.as_slice() else {
        return Err(invalid());
    };
    budget.work(
        crate::graph::size(coordinate.key.len(), super::AT)?,
        super::AT,
    )?;
    let coordinate = Coordinate {
        key: coordinate.key.clone(),
        field: Field::Complex,
        role: CoordinateRole::Family,
    };
    let (coefficients, target) = (original.coefficients.clone(), original.unknowns.clone());
    let family = materialize::admit(
        cas,
        SolveDraft::CoordinateSpace(CoordinateSpaceData {
            header: Reference::Existing(construction.header.clone()),
            coordinates: vec![coordinate],
        }),
        budget,
    )?;
    let forward = identity(cas, construction, &coefficients, &family, &target, budget)?;
    let backward = identity(cas, construction, &coefficients, &target, &family, budget)?;
    Ok((forward, backward))
}

/// The proposed image retains the original solution domain; the checker proves both inclusions.
fn image(
    cas: &mut Cas,
    construction: &Construction,
    problem: &Handle,
    family: &Handle,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let domain = crate::solving::elementary::proof::problem(cas, problem)?
        .domain
        .clone();
    let branch = materialize::admit(
        cas,
        SolveDraft::SolutionBranch(SolutionBranchData {
            problem: Reference::Existing(problem.clone()),
            region: Reference::Existing(construction.region.clone()),
            domain: Reference::Existing(domain),
            kind: SolutionBranchKind::Image {
                family: Reference::Existing(family.clone()),
            },
        }),
        budget,
    )?;
    materialize::admit(
        cas,
        SolveDraft::SolutionSet(SolutionSetData {
            problem: Reference::Existing(problem.clone()),
            branches: vec![Reference::Existing(branch)],
        }),
        budget,
    )
}

/// A zero polynomial has a complete coordinate family, never an empty finite root list.
pub(crate) fn coverage(
    cas: &mut Cas,
    construction: &mut Construction,
    problem: &Handle,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let (family, recovery) = maps(cas, construction, problem, budget)?;
    let set = image(cas, construction, problem, &family, budget)?;
    let mut certificate = CertificateParameterImageCoverage {
        problem: problem.clone(),
        family: family.clone(),
        recovery: None,
        region: construction.region.clone(),
        membership: Vec::new(),
        recovery_proofs: Vec::new(),
        domain: Vec::new(),
        scope: Vec::new(),
    };
    let membership = construction.push(
        SolveSchema::ParameterImageCoverage,
        SolveStatement::FamilyMembership(StatementFamilyMembership {
            problem: problem.clone(),
            family,
            region: construction.region.clone(),
        }),
        SolveCertificate::ParameterImageCoverage(certificate.clone()),
        budget,
    )?;
    certificate.recovery = Some(recovery);
    certificate.membership = vec![membership];
    construction.push(
        SolveSchema::ParameterImageCoverage,
        SolveStatement::Coverage(StatementCoverage {
            problem: problem.clone(),
            set: set.clone(),
            region: construction.region.clone(),
        }),
        SolveCertificate::ParameterImageCoverage(certificate),
        budget,
    )?;
    Ok(set)
}
