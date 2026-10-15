// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit coefficient maps, coordinate identities and paired coefficient covers.

use super::syntax::Correspondence;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::Operator;
use crate::proof_model::{CheckedApplication, Statement};
use crate::solving::coefficients::model::Value;
use crate::solving::coefficients::proof::fault;
use crate::solving::elementary::proof::{AT, coordinates, invalid, premise};
use crate::solving::{CertificateRealRestriction, MapMode, MapTarget, RestrictionPair, SolveDraft};

/// Each row repeats the actual ordered declaration identities of both coordinate products.
fn declarations(
    cas: &Cas,
    real: &Handle,
    complex: &Handle,
    rows: &[RestrictionPair],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let left = coordinates(cas, real)?;
    let right = coordinates(cas, complex)?;
    if (rows.len(), rows.len()) != (left.coordinates.len(), right.coordinates.len()) {
        return Err(invalid());
    }
    for ((pair, a), b) in rows.iter().zip(&left.coordinates).zip(&right.coordinates) {
        budget.work(1, AT)?;
        let first = cas.state.expression(cas.checked(&pair.real)?)?;
        let second = cas.state.expression(cas.checked(&pair.complex)?)?;
        if (
            first.header,
            second.header,
            &first.operator,
            &second.operator,
            &first.scope,
            &second.scope,
        ) != (
            left.header.logical_id(),
            right.header.logical_id(),
            &Operator::Free(a.key.clone()),
            &Operator::Free(b.key.clone()),
            &Vec::new(),
            &Vec::new(),
        ) {
            return Err(invalid());
        }
    }
    Ok(())
}

/// An injective formal map must also send each parameter to the declared corresponding position.
fn images(
    cas: &Cas,
    map: &crate::solving::CoefficientMapData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let crate::solving::CoefficientDomainData::Fraction {
        coordinates: product,
        ..
    } = crate::solving::coefficients::storage::domain(&cas.state, &map.source)?
    else {
        return if map.images.is_empty() {
            Ok(())
        } else {
            Err(invalid())
        };
    };
    if map.source
        == match &map.target {
            MapTarget::CoefficientDomain { domain } => domain.clone(),
            MapTarget::ScalarAmbient { .. } => return Err(invalid()),
        }
        && map.images.is_empty()
    {
        return Ok(());
    }
    let count = coordinates(cas, product)?.coordinates.len();
    if map.images.len() != count {
        return Err(invalid());
    }
    for (index, handle) in map.images.iter().enumerate() {
        let MapTarget::CoefficientDomain { domain } = &map.target else {
            return Err(invalid());
        };
        let engine = crate::solving::coefficients::arithmetic::Engine {
            state: &cas.state,
            domain,
        };
        let stored = engine.read(handle, budget).map_err(|error| fault(&error))?;
        let Value::Fraction(value) = &stored.value else {
            return Err(invalid());
        };
        let [term] = value.numerator.terms.as_slice() else {
            return Err(invalid());
        };
        for (position, power) in term.powers.iter().enumerate() {
            budget.work(1, AT)?;
            if *power != u32::from(position == index) {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

/// Map validation is independent of equation comparison and rejects a false parameter permutation.
pub(crate) fn check(comparison: &mut Correspondence<'_>, budget: &mut Budget) -> Result<(), Fault> {
    let transport = comparison.transport;
    let cas = comparison.cas;
    declarations(
        cas,
        &comparison.real.coefficients,
        &comparison.complex.coefficients,
        &transport.coefficients,
        budget,
    )?;
    declarations(
        cas,
        &comparison.real.unknowns,
        &comparison.complex.unknowns,
        &transport.unknowns,
        budget,
    )?;
    let SolveDraft::CoefficientMap(map) =
        crate::solving::graph::object(&cas.state, cas.checked(&transport.coefficient_map)?)?
    else {
        return Err(invalid());
    };
    if (&map.source, &map.target, map.mode)
        != (
            &comparison.real.coefficient_domain,
            &MapTarget::CoefficientDomain {
                domain: comparison.complex.coefficient_domain.clone(),
            },
            MapMode::Embedding,
        )
    {
        return Err(invalid());
    }
    let overall = transport.regions.first().ok_or_else(invalid)?;
    if map.region != overall.real {
        return Err(invalid());
    }
    crate::solving::coefficients::proof::identity_map(cas, map, comparison.premises, budget)?;
    images(cas, map, budget)?;
    comparison.check(
        &comparison.real.coefficient_domain,
        &comparison.complex.coefficient_domain,
        budget,
    )?;
    for pair in &transport.regions {
        comparison.check(&pair.real, &pair.complex, budget)?;
        crate::solving::elementary::member::contained(cas, &pair.real, &overall.real, budget)?;
        crate::solving::elementary::member::contained(
            cas,
            &pair.complex,
            &overall.complex,
            budget,
        )?;
    }
    comparison.check(
        &comparison.real.predicate,
        &comparison.complex.predicate,
        budget,
    )?;
    comparison.check(&comparison.real.domain, &comparison.complex.domain, budget)?;
    if comparison.real.denominators.len() != comparison.complex.denominators.len() {
        return Err(invalid());
    }
    for (left, right) in comparison
        .real
        .denominators
        .iter()
        .zip(&comparison.complex.denominators)
    {
        comparison.check(left, right, budget)?;
    }
    Ok(())
}

/// Both coefficient covers are proved; paired strata are exact members of those covers.
pub(crate) fn covers(
    certificate: &CertificateRealRestriction,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let transport = &certificate.transport;
    let overall = transport.regions.first().ok_or_else(invalid)?;
    let Statement::Cover {
        domain: real,
        members: left,
    } = &premise(premises, transport.real_cover)?.judgment.statement
    else {
        return Err(invalid());
    };
    let Statement::Cover {
        domain: complex,
        members: right,
    } = &premise(premises, certificate.cover)?.judgment.statement
    else {
        return Err(invalid());
    };
    let strata = transport.regions.get(1..).ok_or_else(invalid)?;
    if (real, complex, left.len()) != (&overall.real, &overall.complex, right.len())
        || strata.is_empty()
    {
        return Err(invalid());
    }
    for (index, pair) in strata.iter().enumerate() {
        budget.work(crate::graph::size(strata.len(), AT)?, AT)?;
        if left.contains(&pair.real) != right.contains(&pair.complex)
            || strata
                .get(..index)
                .ok_or_else(invalid)?
                .iter()
                .any(|previous| previous.real == pair.real || previous.complex == pair.complex)
        {
            return Err(invalid());
        }
    }
    if !left
        .iter()
        .all(|member| strata.iter().any(|pair| &pair.real == member))
        || !right
            .iter()
            .all(|member| strata.iter().any(|pair| &pair.complex == member))
    {
        return Err(invalid());
    }
    Ok(())
}
