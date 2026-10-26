// SPDX-License-Identifier: AGPL-3.0-only
//! Typed object roles are checked independently of primitive availability.

use crate::budget::Budget;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle};
use crate::proof_model::{Ownership, Statement};

/// A well-formed statement never substitutes an arbitrary object for a typed role.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Sort, Location::new("statement object roles"))
}

/// Compare exact coordinate and binder interpretations of set operands.
fn regions(cas: &Cas, first: &Handle, second: &Handle) -> Result<(), Fault> {
    let left = cas.state.region(cas.checked(first)?)?;
    let right = cas.state.region(cas.checked(second)?)?;
    if left.ambient != right.ambient || left.scope != right.scope {
        return Err(invalid());
    }
    Ok(())
}

/// Check two scalar endpoints without imposing a scalar sort on a Jacobian result.
fn endpoints(
    cas: &Cas,
    left: &Handle,
    right: &Handle,
    domain: &Handle,
    same_sort: bool,
) -> Result<(), Fault> {
    if crate::solving::family::body(cas, cas.checked(left)?)?.is_some()
        || crate::solving::family::body(cas, cas.checked(right)?)?.is_some()
    {
        return crate::solving::family::native::validate_endpoints(
            cas, left, right, domain, same_sort,
        );
    }
    let left = cas.state.expression(cas.checked(left)?)?;
    let right = cas.state.expression(cas.checked(right)?)?;
    let domain = cas.state.region(cas.checked(domain)?)?;
    if left.scope != right.scope
        || (same_sort && left.sort != right.sort)
        || (!domain.scope.is_empty() && domain.scope != left.scope)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Validate every currently represented role before it can become a checked judgment.
pub(crate) fn validate(cas: &Cas, statement: &Statement, budget: &mut Budget) -> Result<(), Fault> {
    budget.work(1, Location::new("statement object roles"))?;
    match statement {
        Statement::LatticeCover(value) => {
            crate::lattice::cover::check::roles(cas, value, budget)?;
        }
        Statement::IntegerSelection(value) => crate::decision_integer_selection::roles(cas, value)?,
        Statement::IntegerFamily(value) => crate::decision_cut::family::roles(cas, value)?,
        Statement::Solve(value) => crate::solving::graph::statement(cas, value, budget)?,
        Statement::Germ(value) => crate::germ_check::validate(cas, value, budget)?,
        Statement::Split(value) => crate::split_geometry::validate(cas, value, budget)?,
        Statement::Distribution(value) => {
            crate::distribution_syntax::statement(cas, value, budget)?;
        }
        Statement::Equality {
            left,
            right,
            domain,
            ..
        } => endpoints(cas, left, right, domain, true)?,
        Statement::DerivativeOn {
            function,
            derivative,
            domain,
            ..
        } => {
            endpoints(cas, function, derivative, domain, true)?;
        }
        Statement::Jacobian {
            input,
            output,
            domain,
            ..
        } => endpoints(cas, input, output, domain, false)?,
        Statement::SquareFree {
            input,
            output,
            domain,
            ..
        } => endpoints(cas, input, output, domain, true)?,
        Statement::PredicateFact { region, predicate } => {
            let region = cas.state.region(cas.checked(region)?)?;
            let (_, scope) =
                crate::domain_admission::predicate_context(&cas.state, cas.checked(predicate)?)?;
            if !region.scope.is_empty() && region.scope != *scope {
                return Err(invalid());
            }
        }
        Statement::Subset { subset, superset } => regions(cas, subset, superset)?,
        Statement::Disjoint { left, right } => regions(cas, left, right)?,
        Statement::Cover { domain, members } => {
            cas.state.region(cas.checked(domain)?)?;
            for member in members {
                regions(cas, domain, member)?;
            }
        }
        Statement::Empty { region } | Statement::Constructor { region, .. } => {
            cas.state.region(cas.checked(region)?)?;
        }
        Statement::DefinedDomain { function, domain } => {
            endpoints(cas, function, function, domain, true)?;
        }
        Statement::Compatibility {
            left,
            right,
            region,
        } => {
            let left = cas.state.arm(cas.checked(left)?)?;
            let right = cas.state.arm(cas.checked(right)?)?;
            regions(cas, &cas.handle(left.region), region)?;
            regions(cas, &cas.handle(right.region), region)?;
            if left.sort != right.sort {
                return Err(invalid());
            }
        }
        Statement::Elaboration {
            source,
            result,
            domain,
        } => {
            cas.state.source_form(cas.checked(source)?)?;
            let result = cas.state.expression(cas.checked(result)?)?;
            regions(cas, &cas.handle(result.domain.ok_or_else(invalid)?), domain)?;
        }
        _ => boundary(cas, statement)?,
    }
    Ok(())
}

/// Later boundary statements still retain exact object categories without executable rules.
fn boundary(cas: &Cas, statement: &Statement) -> Result<(), Fault> {
    match statement {
        Statement::Incidence {
            region,
            boundary,
            neighborhood,
            ..
        } => {
            regions(cas, region, neighborhood)?;
            let region = cas.state.region(cas.checked(region)?)?;
            let boundary = cas.state.boundary(cas.checked(boundary)?)?;
            if region.scope != boundary.scope || region.ambient != boundary.ambient {
                return Err(invalid());
            }
        }
        Statement::Ownership {
            function,
            boundary,
            value,
        } => {
            let function = cas.state.expression(cas.checked(function)?)?;
            let boundary = cas.state.boundary(cas.checked(boundary)?)?;
            if function.scope != boundary.scope {
                return Err(invalid());
            }
            if let Ownership::Present(value) = value {
                let value = cas.state.expression(cas.checked(value)?)?;
                if function.sort != value.sort || function.scope != value.scope {
                    return Err(invalid());
                }
            }
        }
        Statement::Jet {
            function,
            region,
            boundary,
            values,
            ..
        } => {
            cas.state.boundary(cas.checked(boundary)?)?;
            endpoints(cas, function, function, region, true)?;
            for value in values {
                endpoints(cas, function, value, region, true)?;
            }
        }
        Statement::Jump {
            function,
            boundary,
            value,
            ..
        } => {
            let function = cas.state.expression(cas.checked(function)?)?;
            let boundary = cas.state.boundary(cas.checked(boundary)?)?;
            let value = cas.state.expression(cas.checked(value)?)?;
            if function.scope != boundary.scope
                || function.scope != value.scope
                || function.sort != value.sort
            {
                return Err(invalid());
            }
        }
        _ => crate::boundary_roles::validate(cas, statement)?,
    }
    Ok(())
}
