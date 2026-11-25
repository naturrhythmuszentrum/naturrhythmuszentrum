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
fn regions(
    cas: &Cas,
    first: &Handle,
    second: &Handle,
    budget: &mut crate::budget::Budget,
) -> Result<(), Fault> {
    let left = cas.state.region(cas.checked(first, budget)?, budget)?;
    let right = cas.state.region(cas.checked(second, budget)?, budget)?;
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
    budget: &mut Budget,
) -> Result<(), Fault> {
    if crate::solving::family::body(cas, cas.checked(left, budget)?, budget)?.is_some()
        || crate::solving::family::body(cas, cas.checked(right, budget)?, budget)?.is_some()
    {
        return crate::solving::family::native::validate_endpoints(
            cas, left, right, domain, same_sort, budget,
        );
    }
    let left = cas.state.expression(cas.checked(left, budget)?, budget)?;
    let right = cas.state.expression(cas.checked(right, budget)?, budget)?;
    let domain = cas.state.region(cas.checked(domain, budget)?, budget)?;
    if left.scope != right.scope
        || (same_sort && left.sort != right.sort)
        || (!domain.scope.is_empty() && domain.scope != left.scope)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Validate every currently represented role before it can become a checked judgment.
#[expect(
    clippy::too_many_lines,
    reason = "the complete statement role dispatch remains visible in one exhaustive match"
)]
pub(crate) fn validate(cas: &Cas, statement: &Statement, budget: &mut Budget) -> Result<(), Fault> {
    crate::work_observation::observe!(budget, Location::new("statement object roles"), {
        budget.work(1, Location::new("statement object roles"))?;
        match statement {
            Statement::Series(value) => crate::series::check::roles(cas, value, budget)?,
            Statement::HalfPiWitness {
                u,
                k,
                context,
                domain,
            } => {
                crate::trig::roles(cas, u, k, context, domain, budget)?;
            }
            Statement::LatticeSelection(value) | Statement::LatticeNearestSelection(value) => {
                crate::lattice::selection::roles(cas, value, budget)?;
            }
            Statement::LatticeCover(value) => {
                crate::lattice::cover::check::roles(cas, value, budget)?;
            }
            Statement::IntegerSelection(value) => {
                crate::decision_integer_selection::roles(cas, value, budget)?;
            }
            Statement::IntegerFamily(value) => {
                crate::decision_cut::family::roles(cas, value, budget)?;
            }
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
            } => endpoints(cas, left, right, domain, true, budget)?,
            Statement::DerivativeOn {
                function,
                derivative,
                domain,
                ..
            } => {
                endpoints(cas, function, derivative, domain, true, budget)?;
            }
            Statement::Jacobian {
                input,
                output,
                domain,
                ..
            } => endpoints(cas, input, output, domain, false, budget)?,
            Statement::SquareFree {
                input,
                output,
                domain,
                ..
            } => endpoints(cas, input, output, domain, true, budget)?,
            Statement::PredicateFact { region, predicate } => {
                let region = cas.state.region(cas.checked(region, budget)?, budget)?;
                let (_, scope) = crate::domain_admission::predicate_context(
                    &cas.state,
                    cas.checked(predicate, budget)?,
                    budget,
                )?;
                if !region.scope.is_empty() && region.scope != *scope {
                    return Err(invalid());
                }
            }
            Statement::Subset { subset, superset } => regions(cas, subset, superset, budget)?,
            Statement::Disjoint { left, right } => regions(cas, left, right, budget)?,
            Statement::Cover { domain, members } => {
                cas.state.region(cas.checked(domain, budget)?, budget)?;
                for member in members {
                    regions(cas, domain, member, budget)?;
                }
            }
            Statement::Empty { region } | Statement::Constructor { region, .. } => {
                cas.state.region(cas.checked(region, budget)?, budget)?;
            }
            Statement::DefinedDomain { function, domain } => {
                endpoints(cas, function, function, domain, true, budget)?;
            }
            Statement::Compatibility {
                left,
                right,
                region,
            } => {
                compatibility(cas, left, right, region, budget)?;
            }
            Statement::Elaboration {
                source,
                result,
                domain,
            } => {
                cas.state
                    .source_form(cas.checked(source, budget)?, budget)?;
                let result = cas.state.expression(cas.checked(result, budget)?, budget)?;
                regions(
                    cas,
                    &cas.handle(result.domain.ok_or_else(invalid)?),
                    domain,
                    budget,
                )?;
            }
            _ => boundary(cas, statement, budget)?,
        }
        Ok(())
    })
}

/// Compatibility compares exact arm regions and the original scalar sort.
fn compatibility(
    cas: &Cas,
    left: &Handle,
    right: &Handle,
    region: &Handle,
    budget: &mut crate::budget::Budget,
) -> Result<(), Fault> {
    let left = cas.state.arm(cas.checked(left, budget)?, budget)?;
    let right = cas.state.arm(cas.checked(right, budget)?, budget)?;
    regions(cas, &cas.handle(left.region), region, budget)?;
    regions(cas, &cas.handle(right.region), region, budget)?;
    if left.sort != right.sort {
        return Err(invalid());
    }
    Ok(())
}

/// Later boundary statements still retain exact object categories without executable rules.
fn boundary(cas: &Cas, statement: &Statement, budget: &mut Budget) -> Result<(), Fault> {
    match statement {
        Statement::Incidence {
            region,
            boundary,
            neighborhood,
            ..
        } => {
            regions(cas, region, neighborhood, budget)?;
            let region = cas.state.region(cas.checked(region, budget)?, budget)?;
            let boundary = cas.state.boundary(cas.checked(boundary, budget)?, budget)?;
            if region.scope != boundary.scope || region.ambient != boundary.ambient {
                return Err(invalid());
            }
        }
        Statement::Ownership {
            function,
            boundary,
            value,
        } => {
            let function = cas
                .state
                .expression(cas.checked(function, budget)?, budget)?;
            let boundary = cas.state.boundary(cas.checked(boundary, budget)?, budget)?;
            if function.scope != boundary.scope {
                return Err(invalid());
            }
            if let Ownership::Present(value) = value {
                let value = cas.state.expression(cas.checked(value, budget)?, budget)?;
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
            cas.state.boundary(cas.checked(boundary, budget)?, budget)?;
            endpoints(cas, function, function, region, true, budget)?;
            for value in values {
                endpoints(cas, function, value, region, true, budget)?;
            }
        }
        Statement::Jump {
            function,
            boundary,
            value,
            ..
        } => {
            let function = cas
                .state
                .expression(cas.checked(function, budget)?, budget)?;
            let boundary = cas.state.boundary(cas.checked(boundary, budget)?, budget)?;
            let value = cas.state.expression(cas.checked(value, budget)?, budget)?;
            if function.scope != boundary.scope
                || function.scope != value.scope
                || function.sort != value.sort
            {
                return Err(invalid());
            }
        }
        _ => crate::boundary_roles::validate(cas, statement, budget)?,
    }
    Ok(())
}
