// SPDX-License-Identifier: AGPL-3.0-only
//! Witnessed discontinuity refuses a multiplier; unsupported smoothness remains pending.

use crate::budget::Budget;
use crate::distribution_syntax::{AT, invalid};
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, LogicalId};
use crate::model::Sort;

/// Distinguish a proved support jump from an unknown smoothness premise before building products.
pub(crate) fn multiplier(
    cas: &mut Cas,
    inputs: &[LogicalId],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let [first, second] = inputs else {
        return Err(invalid());
    };
    let (distribution, function) = if cas.state.expression(*first)?.sort == Sort::Distribution {
        (*first, *second)
    } else if cas.state.expression(*second)?.sort == Sort::Distribution {
        (*second, *first)
    } else {
        return Ok(());
    };
    let space = cas.handle(
        cas.state
            .expression(distribution)?
            .space
            .ok_or_else(invalid)?,
    );
    let function = cas.handle(function);
    match crate::distribution_regular::polynomial(cas, &function, &space, budget) {
        Ok(_) => return Ok(()),
        Err(fault) if !crate::case_obligations::unknown(&fault) => return Err(fault),
        Err(_) => {}
    }
    let points = crate::distribution_support::support(cas, distribution, budget)?;
    let certificate = crate::distribution_regular::certificate(cas, &function, &space, budget)?;
    for point in points {
        let Some(index) = certificate
            .partition
            .cuts
            .iter()
            .position(|cut| *cut == point)
        else {
            continue;
        };
        let (Some(left), Some(right)) = (
            certificate.polynomials.get(index).and_then(Option::as_ref),
            certificate
                .polynomials
                .get(index + 1)
                .and_then(Option::as_ref),
        ) else {
            continue;
        };
        let trace = crate::jet_polynomial::trace(left, &point, 0, budget)?;
        if trace != crate::jet_polynomial::trace(right, &point, 0, budget)?
            || point_mismatch(cas, &function, &space, &point, &trace, budget)?
        {
            return Err(Fault::invalid(
                InvalidCode::DistributionProductUndefined,
                AT,
            ));
        }
    }
    Err(Fault::invalid(InvalidCode::PendingPremise, AT))
}

/// Equal side traces still need the actual point value to establish continuity of a multiplier.
fn point_mismatch(
    cas: &mut Cas,
    function: &crate::graph::Handle,
    space: &crate::graph::Handle,
    point: &crate::number::Rational,
    trace: &crate::number::Rational,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let definition = cas.state.space(space.logical_id())?.clone();
    let domain = cas.handle(definition.domain);
    let singleton = crate::boundary_regions::cell(
        cas,
        &domain,
        &definition.coordinate,
        std::slice::from_ref(point),
        1,
        budget,
    )?;
    let root = function.logical_id();
    let analysis = crate::case_cells::analyze(cas, root, root, singleton.logical_id(), budget)?;
    let Some(value) = analysis.point_value(cas, root, singleton.logical_id(), budget)? else {
        return Ok(true);
    };
    let coefficients =
        crate::polynomial::univariate(&cas.state, value, &definition.coordinate, budget)?;
    Ok(crate::jet_polynomial::trace(&coefficients, point, 0, budget)? != *trace)
}
