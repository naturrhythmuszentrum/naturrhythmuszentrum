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

impl BindingSymbols {
    /// Attach the original identity radius definitions to a selected homogeneous reading.
    ///
    /// Parameter storage contains these output coordinates; a nonidentity source map
    /// requires its separately checked execution and cannot be read as an identity.
    /// # Errors
    /// Refuses substituted targets, duplicate kinds, nonidentity maps and missing totals.
    pub fn with_reading_companions(
        mut self,
        companions: &[&crate::readings::CompanionReading],
        budget: &mut Budget,
    ) -> Result<Self, AdmissionError> {
        for companion in companions {
            budget.work(1, Location::new("selected companion inventory"))?;
            let kind = match companion.kind() {
                crate::readings::CompanionReadingKind::Radius(kind) => kind,
                crate::readings::CompanionReadingKind::Covariance => {
                    return Err(AdmissionError::UnsupportedRecipe);
                }
            };
            let channel = match kind {
                super::CompanionKind::DeterministicBound => &mut self.bound,
                super::CompanionKind::NumericalError => &mut self.error,
            };
            if channel.is_some() {
                return Err(AdmissionError::CompanionMismatch);
            }
            *channel = Some(radius_symbol(&self.value, companion, kind, budget)?);
        }
        if self.error.is_some() && self.bound.is_none() {
            return Err(AdmissionError::CompanionMismatch);
        }
        Rc::make_mut(&mut self.value.0).companions = [self.bound.is_some(), self.error.is_some()];
        self.validate()?;
        Ok(self)
    }
}

/// Preserve the exact parameter definition and target, independently of the numerical ticket.
fn radius_symbol(
    value: &QuantitySymbol,
    companion: &crate::readings::CompanionReading,
    kind: super::CompanionKind,
    budget: &mut Budget,
) -> Result<QuantitySymbol, AdmissionError> {
    let reading = value.reading().ok_or(AdmissionError::UnsupportedRecipe)?;
    let [target] = companion.targets() else {
        return Err(AdmissionError::CompanionMismatch);
    };
    let count = companion
        .map()
        .coefficients()
        .len()
        .checked_add(companion.map().offsets().len())
        .ok_or(AdmissionError::ResourceLimit)?;
    budget.work(
        u64::try_from(count).map_err(|_| AdmissionError::ResourceLimit)?,
        Location::new("selected companion map"),
    )?;
    if target != reading || companion.map() != reading.map() || value.shape()[1] != 1 {
        return Err(AdmissionError::UnsupportedRecipe);
    }
    Ok(QuantitySymbol(Rc::new(graph::SymbolData {
        graph: value.0.graph.clone(),
        node: companion.node().clone(),
        sort: Sort::Value(AtomicSort::Matrix {
            field: Field::Real,
            rows: Extent::Literal(value.shape()[0]),
            columns: Extent::Literal(1),
        }),
        shape: value.shape(),
        quantity: crate::uncertainty(
            crate::UncertaintyMode::Radius,
            value.descriptor(),
            None,
            None,
        )?,
        kind: SymbolKind::Companion(kind),
        companions: [false, false],
        relation: Some(companion.node().clone()),
        target: Some(value.node().clone()),
        lineage: None,
        reading: None,
    })))
}

impl BindingSymbols {
    /// Declare a restored homogeneous projection of the original mixed coordinate reading.
    ///
    /// Both maps are identities and every selected position retains the original family,
    /// entry meaning and fixed reference. The numerical owner must execute restoration,
    /// retain its original companions and publish both generated radius channels together.
    /// # Errors
    /// Refuses unrelated families, reordered or substituted positions, unsupported meanings
    /// and exhausted exact resources. These derived declarations cannot be imported.
    pub fn restored_reading(
        graph: Rc<Cas>,
        original: &PhysicalReading,
        selected: &PhysicalReading,
        positions: &[usize],
        budget: &mut Budget,
    ) -> Result<Self, AdmissionError> {
        identity(original, budget)?;
        if original.family() != selected.family() || positions.len() != selected.entries().len() {
            return Err(AdmissionError::ValueMismatch);
        }
        for ((&position, entry), index) in positions
            .iter()
            .zip(selected.entries())
            .zip(selected.selection().indices())
        {
            budget.work(1, Location::new("restored reading projection"))?;
            if original.entries().get(position) != Some(entry)
                || original.selection().indices().get(position) != Some(index)
            {
                return Err(AdmissionError::ValueMismatch);
            }
        }
        let source = Self::from_reading(graph, selected, budget)?;
        graph::declare(
            source.value().descriptor(),
            source.value().shape(),
            [true, true],
            Some(source.value()),
            budget,
        )
    }
}
