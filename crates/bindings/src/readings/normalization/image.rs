// SPDX-License-Identifier: AGPL-3.0-only
//! Independently checked diagonal images preserve exact constants until explicit rational export.

use super::super::{EntryMeaning, ReadingError, support};
use super::{CoordinateDirection, CoordinateNormalization, storage};
use nrz_cas::{
    Cas, ExactConstant, Handle, Rational,
    limits::{Budget, BudgetKind},
    proof::VerifiedWitness,
    readings::{AffineMap, ExactAffineMap, ReuseKey},
};
use std::rc::Rc;

/// A checked per-entry coordinate image, retaining both endpoint definitions and its original law.
#[derive(Clone, Debug)]
pub struct CoordinateImage(Rc<Image>);

/// Shared image evidence; cloning keeps the same exact definitions and certificate.
#[derive(Debug)]
struct Image {
    /// Normalized source frame; absence means coherent physical input coordinates.
    source: Option<CoordinateNormalization>,
    /// Normalized target frame; absence means coherent physical output coordinates.
    target: Option<CoordinateNormalization>,
    /// Native original inverse or ordered composition, before coefficient reduction.
    exact: ExactAffineMap,
    /// Exact diagonal coefficients, including valid non-rational definitions.
    scales: Vec<ExactConstant>,
    /// Exact per-entry affine origins; absent origins become exact zeros.
    offsets: Vec<ExactConstant>,
    /// Checked equality of the original law and the explicit diagonal image.
    witness: VerifiedWitness,
    /// Full original correspondence and guards, never a key of rounded values.
    key: ReuseKey,
    /// Conservative retained payload for nested exact export.
    bytes: u64,
}

impl CoordinateImage {
    /// Normalized source references, or coherent physical input when absent.
    #[must_use]
    pub fn source(&self) -> Option<&CoordinateNormalization> {
        self.0.source.as_ref()
    }
    /// Normalized target references, or coherent physical output when absent.
    #[must_use]
    pub fn target(&self) -> Option<&CoordinateNormalization> {
        self.0.target.as_ref()
    }
    /// Original native matrix expression, including ordered inverse composition.
    #[must_use]
    pub fn exact(&self) -> &ExactAffineMap {
        &self.0.exact
    }
    /// Complete diagonal exact scale definitions, in original entry order.
    #[must_use]
    pub fn scales(&self) -> &[ExactConstant] {
        &self.0.scales
    }
    /// Complete exact origins, separate from all radius and covariance scales.
    #[must_use]
    pub fn offsets(&self) -> &[ExactConstant] {
        &self.0.offsets
    }
    /// Whole-context equality to the original native map.
    #[must_use]
    pub fn witness(&self) -> &VerifiedWitness {
        &self.0.witness
    }
    /// Complete exact correspondence key; endpoint reference identities remain separate requirements.
    #[must_use]
    pub fn reuse_key(&self) -> &ReuseKey {
        &self.0.key
    }

    /// Conservative retained declaration, constant and certificate payload for nested consumers.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }

    /// Export the checked exact image as rational coefficients without approximating constants.
    ///
    /// Exact cancellation precedes this boundary: degree-to-turn exports `1/360`,
    /// while a remaining pi scale or origin refuses with `UnsupportedExactConstantExport`.
    /// # Errors
    /// Refuses any non-rational required coefficient and cumulative resource exhaustion.
    pub fn export(&self, budget: &mut Budget) -> Result<AffineMap, ReadingError> {
        budget.with_retained_payload(self.0.bytes, support::AT, |budget| {
            storage::export(&self.0.scales, &self.0.offsets, budget)?;
            let count = u32::try_from(self.0.scales.len()).map_err(|_| ReadingError::Shape)?;
            let width = self.0.scales.len();
            let length = width.checked_mul(width).ok_or(ReadingError::Shape)?;
            let mut coefficients = support::reserve(length, budget)?;
            let mut offsets = support::reserve(width, budget)?;
            for (row, scale) in self.0.scales.iter().enumerate() {
                let scale = rational(scale)?;
                for column in 0..width {
                    coefficients.push(if row == column {
                        scale.clone()
                    } else {
                        Rational::zero()
                    });
                }
                offsets.push(rational(&self.0.offsets[row])?.clone());
            }
            Ok(AffineMap::new(count, count, coefficients, offsets, budget)?)
        })
    }
}

/// Mutable construction state is never exposed as checked correspondence.
struct Proposed {
    /// Input frame or coherent coordinates.
    source: Option<CoordinateNormalization>,
    /// Output frame or coherent coordinates.
    target: Option<CoordinateNormalization>,
    /// Original native map.
    exact: ExactAffineMap,
    /// Explicit diagonal coefficients.
    scales: Vec<ExactConstant>,
    /// Explicit point shifts.
    offsets: Vec<ExactConstant>,
}

/// The original diagonal restoration retains all typed coordinates before any zero elimination.
pub(super) fn restoration(
    cas: &mut Cas,
    context: &Handle,
    entries: &[EntryMeaning],
    budget: &mut Budget,
) -> Result<ExactAffineMap, ReadingError> {
    let (mut scales, mut offsets) = allocate(entries.len(), budget)?;
    for entry in entries {
        append(
            &mut scales,
            &mut offsets,
            entries.len(),
            entry.magnitude(),
            entry.shift(),
            budget,
        )?;
    }
    let bytes = storage::pairs(&scales, &offsets, entries.len())?;
    budget.with_retained_payload(bytes, support::AT, |budget| {
        diagonal(cas, context, &scales, &offsets, budget)
    })
}

/// Normalization and restoration use the same retained inverse certificate and entry identities.
pub(super) fn direction(
    cas: &mut Cas,
    frame: &CoordinateNormalization,
    direction: CoordinateDirection,
    budget: &mut Budget,
) -> Result<CoordinateImage, ReadingError> {
    let image = budget.with_retained_payload(frame.0.bytes, support::AT, |budget| {
        let count = frame.entries().len();
        let (mut scales, mut offsets) = allocate(count, budget)?;
        for entry in frame.entries() {
            let bytes = storage::pairs(&scales, &offsets, count)?;
            let (scale, offset) =
                budget.with_retained_payload(bytes, support::AT, |budget| match direction {
                    CoordinateDirection::Normalize => pair(entry, None, budget),
                    CoordinateDirection::Restore => clone_pair(entry, budget),
                })?;
            extend(&mut scales, &mut offsets, count, scale, offset, budget)?;
        }
        let (source, target, exact) = match direction {
            CoordinateDirection::Normalize => {
                (None, Some(frame.clone()), frame.inverse().map().clone())
            }
            CoordinateDirection::Restore => {
                (Some(frame.clone()), None, frame.restoration().clone())
            }
        };
        build(
            cas,
            frame.context(),
            Proposed {
                source,
                target,
                exact,
                scales,
                offsets,
            },
            budget,
        )
    })?;
    retain(image, frame.0.bytes, budget)
}

/// Unlike kinds never become convertible merely because their reference magnitudes agree.
pub(super) fn conversion(
    cas: &mut Cas,
    source: &CoordinateNormalization,
    target: &CoordinateNormalization,
    budget: &mut Budget,
) -> Result<CoordinateImage, ReadingError> {
    if source.context() != target.context() {
        return Err(ReadingError::Unrelated);
    }
    if source.entries().len() != target.entries().len() {
        return Err(ReadingError::Shape);
    }
    let bytes = source
        .0
        .bytes
        .checked_add(target.0.bytes)
        .ok_or(ReadingError::Shape)?;
    let image = budget.with_retained_payload(bytes, support::AT, |budget| {
        let count = source.entries().len();
        let (mut scales, mut offsets) = allocate(count, budget)?;
        for (input, output) in source.entries().iter().zip(target.entries()) {
            input.descriptor().compatible_kind(output.descriptor())?;
            if input.descriptor().role() != output.descriptor().role() {
                return Err(ReadingError::Role);
            }
            let retained = storage::pairs(&scales, &offsets, count)?;
            let (scale, offset) =
                budget.with_retained_payload(retained, support::AT, |budget| {
                    pair(input, Some(output), budget)
                })?;
            extend(&mut scales, &mut offsets, count, scale, offset, budget)?;
        }
        let retained = storage::pairs(&scales, &offsets, count)?;
        let exact = budget.with_retained_payload(retained, support::AT, |budget| {
            cas.compose_reading_maps(target.inverse().map(), source.restoration(), budget)
        })?;
        // The enclosing reservation already accounts for both endpoint certificates.
        build(
            cas,
            source.context(),
            Proposed {
                source: Some(source.clone()),
                target: Some(target.clone()),
                exact,
                scales,
                offsets,
            },
            budget,
        )
    })?;
    retain(image, bytes, budget)
}

/// Retained endpoint evidence remains part of the complete result's finite payload contract.
fn retain(mut image: Image, bytes: u64, budget: &Budget) -> Result<CoordinateImage, ReadingError> {
    image.bytes = image.bytes.checked_add(bytes).ok_or(ReadingError::Shape)?;
    budget.check(BudgetKind::OutputBytes, image.bytes, support::AT)?;
    Ok(CoordinateImage(Rc::new(image)))
}

/// Prove the explicit image against the original inverse or composition before sealing it.
fn build(
    cas: &mut Cas,
    context: &Handle,
    proposed: Proposed,
    budget: &mut Budget,
) -> Result<Image, ReadingError> {
    let bytes = storage::constants(&proposed.scales)?
        .checked_add(storage::constants(&proposed.offsets)?)
        .ok_or(ReadingError::Shape)?;
    let direct = budget.with_retained_payload(bytes, support::AT, |budget| {
        diagonal(cas, context, &proposed.scales, &proposed.offsets, budget)
    })?;
    let witness = budget.with_retained_payload(bytes, support::AT, |budget| {
        cas.prove_reading_equality(
            context,
            proposed.exact.expression(),
            direct.expression(),
            budget,
        )
    })?;
    if !witness.is_discharged() {
        return Err(ReadingError::Definition);
    }
    let bytes = storage::witness(cas, &witness, bytes, budget)?;
    let key = budget.with_retained_payload(bytes, support::AT, |budget| {
        cas.witness_reuse_key(&witness, budget)
    })?;
    let bytes = bytes
        .checked_add(u64::try_from(key.as_bytes().len()).map_err(|_| ReadingError::Shape)?)
        .ok_or(ReadingError::Shape)?;
    budget.check(BudgetKind::OutputBytes, bytes, support::AT)?;
    Ok(Image {
        source: proposed.source,
        target: proposed.target,
        exact: proposed.exact,
        scales: proposed.scales,
        offsets: proposed.offsets,
        witness,
        key,
        bytes,
    })
}

/// Build only the used finite diagonal; an empty coordinate system performs no axis traversal.
fn diagonal(
    cas: &mut Cas,
    context: &Handle,
    scales: &[ExactConstant],
    offsets: &[ExactConstant],
    budget: &mut Budget,
) -> Result<ExactAffineMap, ReadingError> {
    let length = scales
        .len()
        .checked_mul(scales.len())
        .ok_or(ReadingError::Shape)?;
    let bytes = storage::diagonal(scales, offsets, budget)?;
    let mut coefficients = support::reserve(length, budget)?;
    for (row, scale) in scales.iter().enumerate() {
        for column in 0..scales.len() {
            coefficients.push(if row == column {
                scale.clone()
            } else {
                ExactConstant::zero()
            });
        }
    }
    let count = u32::try_from(scales.len()).map_err(|_| ReadingError::Shape)?;
    Ok(budget.with_retained_payload(bytes, support::AT, |budget| {
        cas.constant_reading_map(context, (count, count), &coefficients, offsets, budget)
    })?)
}

/// Export never substitutes an approximation for a valid exact descriptor constant.
fn rational(value: &ExactConstant) -> Result<&Rational, ReadingError> {
    value
        .as_rational()
        .ok_or_else(|| crate::UnitError::UnsupportedExactConstantExport.into())
}

/// Allocate both capacities together before their first owned constant is copied.
fn allocate(
    count: usize,
    budget: &mut Budget,
) -> Result<(Vec<ExactConstant>, Vec<ExactConstant>), ReadingError> {
    // Prefix payload scans are quadratic; reserve their complete schedule before either vector.
    let work = u64::try_from(count).map_err(|_| ReadingError::Shape)?;
    budget.work(
        work.checked_mul(work)
            .and_then(|value| value.checked_mul(4))
            .ok_or(ReadingError::Shape)?,
        support::AT,
    )?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        storage::pairs(&[], &[], count)?,
        support::AT,
    )?;
    Ok((
        support::reserve(count, budget)?,
        support::reserve(count, budget)?,
    ))
}

/// Retained coefficients remain reserved during each additional exact arithmetic call.
fn pair(
    input: &EntryMeaning,
    output: Option<&EntryMeaning>,
    budget: &mut Budget,
) -> Result<(ExactConstant, ExactConstant), ReadingError> {
    let denominator = output.map_or_else(|| input.magnitude(), EntryMeaning::magnitude);
    let one = ExactConstant::one();
    let numerator = output.map_or_else(|| &one, |_| input.magnitude());
    let scale = numerator.div(denominator, budget)?;
    let offset = budget.with_retained_payload(
        storage::constants(std::slice::from_ref(&scale))?,
        support::AT,
        |budget| {
            let shifted = match output {
                Some(output) => input.shift().sub(output.shift(), budget)?,
                None => input.shift().negated(budget)?,
            };
            let retained = storage::constants(std::slice::from_ref(&shifted))?;
            budget.with_retained_payload(retained, support::AT, |budget| {
                Ok::<_, ReadingError>(shifted.div(denominator, budget)?)
            })
        },
    )?;
    Ok((scale, offset))
}

/// Clone a new pair only after accounting for its complete limb payload and the old arrays.
fn append(
    scales: &mut Vec<ExactConstant>,
    offsets: &mut Vec<ExactConstant>,
    count: usize,
    scale: &ExactConstant,
    offset: &ExactConstant,
    budget: &Budget,
) -> Result<(), ReadingError> {
    let bytes = storage::pairs(scales, offsets, count)?
        .checked_add(storage::constants(std::slice::from_ref(scale))?)
        .and_then(|value| value.checked_add(storage::constants(std::slice::from_ref(offset)).ok()?))
        .ok_or(ReadingError::Shape)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, support::AT)?;
    scales.push(scale.clone());
    offsets.push(offset.clone());
    Ok(())
}

/// Admit both new owned constants before cloning their immutable original definitions.
fn clone_pair(
    entry: &EntryMeaning,
    budget: &Budget,
) -> Result<(ExactConstant, ExactConstant), ReadingError> {
    let bytes = storage::constants(std::slice::from_ref(entry.magnitude()))?
        .checked_add(storage::constants(std::slice::from_ref(entry.shift()))?)
        .ok_or(ReadingError::Shape)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, support::AT)?;
    Ok((entry.magnitude().clone(), entry.shift().clone()))
}

/// Move already accounted exact arithmetic results into the preallocated image without an extra clone.
fn extend(
    scales: &mut Vec<ExactConstant>,
    offsets: &mut Vec<ExactConstant>,
    count: usize,
    scale: ExactConstant,
    offset: ExactConstant,
    budget: &Budget,
) -> Result<(), ReadingError> {
    let bytes = storage::pairs(scales, offsets, count)?
        .checked_add(storage::constants(std::slice::from_ref(&scale))?)
        .and_then(|value| {
            value.checked_add(storage::constants(std::slice::from_ref(&offset)).ok()?)
        })
        .ok_or(ReadingError::Shape)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, support::AT)?;
    scales.push(scale);
    offsets.push(offset);
    Ok(())
}
