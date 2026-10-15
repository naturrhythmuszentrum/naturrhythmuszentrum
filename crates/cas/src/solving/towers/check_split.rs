// SPDX-License-Identifier: AGPL-3.0-only
//! Independent Chinese-remainder products, coprimality and restriction-map checks.

use super::dense::Dense;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::proof_model::{ApplicationDraft, CheckedApplication, Statement};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::proof::{defined, fault};
use crate::solving::coefficients::storage::{AT, domain, invalid, vector};
use crate::solving::{
    CertificateComponentSplit, CoefficientDomainData, ComponentSplitData, MapMode, MapTarget,
    SolveDraft, SolveStatement,
};

/// Formal differentiation uses parent integer constants, never floating arithmetic.
pub(crate) fn derivative(
    cas: &Cas,
    polynomial: &Dense,
    budget: &mut Budget,
) -> Result<Dense, Fault> {
    let parent = Engine {
        state: &cas.state,
        domain: &polynomial.domain,
    };
    let mut coefficients = vector(polynomial.coefficients.len().saturating_sub(1), budget)?;
    for (index, coefficient) in polynomial.coefficients.iter().enumerate().skip(1) {
        let value = crate::number::Rational::from_ratio(
            i64::try_from(index).map_err(|_| invalid())?,
            1,
            budget,
        )?;
        coefficients.push(
            parent
                .mul(
                    coefficient,
                    &parent
                        .rational(&value, budget)
                        .map_err(|error| fault(&error))?,
                    budget,
                )
                .map_err(|error| fault(&error))?,
        );
    }
    Dense::new(&parent, coefficients, budget).map_err(|error| fault(&error))
}

/// A checked derivative unit supplies square-freeness in exactly the parent quotient.
fn square_free(
    cas: &Cas,
    extension: &Handle,
    modulus: &Dense,
    region: &Handle,
    premise: &CheckedApplication,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let Statement::Solve(statement) = &premise.judgment.statement else {
        return Ok(false);
    };
    let SolveStatement::Unit(unit) = statement.as_ref() else {
        return Ok(false);
    };
    if unit.region != *region
        || !matches!(domain(&cas.state, &unit.domain)?, CoefficientDomainData::Quotient { level } if level == extension)
    {
        return Ok(false);
    }
    let engine = Engine {
        state: &cas.state,
        domain: &unit.domain,
    };
    let element = engine
        .read(&unit.value, budget)
        .map_err(|error| fault(&error))?;
    let value =
        super::arithmetic::dense(&engine, &element, budget).map_err(|error| fault(&error))?;
    value
        .equals(&derivative(cas, modulus, budget)?, &cas.state, budget)
        .map_err(|error| fault(&error))
}

/// Reducedness is inductive: every earlier quotient requires its own derivative unit.
fn parent_reduced(
    cas: &Cas,
    parent: &Handle,
    region: &Handle,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut current = parent;
    while let CoefficientDomainData::Quotient { level } = domain(&cas.state, current)? {
        budget.work(1, AT)?;
        let SolveDraft::ExtensionLevel(extension) =
            crate::solving::graph::object(&cas.state, level.logical_id())?
        else {
            return Err(invalid());
        };
        let modulus = Dense::read(&cas.state, &extension.polynomial, budget)
            .map_err(|error| fault(&error))?;
        let mut found = false;
        for premise in premises {
            if square_free(cas, level, &modulus, region, premise, budget)? {
                found = true;
                break;
            }
        }
        if !found {
            return Err(invalid());
        }
        current = &extension.parent;
    }
    Ok(())
}

/// Both factors reconstruct m and their supplied coprimality multipliers reconstruct one.
fn products(
    cas: &Cas,
    split: &ComponentSplitData<Handle>,
    certificate: &CertificateComponentSplit,
    modulus: &Dense,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let left =
        Dense::read(&cas.state, &split.left_factor, budget).map_err(|error| fault(&error))?;
    let right =
        Dense::read(&cas.state, &split.right_factor, budget).map_err(|error| fault(&error))?;
    let product = left
        .mul(&right, &cas.state, budget)
        .map_err(|error| fault(&error))?;
    if !product
        .equals(modulus, &cas.state, budget)
        .map_err(|error| fault(&error))?
    {
        return Err(invalid());
    }
    let premise = premises
        .get(usize::try_from(certificate.product).map_err(|_| invalid())?)
        .ok_or_else(invalid)?;
    let Statement::Solve(statement) = &premise.judgment.statement else {
        return Err(invalid());
    };
    let SolveStatement::CoefficientEquality(equality) = statement.as_ref() else {
        return Err(invalid());
    };
    if equality.region != split.region {
        return Err(invalid());
    }
    for endpoint in [&equality.left, &equality.right] {
        if !Dense::read(&cas.state, endpoint, budget)
            .map_err(|error| fault(&error))?
            .equals(modulus, &cas.state, budget)
            .map_err(|error| fault(&error))?
        {
            return Err(invalid());
        }
    }
    let [first, second] = certificate.bezout.as_slice() else {
        return Err(invalid());
    };
    let first_product = left
        .mul(
            &Dense::read(&cas.state, first, budget).map_err(|error| fault(&error))?,
            &cas.state,
            budget,
        )
        .map_err(|error| fault(&error))?;
    let second_product = right
        .mul(
            &Dense::read(&cas.state, second, budget).map_err(|error| fault(&error))?,
            &cas.state,
            budget,
        )
        .map_err(|error| fault(&error))?;
    let sum = first_product
        .sum(&second_product, false, &cas.state, budget)
        .map_err(|error| fault(&error))?;
    let parent = Engine {
        state: &cas.state,
        domain: &modulus.domain,
    };
    if !sum
        .equals(
            &Dense::one(&parent, budget).map_err(|error| fault(&error))?,
            &cas.state,
            budget,
        )
        .map_err(|error| fault(&error))?
    {
        return Err(invalid());
    }
    Ok(())
}

/// The quotient generator must project to that child's own reduced generator.
fn restriction(
    cas: &Cas,
    handle: &Handle,
    child: &Handle,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let SolveDraft::CoefficientMap(map) =
        crate::solving::graph::object(&cas.state, handle.logical_id())?
    else {
        return Err(invalid());
    };
    if map.mode != MapMode::Specialization
        || map.target
            != (MapTarget::CoefficientDomain {
                domain: child.clone(),
            })
    {
        return Err(invalid());
    }
    for denominator in &map.denominators {
        crate::solving::coefficients::proof::denominator(
            cas,
            denominator,
            &map.source,
            &map.region,
            premises,
            budget,
        )?;
    }
    let [image] = map.images.as_slice() else {
        return Err(invalid());
    };
    let engine = Engine {
        state: &cas.state,
        domain: child,
    };
    let (_, extension) = super::arithmetic::level(&engine).map_err(|error| fault(&error))?;
    let parent = Engine {
        state: &cas.state,
        domain: &extension.parent,
    };
    let mut entries = vector(2, budget)?;
    entries.push(
        parent
            .constant(false, budget)
            .map_err(|error| fault(&error))?,
    );
    entries.push(
        parent
            .constant(true, budget)
            .map_err(|error| fault(&error))?,
    );
    let generator = super::arithmetic::reduced(
        &engine,
        Dense::new(&parent, entries, budget).map_err(|error| fault(&error))?,
        budget,
    )
    .map_err(|error| fault(&error))?;
    if !engine
        .formal_equal(
            &engine.read(image, budget).map_err(|error| fault(&error))?,
            &generator,
            budget,
        )
        .map_err(|error| fault(&error))?
    {
        return Err(invalid());
    }
    Ok(())
}

/// Replay the complete disjoint algebra/root cover before the shared transaction publishes it.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateComponentSplit,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.solving(BudgetKind::ComponentSplits, 1, AT)?;
    budget.solving(BudgetKind::SolutionBranches, 2, AT)?;
    let SolveDraft::ComponentSplit(split) =
        crate::solving::graph::object(&cas.state, cas.checked(&certificate.split)?)?
    else {
        return Err(invalid());
    };
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    let SolveStatement::ComponentCover(cover) = statement.as_ref() else {
        return Err(invalid());
    };
    if cover.split != certificate.split
        || cover.region != split.region
        || cas.state.context(cas.checked(&draft.context)?)?.admissible != split.region.logical_id()
    {
        return Err(invalid());
    }
    let SolveDraft::ExtensionLevel(extension) =
        crate::solving::graph::object(&cas.state, split.parent.logical_id())?
    else {
        return Err(invalid());
    };
    for handle in [
        &extension.polynomial,
        &split.left_factor,
        &split.right_factor,
    ]
    .into_iter()
    .chain(&certificate.bezout)
    {
        defined(cas, handle, &split.region, premises, budget)?;
    }
    let modulus =
        Dense::read(&cas.state, &extension.polynomial, budget).map_err(|error| fault(&error))?;
    let premise = premises
        .get(usize::try_from(certificate.square_free).map_err(|_| invalid())?)
        .ok_or_else(invalid)?;
    if !square_free(cas, &split.parent, &modulus, &split.region, premise, budget)? {
        return Err(invalid());
    }
    parent_reduced(cas, &extension.parent, &split.region, premises, budget)?;
    products(cas, split, certificate, &modulus, premises, budget)?;
    restriction(cas, &split.left_map, &split.left_domain, premises, budget)?;
    restriction(cas, &split.right_map, &split.right_domain, premises, budget)?;
    localizations(cas, split, budget)?;
    super::transport::check(cas, split, &certificate.selections, premises, budget)
}

/// Neither component may silently drop an obligation from their common finite localization.
fn localizations(
    cas: &Cas,
    split: &ComponentSplitData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let SolveDraft::CoefficientMap(left) =
        crate::solving::graph::object(&cas.state, split.left_map.logical_id())?
    else {
        return Err(invalid());
    };
    let SolveDraft::CoefficientMap(right) =
        crate::solving::graph::object(&cas.state, split.right_map.logical_id())?
    else {
        return Err(invalid());
    };
    budget.work(
        crate::graph::size(left.denominators.len().max(right.denominators.len()), AT)?,
        AT,
    )?;
    if left.denominators != right.denominators {
        return Err(invalid());
    }
    Ok(())
}
