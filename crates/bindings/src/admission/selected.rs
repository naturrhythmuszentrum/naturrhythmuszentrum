// SPDX-License-Identifier: AGPL-3.0-only
//! The homogeneous recipe consumes only an exact identity-coordinate general reading.

use super::{AdmissionError, BindingSymbols, HomogeneousRecipe, QuantitySymbol, SymbolKind, graph};
use crate::readings::PhysicalReading;
use nrz_cas::{
    Cas, Location, Rational,
    limits::Budget,
    syntax::{AtomicSort, Extent, Field, Sort},
};
use std::rc::Rc;

impl BindingSymbols {
    /// Select an exact general reading for the closed homogeneous numerical recipe.
    ///
    /// The original definition and its graph are retained; no fresh unrelated symbol
    /// or numerical import can replace the selected reading. Nonidentity maps require
    /// coordinate lowering before this recipe can consume their stored coordinates.
    /// # Errors
    /// Refuses mixed/point/noncoherent meanings, nonidentity maps, unsupported sorts,
    /// foreign graph identities and exhausted exact resources.
    pub fn from_reading(
        graph: Rc<Cas>,
        reading: &PhysicalReading,
        budget: &mut Budget,
    ) -> Result<Self, AdmissionError> {
        let _syntax = graph.describe_syntax(reading.node(), budget)?;
        let at = Location::new("homogeneous selected reading");
        let count = reading.entries().len();
        budget.work(
            u64::try_from(count).map_err(|_| AdmissionError::ResourceLimit)?,
            at,
        )?;
        let recipe = HomogeneousRecipe::check(
            reading
                .entries()
                .iter()
                .map(|entry| Some(entry.descriptor())),
            count,
        )?;
        for entry in reading.entries() {
            recipe.check_descriptor(entry.descriptor())?;
        }
        identity(reading, budget)?;
        let shape = match reading.sort() {
            Sort::Value(AtomicSort::Scalar(Field::Real)) => [1, 1],
            Sort::Value(AtomicSort::Matrix {
                field: Field::Real,
                rows: Extent::Literal(rows),
                columns: Extent::Literal(columns),
            }) => [*rows, *columns],
            _ => return Err(AdmissionError::UnsupportedRecipe),
        };
        let symbol = QuantitySymbol(Rc::new(graph::SymbolData {
            graph: graph::Authority::Reading(graph),
            node: reading.node().clone(),
            sort: reading.sort().clone(),
            shape,
            quantity: recipe.descriptor().clone(),
            kind: SymbolKind::Value,
            companions: [false, false],
            relation: Some(reading.node().clone()),
            target: Some(reading.family().root().clone()),
            lineage: None,
            reading: Some(reading.clone()),
        }));
        Self::check(symbol, None, None)
    }
}

/// Every original coefficient and offset is checked, including entries multiplied by zero.
fn identity(reading: &PhysicalReading, budget: &mut Budget) -> Result<(), AdmissionError> {
    let map = reading.map();
    if map.inputs() != map.outputs() {
        return Err(AdmissionError::UnsupportedRecipe);
    }
    let count = usize::try_from(map.inputs()).map_err(|_| AdmissionError::ResourceLimit)?;
    for (index, value) in map.coefficients().iter().enumerate() {
        budget.work(1, Location::new("homogeneous selected reading"))?;
        let expected = if index / count == index % count {
            Rational::one()
        } else {
            Rational::zero()
        };
        if value != &expected {
            return Err(AdmissionError::UnsupportedRecipe);
        }
    }
    for value in map.offsets() {
        budget.work(1, Location::new("homogeneous selected reading"))?;
        if !value.is_zero() {
            return Err(AdmissionError::UnsupportedRecipe);
        }
    }
    Ok(())
}
