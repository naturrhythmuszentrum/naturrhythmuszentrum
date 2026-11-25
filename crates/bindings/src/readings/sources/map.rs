// SPDX-License-Identifier: AGPL-3.0-only
//! Both selected targets compose over one explicit order of original scalar coordinates.

use super::{ReadingSource, SourceError, check_sources};
use crate::readings::{PhysicalReading, ReadingError, SourceJacobianUsage, support};
use nrz_cas::{
    Cas, Rational,
    limits::{Budget, BudgetKind},
    readings::AffineMap,
};
use std::rc::Rc;

/// Two exact target maps over an authenticated original source order.
///
/// Expansion, selection and reading are composed before any numerical covariance is read.
/// Repeated and negative copies remain the same coordinate. This object supplies no
/// covariance parameters, independence premise or nonlinear derivative.
#[derive(Clone, Debug)]
pub struct ReadingSourceMap(Rc<Definition>);

/// New paired-map owner; original graph and physical declaration evidence remain shared.
#[derive(Debug)]
struct Definition {
    /// Original scalar declarations in caller order, never sorted by display names.
    sources: Vec<ReadingSource>,
    /// Both complete physical readings, including their selected-copy identities.
    targets: [PhysicalReading; 2],
    /// Source-to-target affine maps with identical input-column order.
    maps: [AffineMap; 2],
    /// New owned metadata and exact coefficient allowance.
    bytes: u64,
    /// Caller snapshots around construction, including consumed prefixes.
    usage: SourceJacobianUsage,
}

impl ReadingSourceMap {
    /// Authenticate the complete original source basis and compose both target maps.
    /// # Errors
    /// Refuses duplicate/missing source coordinates, substituted physical families or contexts,
    /// unsupported source declarations and cumulative work or payload exhaustion.
    pub fn new(
        cas: &Cas,
        sources: Vec<ReadingSource>,
        targets: [PhysicalReading; 2],
        budget: &mut Budget,
    ) -> Result<Self, SourceError> {
        let before = budget.usage();
        let context = targets[0].family().context();
        let _header = cas.context_header(context, budget)?;
        check_sources(context, &sources, budget)?;
        if targets[1].family().context() != context {
            return Err(SourceError::Conflict);
        }
        let base = storage(sources.capacity(), &[])?;
        let first = budget.with_retained_payload(base, support::AT, |budget| {
            compose(cas, &sources, &targets[0], budget)
        })?;
        let retained = storage(sources.capacity(), std::slice::from_ref(&first))?;
        let second = budget.with_retained_payload(retained, support::AT, |budget| {
            compose(cas, &sources, &targets[1], budget)
        })?;
        let maps = [first, second];
        let bytes = storage(sources.capacity(), &maps)?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, support::AT)?;
        budget.check(BudgetKind::OutputBytes, bytes, support::AT)?;
        Ok(Self(Rc::new(Definition {
            sources,
            targets,
            maps,
            bytes,
            usage: SourceJacobianUsage {
                before,
                after: budget.usage(),
            },
        })))
    }
    /// Original declarations in the exact shared covariance-column order.
    #[must_use]
    pub fn sources(&self) -> &[ReadingSource] {
        &self.0.sources
    }
    /// Both complete original target readings, retaining copy/selection identity.
    #[must_use]
    pub fn targets(&self) -> &[PhysicalReading; 2] {
        &self.0.targets
    }
    /// Both source-to-target maps; affine offsets do not enter covariance contraction.
    #[must_use]
    pub fn maps(&self) -> &[AffineMap; 2] {
        &self.0.maps
    }
    /// New map/metadata allowance, excluding previously owned graph proof inventories.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
    /// Cumulative caller snapshots around exact composition.
    #[must_use]
    pub fn usage(&self) -> SourceJacobianUsage {
        self.0.usage
    }
}

/// Every original coordinate is matched even when its coefficient cancels to zero.
fn compose(
    cas: &Cas,
    sources: &[ReadingSource],
    target: &PhysicalReading,
    budget: &mut Budget,
) -> Result<AffineMap, SourceError> {
    let family = target.family();
    let original = target.family_map(cas, budget)?;
    let mut positions = support::reserve(family.original_entries().len(), budget)?;
    for coordinate in 0..family.original_entries().len() {
        let mut found = None;
        for (index, source) in sources.iter().enumerate() {
            budget.work(1, support::AT)?;
            let reading = source.reading();
            if reading.family().root() != family.root() {
                continue;
            }
            if reading.family().original_entries() != family.original_entries() {
                return Err(SourceError::Conflict);
            }
            let [entry] = reading.selection().indices() else {
                return Err(SourceError::Declaration);
            };
            let position = u64::from(entry.column) * u64::from(reading.family().shape()[0])
                + u64::from(entry.row);
            if position == u64::try_from(coordinate).map_err(|_| ReadingError::Shape)? {
                found = Some(index);
            }
        }
        positions.push(found.ok_or(SourceError::Missing)?);
    }
    let rows = usize::try_from(original.outputs()).map_err(|_| ReadingError::Shape)?;
    let count = rows.checked_mul(sources.len()).ok_or(ReadingError::Shape)?;
    let mut coefficients = support::reserve(count, budget)?;
    coefficients.resize_with(count, Rational::zero);
    for row in 0..rows {
        for (coordinate, &column) in positions.iter().enumerate() {
            budget.work(1, support::AT)?;
            coefficients[row * sources.len() + column] =
                original.coefficients()[row * positions.len() + coordinate].clone();
        }
    }
    Ok(AffineMap::new(
        u32::try_from(sources.len()).map_err(|_| ReadingError::Shape)?,
        original.outputs(),
        coefficients,
        original.offsets().to_vec(),
        budget,
    )?)
}

/// Count new containers and rounded limb allowances; shared source declarations keep their owners.
fn storage(source_capacity: usize, maps: &[AffineMap]) -> Result<u64, ReadingError> {
    let metadata = size_of::<Definition>()
        .checked_add(2 * size_of::<usize>())
        .and_then(|bytes| {
            bytes.checked_add(source_capacity.checked_mul(size_of::<ReadingSource>())?)
        })
        .ok_or(ReadingError::Shape)?;
    let mut bytes = u64::try_from(metadata).map_err(|_| ReadingError::Shape)?;
    for value in maps
        .iter()
        .flat_map(|map| map.coefficients().iter().chain(map.offsets()))
    {
        let (numerator, denominator) = value.component_bits();
        let limbs = numerator
            .div_ceil(64)
            .checked_add(denominator.div_ceil(64))
            .and_then(|count| count.checked_mul(16))
            .ok_or(ReadingError::Shape)?;
        bytes = bytes
            .checked_add(u64::try_from(size_of::<Rational>()).map_err(|_| ReadingError::Shape)?)
            .and_then(|count| count.checked_add(limbs))
            .ok_or(ReadingError::Shape)?;
    }
    Ok(bytes)
}
