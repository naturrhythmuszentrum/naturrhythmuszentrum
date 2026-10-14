// SPDX-License-Identifier: AGPL-3.0-only
//! Typed real-axis roles for finite boundary calculus.

use crate::boundary_model::CaseDerivative;
use crate::case_syntax::invalid;
use crate::domain_model::AmbientDomain;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::{AtomicSort, Field, Sort};
use crate::proof_model::Statement;

/// Require a declared real coordinate in the function's actual header.
pub(crate) fn coordinate(cas: &Cas, function: &Handle, key: &str) -> Result<(), Fault> {
    let value = cas.state.expression(cas.checked(function)?)?;
    let real = Sort::Value(AtomicSort::Scalar(Field::Real));
    if value.sort != real || !value.scope.is_empty() {
        return Err(invalid());
    }
    let header = cas.state.header(value.header)?;
    if !header
        .environment
        .declarations
        .iter()
        .any(|item| item.key == key && item.sort == real)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Boundary identity includes its real coordinate and exact function scope.
pub(crate) fn point(
    cas: &Cas,
    function: &Handle,
    boundary: &Handle,
    key: &str,
) -> Result<(), Fault> {
    coordinate(cas, function, key)?;
    let value = cas.state.boundary(cas.checked(boundary)?)?;
    if value.ambient != AmbientDomain::Real(key.into())
        || value.scope != cas.state.expression(cas.checked(function)?)?.scope
    {
        return Err(invalid());
    }
    Ok(())
}

/// Region claims cannot move between assignment universes or coordinate axes.
pub(crate) fn region(
    cas: &Cas,
    function: &Handle,
    domain: &Handle,
    key: &str,
) -> Result<(), Fault> {
    coordinate(cas, function, key)?;
    let value = cas.state.region(cas.checked(domain)?)?;
    if value.ambient != AmbientDomain::Real(key.into())
        || value.scope != cas.state.expression(cas.checked(function)?)?.scope
    {
        return Err(invalid());
    }
    Ok(())
}

/// Validate the added statement vocabulary without trusting its proposed mathematics.
pub(crate) fn validate(cas: &Cas, statement: &Statement) -> Result<(), Fault> {
    match statement {
        Statement::BoundaryPartition(value) => {
            region(cas, &value.function, &value.domain, &value.coordinate)?;
            for boundary in &value.boundaries {
                point(cas, &value.function, boundary, &value.coordinate)?;
            }
            for cell in &value.cells {
                region(cas, &value.function, cell, &value.coordinate)?;
            }
        }
        Statement::CaseDerivative(value) => derivative(cas, value)?,
        Statement::DerivativeAt(value) => {
            positive(value.order)?;
            point(cas, &value.function, &value.boundary, &value.coordinate)?;
            coordinate(cas, &value.value, &value.coordinate)?;
        }
        Statement::DerivativeDoesNotExist(value) => {
            positive(value.order)?;
            point(cas, &value.function, &value.boundary, &value.coordinate)?;
        }
        Statement::TraceValue(value) => {
            let boundary = cas.state.boundary(cas.checked(&value.boundary)?)?;
            let AmbientDomain::Real(key) = &boundary.ambient else {
                return Err(invalid());
            };
            point(cas, &value.function, &value.boundary, key)?;
            coordinate(cas, &value.value, key)?;
        }
        _ => return Err(invalid()),
    }
    Ok(())
}

/// A positive order is part of a classical derivative claim.
pub(crate) const fn positive(order: u32) -> Result<(), Fault> {
    if order == 0 { Err(invalid()) } else { Ok(()) }
}

/// Both endpoints have the same real coordinate interpretation.
fn derivative(cas: &Cas, value: &CaseDerivative) -> Result<(), Fault> {
    positive(value.order)?;
    region(cas, &value.function, &value.domain, &value.coordinate)?;
    coordinate(cas, &value.derivative, &value.coordinate)
}
