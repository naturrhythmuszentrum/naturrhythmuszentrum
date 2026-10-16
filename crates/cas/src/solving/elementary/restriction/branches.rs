// SPDX-License-Identifier: AGPL-3.0-only
//! Each retained or rejected branch is checked on its own paired coefficient stratum.

use super::syntax::Correspondence;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::{AtomicSort, Field, Sort};
use crate::proof_model::Statement;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::storage::{reserve, vector};
use crate::solving::elementary::evaluate::Evaluation;
use crate::solving::elementary::proof::{AT, invalid, premise, tuple};
use crate::solving::{
    CertificateRealRestriction, SolutionBranchData, SolutionBranchKind, SolutionSetData, SolveDraft,
};

/// Borrow a branch without assigning its syntax any completeness authority.
fn branch<'a>(cas: &'a Cas, handle: &Handle) -> Result<&'a SolutionBranchData<Handle>, Fault> {
    match crate::solving::graph::object(&cas.state, cas.checked(handle)?)? {
        SolveDraft::SolutionBranch(value) => Ok(value),
        _ => Err(invalid()),
    }
}

/// A zero-polynomial family is restricted only when it is the full identity family.
fn identity(
    comparison: &Correspondence<'_>,
    handle: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let family = crate::solving::elementary::proof::family(comparison.cas, handle)?;
    let [entry] = tuple(comparison.cas, &family.body)?.entries.as_slice() else {
        return Err(invalid());
    };
    let product = crate::solving::elementary::proof::coordinates(comparison.cas, &family.family)?;
    if product.coordinates.len() != 1
        || !matches!(
            comparison
                .cas
                .state
                .region(family.region.logical_id())?
                .expression,
            crate::domain_model::RegionExpr::Ambient
        )
    {
        return Err(invalid());
    }
    let engine = Engine {
        state: &comparison.cas.state,
        domain: &comparison.real.coefficient_domain,
    };
    let variable = crate::solving::elementary::value::Value::variable(
        crate::solving::coefficients::forms::Variable::Slot(u32::MAX),
        &engine,
        budget,
    )?;
    let evaluation = Evaluation {
        cas: comparison.cas,
        engine,
        region: &comparison
            .transport
            .regions
            .first()
            .ok_or_else(invalid)?
            .real,
        premises: comparison.premises,
        free: &[],
        bound: std::slice::from_ref(&variable),
    };
    let actual = evaluation.expression(entry.logical_id(), true, budget)?;
    if !actual.equals(&variable, &evaluation.engine, budget)? {
        return Err(invalid());
    }
    Ok(())
}

/// An explicit realness premise must classify this exact expression on this exact region.
fn real_fact(
    comparison: &Correspondence<'_>,
    certificate: &CertificateRealRestriction,
    entry: &Handle,
    region: &Handle,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    for slot in &certificate.classification {
        budget.work(1, AT)?;
        if let Statement::Solve(statement) =
            &premise(comparison.premises, *slot)?.judgment.statement
            && let crate::solving::SolveStatement::RealValues(value) = statement.as_ref()
            && &value.region == region
            && value.values.contains(entry)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Classification does not borrow a neighboring branch's region or root choices.
fn classify(
    comparison: &Correspondence<'_>,
    certificate: &CertificateRealRestriction,
    source: &SolutionBranchData<Handle>,
    retained: bool,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let tuples = match &source.kind {
        SolutionBranchKind::Finite { tuples } => tuples,
        SolutionBranchKind::Empty => return Ok(()),
        SolutionBranchKind::Image { .. } if retained => return Ok(()),
        _ => return Err(invalid()),
    };
    let evaluation = Evaluation {
        cas: comparison.cas,
        engine: Engine {
            state: &comparison.cas.state,
            domain: &comparison.complex.coefficient_domain,
        },
        region: &source.region,
        premises: comparison.premises,
        free: &[],
        bound: &[],
    };
    let mut parameters = vector(comparison.transport.coefficients.len(), budget)?;
    parameters.extend(
        comparison
            .transport
            .coefficients
            .iter()
            .map(|pair| pair.complex.clone()),
    );
    for handle in tuples {
        let [entry] = tuple(comparison.cas, handle)?.entries.as_slice() else {
            return Err(invalid());
        };
        if retained && real_fact(comparison, certificate, entry, &source.region, budget)? {
            continue;
        }
        let classification = crate::solving::elementary::complex::restricted(
            &evaluation,
            entry.logical_id(),
            &parameters,
            retained,
            &certificate.classification,
            budget,
        );
        if retained {
            classification?;
        } else if !crate::solving::elementary::product::decides(classification)? {
            crate::solving::elementary::nonreal::check(&evaluation, entry, budget)?;
        }
    }
    Ok(())
}

/// A checked scalar equality may change a retained expression's presentation on this exact stratum.
fn equivalent(
    comparison: &mut Correspondence<'_>,
    real: &Handle,
    complex: &Handle,
    region: &Handle,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    if comparison.attempt(real, complex, budget)? {
        return Ok(true);
    }
    for previous in comparison.premises {
        budget.work(1, AT)?;
        let endpoints = match &previous.judgment.statement {
            Statement::Solve(statement) => match statement.as_ref() {
                crate::solving::SolveStatement::CoefficientEquality(value)
                    if &value.region == region =>
                {
                    Some((&value.left, &value.right))
                }
                _ => None,
            },
            Statement::Equality {
                left,
                right,
                domain,
                domain_contract: crate::proof_model::DomainContract::DefinedAndEqual,
            } if domain == region => Some((left, right)),
            _ => None,
        };
        if let Some((left, right)) = endpoints {
            let target = if left == complex {
                right
            } else if right == complex {
                left
            } else {
                continue;
            };
            if comparison.attempt(real, target, budget)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Both inclusions preserve the same root correspondence across the entire stratum.
fn tuples(
    comparison: &mut Correspondence<'_>,
    real: &[Handle],
    complex: &[Handle],
    region: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for (left, right, reverse) in [(real, complex, false), (complex, real, true)] {
        for handle in left {
            let [entry] = tuple(comparison.cas, handle)?.entries.as_slice() else {
                return Err(invalid());
            };
            let mut found = false;
            for candidate in right {
                let [other] = tuple(comparison.cas, candidate)?.entries.as_slice() else {
                    return Err(invalid());
                };
                let (a, b) = if reverse {
                    (other, entry)
                } else {
                    (entry, other)
                };
                if comparison.cas.state.expression(a.logical_id())?.sort
                    != Sort::Value(AtomicSort::Scalar(Field::Real))
                {
                    return Err(invalid());
                }
                if equivalent(comparison, a, b, region, budget)? {
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

/// Whole-family transport is deliberately the identity subcase, preserving all real preimages.
fn family(
    comparison: &mut Correspondence<'_>,
    real: &Handle,
    complex: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    identity(comparison, real, budget)?;
    let left = crate::solving::elementary::proof::family(comparison.cas, real)?;
    let right = crate::solving::elementary::proof::family(comparison.cas, complex)?;
    if (
        &left.coefficients,
        &right.coefficients,
        &left.target,
        &right.target,
    ) != (
        &comparison.real.coefficients,
        &comparison.complex.coefficients,
        &comparison.real.unknowns,
        &comparison.complex.unknowns,
    ) {
        return Err(invalid());
    }
    comparison.check(&left.region, &right.region, budget)?;
    tuples(
        comparison,
        std::slice::from_ref(&left.body),
        std::slice::from_ref(&right.body),
        &right.region,
        budget,
    )
}

/// Compare all retained values in one region; no target member is silently widened.
fn stratum(
    comparison: &mut Correspondence<'_>,
    certificate: &CertificateRealRestriction,
    target: &SolutionSetData<Handle>,
    pair: &crate::solving::RestrictionPair,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut real = vector(0, budget)?;
    let mut complex = vector(0, budget)?;
    let mut real_family = None;
    let mut complex_family = None;
    for (members, region, entries, image) in [
        (&target.branches, &pair.real, &mut real, &mut real_family),
        (
            &certificate.retained,
            &pair.complex,
            &mut complex,
            &mut complex_family,
        ),
    ] {
        for handle in members {
            let current = branch(comparison.cas, handle)?;
            if current.region != *region {
                continue;
            }
            match &current.kind {
                SolutionBranchKind::Finite { tuples } => {
                    reserve(entries, tuples.len(), budget)?;
                    entries.extend_from_slice(tuples);
                }
                SolutionBranchKind::Image { family } if image.is_none() => *image = Some(family),
                SolutionBranchKind::Empty => {}
                _ => return Err(invalid()),
            }
        }
    }
    match (real_family, complex_family) {
        (None, None) => tuples(comparison, &real, &complex, &pair.complex, budget),
        (Some(a), Some(b)) if real.is_empty() && complex.is_empty() => {
            family(comparison, a, b, budget)
        }
        _ => Err(invalid()),
    }
}

/// The complete source branch set is partitioned exactly once, and every target region is paired.
pub(crate) fn check(
    comparison: &mut Correspondence<'_>,
    certificate: &CertificateRealRestriction,
    source: &SolutionSetData<Handle>,
    target: &SolutionSetData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if certificate
        .retained
        .len()
        .checked_add(certificate.rejected.len())
        != Some(source.branches.len())
    {
        return Err(invalid());
    }
    for handle in &source.branches {
        budget.work(crate::graph::size(source.branches.len(), AT)?, AT)?;
        if certificate
            .retained
            .iter()
            .chain(&certificate.rejected)
            .filter(|candidate| *candidate == handle)
            .count()
            != 1
        {
            return Err(invalid());
        }
        let current = branch(comparison.cas, handle)?;
        if !comparison
            .transport
            .regions
            .iter()
            .any(|pair| pair.complex == current.region)
        {
            return Err(invalid());
        }
        classify(
            comparison,
            certificate,
            current,
            certificate.retained.contains(handle),
            budget,
        )?;
    }
    for handle in &target.branches {
        let current = branch(comparison.cas, handle)?;
        if (&current.problem, &current.domain) != (&target.problem, &comparison.real.domain)
            || !comparison
                .transport
                .regions
                .iter()
                .any(|pair| pair.real == current.region)
        {
            return Err(invalid());
        }
    }
    for pair in &comparison.transport.regions {
        stratum(comparison, certificate, target, pair, budget)?;
    }
    Ok(())
}
