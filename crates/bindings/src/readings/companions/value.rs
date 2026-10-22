// SPDX-License-Identifier: AGPL-3.0-only
//! Companion definitions derive their maps from checked physical target readings.

use super::super::{PhysicalReading, ReadingError, maps, support};
use super::{CompanionParameters, CompanionReadingError, CompanionReadingKind, parameters};
use crate::{Descriptor, PairingWitness};
use nrz_cas::{Cas, Handle, limits::Budget, readings::AffineMap};

/// A distinct companion symbol defined by the closed radius or covariance reading law.
#[derive(Clone, Debug)]
pub struct CompanionReading {
    /// Complete original parameter declaration with exact source target identities.
    pub(super) parameters: CompanionParameters,
    /// Distinct original output parameter declaration.
    pub(super) symbol: Handle,
    /// Actual output parameter definition, never resolved by display name.
    pub(super) node: Handle,
    /// Output targets in the same order as the covariance factors.
    pub(super) targets: Vec<PhysicalReading>,
    /// Exact linear parameter transformation, with zero offsets.
    pub(super) map: AffineMap,
    /// Output difference/product units in logical column order.
    pub(super) units: Vec<PairingWitness>,
}

impl CompanionReading {
    /// Define a companion through `abs(T)` or the two-sided `T*U*Q^T` law.
    ///
    /// Radius inputs name actual selected stored copies. Covariance inputs name the
    /// shared original coordinates and include complete expansion and selection maps.
    /// Additional numerical evaluation/import error is an external obligation.
    /// # Errors
    /// Refuses substituted target selections/families, wrong units/shapes and exact limits.
    pub fn define(
        cas: &mut Cas,
        symbol: &Handle,
        parameters: &CompanionParameters,
        targets: &[PhysicalReading],
        budget: &mut Budget,
    ) -> Result<Self, CompanionReadingError> {
        check_targets(parameters, targets, budget)?;
        if symbol == parameters.node() || targets.iter().any(|target| target.symbol() == symbol) {
            return Err(CompanionReadingError::Target);
        }
        let map = reading_map(cas, parameters.kind, targets, budget)?;
        let mut output = support::reserve(targets.len(), budget)?;
        for target in targets {
            let mut entries = support::reserve(target.entries().len(), budget)?;
            entries.extend(target.entries().iter().cloned());
            output.push(parameters::ParameterTarget {
                node: target.node().clone(),
                entries,
            });
        }
        let units = parameters::units(parameters.kind, &output, budget)?;
        let (sort, _, _, header) = support::inspect(cas, symbol, budget)?;
        let rows = u32::try_from(targets[0].entries().len()).map_err(|_| ReadingError::Shape)?;
        let columns = u32::try_from(targets.get(1).map_or(1, |target| target.entries().len()))
            .map_err(|_| ReadingError::Shape)?;
        if header != cas.context_header(&parameters.context)?
            || support::shape(&sort)? != [rows, columns]
        {
            return Err(ReadingError::Shape.into());
        }
        let node = cas.define_reading(symbol, parameters.node(), &map, budget)?;
        Ok(Self {
            parameters: parameters.clone(),
            symbol: symbol.clone(),
            node,
            targets: targets.to_vec(),
            map,
            units,
        })
    }
    /// Original distinct companion declaration.
    #[must_use]
    pub const fn symbol(&self) -> &Handle {
        &self.symbol
    }
    /// Exact defined companion symbol and its full original parameter expression.
    #[must_use]
    pub const fn node(&self) -> &Handle {
        &self.node
    }
    /// Closed interpretation distinguishes covered total bounds from numerical error.
    #[must_use]
    pub const fn kind(&self) -> CompanionReadingKind {
        self.parameters.kind
    }
    /// Retained input parameter symbol and ordered source targets.
    #[must_use]
    pub const fn parameters(&self) -> &CompanionParameters {
        &self.parameters
    }
    /// Actual output reading targets; covariance retains both in order.
    #[must_use]
    pub fn targets(&self) -> &[PhysicalReading] {
        &self.targets
    }
    /// Exact parameter map; origins never contribute to this map.
    #[must_use]
    pub const fn map(&self) -> &AffineMap {
        &self.map
    }
    /// Per-entry output quantity pairings.
    #[must_use]
    pub fn units(&self) -> &[PairingWitness] {
        &self.units
    }
    /// Validate supplied parameter descriptors without accepting numerical evidence.
    /// # Errors
    /// Refuses changed target product/difference units or affine offsets.
    pub fn check_units(&self, actual: &[Descriptor]) -> Result<(), CompanionReadingError> {
        parameters::check_units(&self.units, actual)
    }
    /// Match exact output targets before attaching numerical companion versions.
    /// # Errors
    /// Refuses another reading, including another stored copy with the same schema.
    pub fn check_targets(&self, targets: &[PhysicalReading]) -> Result<(), CompanionReadingError> {
        if targets.len() != self.targets.len() {
            return Err(CompanionReadingError::Target);
        }
        for (expected, actual) in self.targets.iter().zip(targets) {
            if expected.node() != actual.node() || expected.entries() != actual.entries() {
                return Err(CompanionReadingError::Target);
            }
        }
        Ok(())
    }
}

/// Original paired sources cannot be replaced by descriptor-equal independent quantities.
fn check_targets(
    parameters: &CompanionParameters,
    targets: &[PhysicalReading],
    budget: &mut Budget,
) -> Result<(), CompanionReadingError> {
    if targets.len() != parameters::target_count(parameters.kind) {
        return Err(CompanionReadingError::Target);
    }
    for (original, reading) in parameters.targets.iter().zip(targets) {
        let actual = parameters::parameter_target(parameters.kind, reading, budget)?;
        if original != &actual || reading.family().context() != &parameters.context {
            return Err(CompanionReadingError::Target);
        }
    }
    Ok(())
}

/// Compose covariance lineage through the common family; retain actual-copy radii separately.
fn reading_map(
    cas: &Cas,
    kind: CompanionReadingKind,
    targets: &[PhysicalReading],
    budget: &mut Budget,
) -> Result<AffineMap, CompanionReadingError> {
    let first = targets.first().ok_or(CompanionReadingError::Target)?;
    Ok(match kind {
        CompanionReadingKind::Radius(_) => maps::radius(first.map(), budget)?,
        CompanionReadingKind::Covariance => {
            let second = targets.get(1).ok_or(CompanionReadingError::Target)?;
            maps::covariance(
                &first.family_map(cas, budget)?,
                &second.family_map(cas, budget)?,
                budget,
            )?
        }
    })
}
