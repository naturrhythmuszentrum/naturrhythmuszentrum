// SPDX-License-Identifier: AGPL-3.0-only
//! Exact parent-before-child splitting-chain reconstruction for root adjunction.

use super::dense::Dense;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{AT, domain, invalid, vector};
use crate::solving::{CoefficientDomainData, SolveDraft};

/// Embed all parent coefficients through the canonical monic-quotient inclusion.
fn include(parent: &Dense, engine: &Engine<'_>, budget: &mut Budget) -> Result<Dense, Fault> {
    let mut coefficients = vector(parent.coefficients.len(), budget)?;
    for value in &parent.coefficients {
        coefficients.push(
            engine
                .include_parent(value, budget)
                .map_err(|error| fault(&error))?,
        );
    }
    Dense::new(engine, coefficients, budget).map_err(|error| fault(&error))
}

/// The factor X-t uses the actual child generator, not an unrelated selected expression.
fn factor(engine: &Engine<'_>, budget: &mut Budget) -> Result<Dense, Fault> {
    let (_, extension) = super::arithmetic::level(engine).map_err(|error| fault(&error))?;
    let parent = Engine {
        state: engine.state,
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
        engine,
        Dense::new(&parent, entries, budget).map_err(|error| fault(&error))?,
        budget,
    )
    .map_err(|error| fault(&error))?;
    let mut coefficients = vector(2, budget)?;
    coefficients.push(
        engine
            .neg(&generator, budget)
            .map_err(|error| fault(&error))?,
    );
    coefficients.push(
        engine
            .constant(true, budget)
            .map_err(|error| fault(&error))?,
    );
    Dense::new(engine, coefficients, budget).map_err(|error| fault(&error))
}

/// Every supplied next polynomial strictly drops degree and reconstructs the previous one in its new level.
pub(crate) fn check(
    cas: &Cas,
    original: &Handle,
    chain: &[Handle],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut previous = original;
    for (index, next) in chain.iter().enumerate() {
        let depth = crate::solving::coefficients::storage::add(crate::graph::size(index, AT)?, 1)?;
        budget.solving(BudgetKind::TowerLevels, depth, AT)?;
        let parent = Dense::read(&cas.state, previous, budget).map_err(|error| fault(&error))?;
        let quotient = Dense::read(&cas.state, next, budget).map_err(|error| fault(&error))?;
        let CoefficientDomainData::Quotient { level } = domain(&cas.state, &quotient.domain)?
        else {
            return Err(invalid());
        };
        let SolveDraft::ExtensionLevel(extension) =
            crate::solving::graph::object(&cas.state, level.logical_id())?
        else {
            return Err(invalid());
        };
        if extension.polynomial != *previous
            || extension.parent != parent.domain
            || quotient.coefficients.len().checked_add(1) != Some(parent.coefficients.len())
        {
            return Err(invalid());
        }
        let engine = Engine {
            state: &cas.state,
            domain: &quotient.domain,
        };
        let product = factor(&engine, budget)?
            .mul(&quotient, &cas.state, budget)
            .map_err(|error| fault(&error))?;
        if !product
            .equals(&include(&parent, &engine, budget)?, &cas.state, budget)
            .map_err(|error| fault(&error))?
        {
            return Err(invalid());
        }
        previous = next;
    }
    Ok(())
}
