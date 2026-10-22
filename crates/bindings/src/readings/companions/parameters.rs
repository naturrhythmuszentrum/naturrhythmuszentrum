// SPDX-License-Identifier: AGPL-3.0-only
//! Parameter declarations retain actual selected or common-family target expressions.

use super::super::{EntryMeaning, PhysicalReading, ReadingError, support};
use super::CompanionReadingError;
use crate::{CompanionKind, Descriptor, PairingWitness, UncertaintyMode};
use nrz_cas::{Cas, Handle, limits::Budget, syntax::Operator};

/// Closed interpretation of a physical companion parameter symbol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompanionReadingKind {
    /// Selected-entry deterministic or numerical difference radii.
    Radius(CompanionKind),
    /// Covariance over two ordered original free-coordinate families.
    Covariance,
}

/// Exact source target with its original physical coordinates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ParameterTarget {
    /// Actual selection expression for radii, or free-family declaration for covariance.
    pub(super) node: Handle,
    /// Physical source coordinates in the same order as the parameter matrix.
    pub(super) entries: Vec<EntryMeaning>,
}

/// Declared companion parameter symbol; no numerical positivity or PSD proof is implied.
#[derive(Clone, Debug)]
pub struct CompanionParameters {
    /// Distinct declared parameter symbol, independent of every physical value target.
    pub(super) node: Handle,
    /// Exact header and domain context.
    pub(super) context: Handle,
    /// Closed deterministic/numerical/covariance interpretation.
    pub(super) kind: CompanionReadingKind,
    /// One selected bound target or two ordered covariance family targets.
    pub(super) targets: Vec<ParameterTarget>,
    /// Difference or product quantity declarations in logical column order.
    pub(super) units: Vec<PairingWitness>,
}

impl CompanionParameters {
    /// Declare a parameter over actual selected copies or original joint coordinates.
    /// # Errors
    /// Refuses missing/extra targets, wrong declaration shape/scope and invalid paired units.
    pub fn new(
        cas: &Cas,
        context: &Handle,
        symbol: &Handle,
        kind: CompanionReadingKind,
        targets: &[PhysicalReading],
        budget: &mut Budget,
    ) -> Result<Self, CompanionReadingError> {
        let expected = target_count(kind);
        if targets
            .iter()
            .any(|target| target.symbol() == symbol || target.family().root() == symbol)
        {
            return Err(CompanionReadingError::Target);
        }
        if targets.len() != expected {
            return Err(CompanionReadingError::Target);
        }
        let mut checked = support::reserve(expected, budget)?;
        for target in targets {
            if target.family().context() != context {
                return Err(CompanionReadingError::Target);
            }
            checked.push(parameter_target(kind, target, budget)?);
        }
        let (sort, operator, _, header) = support::inspect(cas, symbol, budget)?;
        if !matches!(operator, Operator::Free(_)) || header != cas.context_header(context)? {
            return Err(ReadingError::Definition.into());
        }
        if support::shape(&sort)? != parameter_shape(&checked)? {
            return Err(ReadingError::Shape.into());
        }
        let units = units(kind, &checked, budget)?;
        Ok(Self {
            node: symbol.clone(),
            context: context.clone(),
            kind,
            targets: checked,
            units,
        })
    }
    /// Actual parameter declaration; no sampled payload is part of this identity.
    #[must_use]
    pub const fn node(&self) -> &Handle {
        &self.node
    }
    /// Physical companion interpretation.
    #[must_use]
    pub const fn kind(&self) -> CompanionReadingKind {
        self.kind
    }
    /// Ordered parameter unit pairings, including both covariance target kinds.
    #[must_use]
    pub fn units(&self) -> &[PairingWitness] {
        &self.units
    }
    /// Actual selected-entry or original-family target handles in order.
    pub fn targets(&self) -> impl ExactSizeIterator<Item = &Handle> {
        self.targets.iter().map(|target| &target.node)
    }
    /// Match numerical parameter descriptors against these exact difference/product units.
    /// # Errors
    /// Refuses omitted entries or any changed companion kind, unit or affine offset.
    pub fn check_units(&self, actual: &[Descriptor]) -> Result<(), CompanionReadingError> {
        check_units(&self.units, actual)
    }
}

/// Radius and covariance use different source-coordinate contracts.
pub(super) fn parameter_target(
    kind: CompanionReadingKind,
    target: &PhysicalReading,
    budget: &mut Budget,
) -> Result<ParameterTarget, CompanionReadingError> {
    let (node, entries) = match kind {
        CompanionReadingKind::Covariance => {
            let mut entries = support::reserve(target.family.root_entries.len(), budget)?;
            entries.extend(target.family.root_entries.iter().cloned());
            (target.family.root().clone(), entries)
        }
        CompanionReadingKind::Radius(_) => {
            let mut entries = support::reserve(target.selection.indices().len(), budget)?;
            for index in target.selection.indices() {
                let position = u64::from(index.column) * u64::from(target.family.shape()[0])
                    + u64::from(index.row);
                entries.push(
                    target
                        .family
                        .entries()
                        .get(usize::try_from(position).map_err(|_| ReadingError::Shape)?)
                        .ok_or(ReadingError::Shape)?
                        .clone(),
                );
            }
            (target.selected().clone(), entries)
        }
    };
    Ok(ParameterTarget { node, entries })
}

/// Required target arity is part of the closed interpretation.
pub(super) const fn target_count(kind: CompanionReadingKind) -> usize {
    match kind {
        CompanionReadingKind::Radius(_) => 1,
        CompanionReadingKind::Covariance => 2,
    }
}

/// One-target bounds are columns; covariance carries both complete ordered extents.
fn parameter_shape(targets: &[ParameterTarget]) -> Result<[u32; 2], ReadingError> {
    let rows = targets.first().ok_or(ReadingError::Shape)?.entries.len();
    let columns = targets.get(1).map_or(1, |target| target.entries.len());
    Ok([
        u32::try_from(rows).map_err(|_| ReadingError::Shape)?,
        u32::try_from(columns).map_err(|_| ReadingError::Shape)?,
    ])
}

/// Derive one physical difference/product pairing for every parameter entry.
pub(super) fn units(
    kind: CompanionReadingKind,
    targets: &[ParameterTarget],
    budget: &mut Budget,
) -> Result<Vec<PairingWitness>, CompanionReadingError> {
    let first = &targets
        .first()
        .ok_or(CompanionReadingError::Target)?
        .entries;
    let second = targets.get(1).map(|target| target.entries.as_slice());
    let columns = second.map_or(1, <[EntryMeaning]>::len);
    let mut pairings = support::reserve(
        first
            .len()
            .checked_mul(columns)
            .ok_or(ReadingError::Shape)?,
        budget,
    )?;
    for column in 0..columns {
        for entry in first {
            let other = second
                .and_then(|entries| entries.get(column))
                .map(EntryMeaning::descriptor);
            let mode = match kind {
                CompanionReadingKind::Radius(_) => UncertaintyMode::Radius,
                CompanionReadingKind::Covariance => UncertaintyMode::Covariance,
            };
            pairings.push(PairingWitness::uncertainty(
                mode,
                entry.descriptor(),
                other,
                None,
            )?);
        }
    }
    Ok(pairings)
}

/// Units are exact declarations, including distinct paired uncertainty kinds.
pub(super) fn check_units(
    expected: &[PairingWitness],
    actual: &[Descriptor],
) -> Result<(), CompanionReadingError> {
    if expected.len() != actual.len() {
        return Err(CompanionReadingError::Unit);
    }
    for (pairing, descriptor) in expected.iter().zip(actual) {
        if pairing.uncertainty_descriptor() != Some(descriptor) {
            return Err(CompanionReadingError::Unit);
        }
    }
    Ok(())
}
