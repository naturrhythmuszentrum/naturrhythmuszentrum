// SPDX-License-Identifier: AGPL-3.0-only
//! Rounded repeated coordinates retain their selected copies and numerical error witnesses.

use super::{LiveBindingError, ReadingRegion, admission::charge};
use crate::{CompanionAdapter, EntryTicket, HeapError, ReadingAdapter};
use nrz_bindings::{
    Budget, CompanionKind,
    readings::{CompanionReadingKind, algebra::Handle},
};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// Outward discrepancy between two stored copies of one exact family coordinate.
///
/// The retained representation proves their shared lineage. The supplied numerical
/// radii are declarations about that same exact target, never independent sources.
#[derive(Clone, Debug)]
pub struct CopyDiscrepancy {
    /// Original expanded family whose reconstruction identifies the two exact copies.
    pub(super) family: Handle,
    /// Actual selected storage positions, in comparison order.
    pub(super) entries: [EntryTicket<usize>; 2],
    /// Numerical-error storage for exactly these two positions.
    pub(super) parameters: [EntryTicket<usize>; 2],
    /// Whether the second center is negated before comparison.
    negated: bool,
    /// Outward bound on the absolute center difference, widened exactly to binary64.
    upper: f64,
    /// Each selected copy's declared numerical radius, widened exactly to binary64.
    errors: [f64; 2],
}

impl CopyDiscrepancy {
    /// Exact expanded family; the binding retains its reconstruction and source identity.
    #[must_use]
    pub const fn family(&self) -> &Handle {
        &self.family
    }
    /// Ordered stored copies compared by this numerical check.
    #[must_use]
    pub const fn entries(&self) -> &[EntryTicket<usize>; 2] {
        &self.entries
    }
    /// Ordered numerical-error parameter positions for those same copies.
    #[must_use]
    pub const fn parameters(&self) -> &[EntryTicket<usize>; 2] {
        &self.parameters
    }
    /// Whether the second copy represents the negative of the first.
    #[must_use]
    pub const fn negated(&self) -> bool {
        self.negated
    }
    /// Outward absolute discrepancy; binary32 bounds widen exactly to this reporting format.
    #[must_use]
    pub const fn upper(&self) -> f64 {
        self.upper
    }
    /// Original separate numerical radii, without adding them again to deterministic totals.
    #[must_use]
    pub const fn errors(&self) -> [f64; 2] {
        self.errors
    }
}

/// Check all differing repeated and negative copies before any heap allocation.
pub(super) fn check<P: Scalar>(
    adapter: &ReadingAdapter<usize>,
    companions: &[CompanionAdapter<usize>],
    regions: &[ReadingRegion<P>],
    values: &[P],
    budget: &mut Budget,
) -> Result<Vec<CopyDiscrepancy>, LiveBindingError> {
    if values.len() != 4 || adapter.entries().len() != 4 {
        return Err(HeapError::InvalidLayout.into());
    }
    charge::<CopyDiscrepancy>(2, budget)?;
    budget.work(48, super::admission::AT)?;
    let context = ArithmeticContext::new()?;
    let mut result = crate::binding::buffer(2)?;
    for (first, second, negated) in [(0, 3, false), (1, 2, true)] {
        let compared = if negated {
            -values[second]
        } else {
            values[second]
        };
        if values[first] == compared {
            continue;
        }
        let entries = [
            adapter.entries()[first].clone(),
            adapter.entries()[second].clone(),
        ];
        let left = radius(adapter, &entries[0], companions, regions, budget)?;
        let right = radius(adapter, &entries[1], companions, regions, budget)?;
        let discrepancy = point(values[first])?
            .sub(point(compared)?, &context)?
            .abs()
            .upper();
        let allowance = point(left.1)?.add(point(right.1)?, &context)?;
        if discrepancy > allowance.lower() {
            return Err(LiveBindingError::UncoveredCopy);
        }
        result.push(CopyDiscrepancy {
            family: adapter.reading().family().node().clone(),
            entries,
            parameters: [left.0, right.0],
            negated,
            upper: discrepancy.get().to_f64(),
            errors: [left.1.to_f64(), right.1.to_f64()],
        });
    }
    Ok(result)
}

/// A radius must name the exact selected scalar copy, with an identity output parameter map.
fn radius<P: Scalar>(
    whole: &ReadingAdapter<usize>,
    entry: &EntryTicket<usize>,
    companions: &[CompanionAdapter<usize>],
    regions: &[ReadingRegion<P>],
    budget: &mut Budget,
) -> Result<(EntryTicket<usize>, P), LiveBindingError> {
    charge::<EntryTicket<usize>>(companions.len(), budget)?;
    let mut found = None;
    for companion in companions {
        if companion.companion().kind()
            != CompanionReadingKind::Radius(CompanionKind::NumericalError)
        {
            continue;
        }
        let [target] = companion.targets() else {
            continue;
        };
        let selected = target.entries().len() == 1 && target.entries().first() == Some(entry);
        let family = target.reading().family().node() == whole.reading().family().node();
        if !selected || !family || !identity(target.reading().map()) {
            continue;
        }
        if found.is_some() || !identity(companion.companion().map()) {
            return Err(crate::ReadingAdapterError::Target.into());
        }
        let [parameter] = companion.entries() else {
            return Err(HeapError::MissingCoverage.into());
        };
        found = Some((
            parameter.clone(),
            super::companions::value(regions, parameter)?,
        ));
    }
    found.ok_or_else(|| HeapError::MissingCoverage.into())
}

/// Scalar identity means the parameter bounds this stored copy without an unexecuted transformation.
fn identity(map: &nrz_bindings::readings::algebra::readings::AffineMap) -> bool {
    let shape = map.inputs() == 1 && map.outputs() == 1;
    let coefficient = map
        .coefficients()
        .first()
        .is_some_and(|value| value == &nrz_bindings::readings::algebra::Rational::one());
    let offset = map
        .offsets()
        .first()
        .is_some_and(nrz_bindings::readings::algebra::Rational::is_zero);
    shape && coefficient && offset
}

/// Only finite represented centers and nonnegative supplied error parameters enter this arithmetic.
fn point<P: Scalar>(value: P) -> Result<Enclosure<P>, nrz_scalar::ArithmeticFailure> {
    Ok(Enclosure::singleton(Finite::new(value)?))
}

/// Full owned evidence remains reserved while later exact dependency calls allocate their scratch.
pub(super) fn bytes(count: usize) -> Result<u64, LiveBindingError> {
    u64::try_from(count)
        .ok()
        .and_then(|value| {
            value.checked_mul(u64::try_from(std::mem::size_of::<CopyDiscrepancy>()).ok()?)
        })
        .ok_or_else(|| HeapError::ExtentOverflow.into())
}

/// Preserve every prior copy witness while checking and retaining another expanded block.
pub(super) fn append<P: Scalar>(
    copies: &mut Vec<CopyDiscrepancy>,
    adapter: &ReadingAdapter<usize>,
    companions: &[CompanionAdapter<usize>],
    regions: &[ReadingRegion<P>],
    values: &[P],
    budget: &mut Budget,
) -> Result<(), LiveBindingError> {
    let payload = values
        .len()
        .checked_mul(std::mem::size_of::<P>())
        .ok_or(HeapError::ExtentOverflow)?;
    let retained = bytes(copies.capacity())?
        .checked_add(u64::try_from(payload).map_err(|_| HeapError::ExtentOverflow)?)
        .ok_or(HeapError::ExtentOverflow)?;
    let proven = budget.with_retained_payload(retained, super::admission::AT, |budget| {
        check(adapter, companions, regions, values, budget)
    })?;
    let count = copies
        .len()
        .checked_add(proven.capacity())
        .and_then(|value| value.checked_add(proven.len()))
        .ok_or(HeapError::ExtentOverflow)?;
    charge::<CopyDiscrepancy>(count, budget)?;
    copies
        .try_reserve_exact(proven.len())
        .map_err(|_| HeapError::Capacity)?;
    copies.extend(proven);
    Ok(())
}
