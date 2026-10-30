// SPDX-License-Identifier: AGPL-3.0-only
//! The admitted exact inverse transports the whole original box with rational arithmetic.

use std::collections::BTreeMap;

use super::{CheckedLatticeBasis, LatticeBasisQuestion, RationalBoxEnclosure};
use crate::budget::{Budget, BudgetKind};
use crate::decision::RationalEnclosure;
use crate::decision_pi::Interval;
use crate::encoding::Encoder;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, size};
use crate::number::Rational;

/// New affine-box work has its own owner, leaving scalar replay costs unchanged.
const AT: Location = Location::new("lattice affine box enclosure");

/// An untrusted outward transport of an original box through an exact admitted inverse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LatticeBoxEnclosure {
    /// Exact original basis, offset and context whose inverse is being used.
    pub basis: LatticeBasisQuestion,
    /// Original-target box E, preserving every coordinate's original replay evidence.
    pub original: RationalBoxEnclosure,
    /// Exact admitted homogeneous inverse `[A, -A phi; 0, 1]`.
    pub inverse: Handle,
    /// Row-major scalar bounds for the first d rows of the homogeneous inverse.
    /// Each row includes d linear entries followed by its translation entry.
    pub coefficients: Vec<RationalEnclosure>,
    /// Proposed closed basis-coordinate box H, in the same ordered frame.
    pub bounds: Vec<(Rational, Rational)>,
}

/// Sealed local judgment `A*(q-phi) in H`, proved by transporting all of E.
///
/// Retained inverse proof roots authenticate the affine relation in this store.
/// This receipt supplies no portable vector theorem or selected Integer tuple.
#[derive(Clone, Debug)]
pub struct CheckedLatticeBoxEnclosure {
    /// Complete original and transformed boxes, inverse and coefficient certificates.
    contract: LatticeBoxEnclosure,
    /// Determinant, right-inverse and left-inverse roots from the admitted basis.
    basis_proofs: [Handle; 3],
    /// Conservative returned ownership, excluding shared store-owned proofs.
    bytes: u64,
}

impl CheckedLatticeBoxEnclosure {
    /// The complete original-target affine-box judgment and all retained origins.
    #[must_use]
    pub const fn contract(&self) -> &LatticeBoxEnclosure {
        &self.contract
    }

    /// Borrow the original determinant and both inverse-identity proof roots.
    #[must_use]
    pub const fn basis_proofs(&self) -> &[Handle; 3] {
        &self.basis_proofs
    }

    /// Conservative prepaid ownership of the complete local result.
    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        self.bytes
    }
}

impl Cas {
    /// Replay the original vector and bound the entire box through its admitted inverse.
    ///
    /// Scalar replay checks each exact inverse entry, including zero coefficients.
    /// Exact interval products and sums then check inclusion in the proposed H.
    /// Every irrational coefficient retains its original expression and certificate.
    /// # Errors
    /// Refuses changed bases, inverses, coordinate order or contexts, invalid original
    /// or coefficient boxes, unsupported reciprocal bounds and exhausted resources.
    /// The operation changes no graph state and returns no partial coordinate result.
    pub fn check_lattice_box_enclosure(
        &mut self,
        basis: &CheckedLatticeBasis,
        contract: &LatticeBoxEnclosure,
        budget: &mut Budget,
    ) -> Result<CheckedLatticeBoxEnclosure, Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = crate::decision_kernel::cache::attempt(self, budget, |cas, budget| {
            cas.lattice_box_enclosure(basis, contract, budget)
        });
        budget.restore_limits(previous);
        result
    }

    /// Validate all identities and certificates before copying the complete receipt.
    fn lattice_box_enclosure(
        &self,
        basis: &CheckedLatticeBasis,
        contract: &LatticeBoxEnclosure,
        budget: &mut Budget,
    ) -> Result<CheckedLatticeBoxEnclosure, Fault> {
        self.validate_lattice_basis(&contract.basis, basis, budget)?;
        self.checked(&contract.inverse)?;
        if contract.inverse != *basis.inverse().map().expression() {
            return Err(mismatch());
        }
        let bytes = validate(self, contract, basis.dimension(), budget)?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes.saturating_mul(2), AT)?;
        budget.work(bytes, AT)?;
        Ok(CheckedLatticeBoxEnclosure {
            contract: contract.clone(),
            basis_proofs: [
                basis.inverse().nonsingularity().proof().clone(),
                basis.inverse().witness().proof().clone(),
                basis.left_inverse().proof().clone(),
            ],
            bytes,
        })
    }
}

/// Shared semantic replay after the caller authenticates the original basis and inverse.
pub(super) fn validate(
    cas: &Cas,
    contract: &LatticeBoxEnclosure,
    dimension: u32,
    budget: &mut Budget,
) -> Result<u64, Fault> {
    budget.work(4, AT)?;
    cas.checked(&contract.inverse)?;
    if contract.original.context != contract.basis.context
        || size(contract.original.coordinates.len(), AT)? != u64::from(dimension)
        || contract.bounds.len() != contract.original.coordinates.len()
    {
        return Err(mismatch());
    }
    let mut bytes = super::enclosure::validate(cas, &contract.original, budget)?;
    bytes = bytes.saturating_add(512);
    let count = contract.original.coordinates.len();
    let width = count.saturating_add(1);
    let entries = &cas
        .state
        .expression(cas.checked(&contract.inverse)?)?
        .operands;
    let coefficient_count = count.saturating_mul(width);
    if contract.coefficients.len() != coefficient_count {
        return Err(Fault::invalid(InvalidCode::ReadingShape, AT));
    }
    budget.check(BudgetKind::MatrixEntries, size(coefficient_count, AT)?, AT)?;
    bytes = coefficients(cas, contract, entries, bytes, budget)?;
    for (lower, upper) in &contract.bounds {
        lower.check(budget, AT)?;
        upper.check(budget, AT)?;
        bytes = bytes
            .saturating_add(number_bytes(lower))
            .saturating_add(number_bytes(upper));
    }
    let scratch = budget
        .limits()
        .component_bits
        .div_ceil(8)
        .saturating_mul(64)
        .saturating_add(1_024);
    budget.with_retained_payload(bytes.saturating_add(scratch), AT, |budget| {
        transport(contract, width, budget)
    })?;
    Ok(bytes)
}

/// Reuse only byte-identical complete scalar contracts within this one transform.
fn coefficients(
    cas: &Cas,
    contract: &LatticeBoxEnclosure,
    entries: &[crate::graph::LogicalId],
    mut bytes: u64,
    budget: &mut Budget,
) -> Result<u64, Fault> {
    let mut checked = BTreeMap::<Vec<u8>, u64>::new();
    let mut keys = 0_u64;
    for (index, coefficient) in contract.coefficients.iter().enumerate() {
        budget.work(3, AT)?;
        if entries.get(index) != Some(&cas.checked(&coefficient.target)?)
            || coefficient.context != contract.original.context
            || coefficient.input_family != contract.original.input_family
        {
            return Err(mismatch());
        }
        // One-coordinate maps retain the original validation cost without reuse keys.
        if contract.original.coordinates.len() == 1 {
            let scalar = budget.with_retained_payload(bytes, AT, |budget| {
                crate::decision_enclosure::validate(cas, coefficient, budget)
            })?;
            bytes = bytes.saturating_add(scalar);
            budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
            continue;
        }
        let key = budget.with_retained_payload(bytes.saturating_add(keys), AT, |budget| {
            let mut out = Encoder::new(budget, AT);
            crate::decision_wire::encode_enclosure(coefficient, &mut out, &mut |handle, out| {
                cas.checked(handle)?;
                crate::statement::handle(handle, out)
            })?;
            Ok(out.finish())
        })?;
        let key_bytes = size(key.len(), AT)?.saturating_mul(4).saturating_add(128);
        budget.work(
            key_bytes.saturating_mul(size(checked.len(), AT)?.saturating_add(1)),
            AT,
        )?;
        let scalar = if let Some(scalar) = checked.get(&key) {
            *scalar
        } else {
            let scalar = budget.with_retained_payload(
                bytes.saturating_add(keys).saturating_add(key_bytes),
                AT,
                |budget| crate::decision_enclosure::validate(cas, coefficient, budget),
            )?;
            keys = keys.saturating_add(key_bytes);
            checked.insert(key, scalar);
            scalar
        };
        bytes = bytes.saturating_add(scalar);
        budget.check(
            BudgetKind::StagedPayloadBytes,
            bytes.saturating_add(keys),
            AT,
        )?;
    }
    Ok(bytes)
}

/// Check exact row extrema for all original-box realizations, without sharpening q to a point.
fn transport(
    contract: &LatticeBoxEnclosure,
    width: usize,
    budget: &mut Budget,
) -> Result<(), Fault> {
    use std::cmp::Ordering;
    for (row, (lower, upper)) in contract
        .coefficients
        .chunks_exact(width)
        .zip(&contract.bounds)
    {
        let (offset, linear) = row.split_last().ok_or_else(mismatch)?;
        let mut sum = interval(offset);
        for (coefficient, coordinate) in linear.iter().zip(&contract.original.coordinates) {
            let product = interval(coefficient).product(&interval(coordinate), budget)?;
            sum.lower = sum.lower.add(&product.lower, budget)?;
            sum.upper = sum.upper.add(&product.upper, budget)?;
        }
        let includes_lower = lower.compare(&sum.lower, budget)? != Ordering::Greater;
        let includes_upper = sum.upper.compare(upper, budget)? != Ordering::Greater;
        if !includes_lower || !includes_upper {
            return Err(Fault::invalid(InvalidCode::DecisionBox, AT));
        }
    }
    Ok(())
}

/// Bounds have already been checked against the original scalar expression.
fn interval(enclosure: &RationalEnclosure) -> Interval {
    Interval {
        lower: enclosure.lower.clone(),
        upper: enclosure.upper.clone(),
    }
}

/// Conservative rational endpoint ownership without copying bigint components.
fn number_bytes(value: &Rational) -> u64 {
    let (numerator, denominator) = value.component_bits();
    numerator
        .div_ceil(8)
        .saturating_add(denominator.div_ceil(8))
        .saturating_mul(4)
        .saturating_add(128)
}

/// Original basis, inverse and coordinate identities are immutable parts of the judgment.
const fn mismatch() -> Fault {
    Fault::invalid(InvalidCode::EndpointMismatch, AT)
}
