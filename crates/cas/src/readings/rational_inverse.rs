// SPDX-License-Identifier: AGPL-3.0-only
//! Rational inversion is a specialization of the one exact map producer and checker.

use super::{AT, AffineMap, affine::length, invalid, reserve};
use crate::budget::{Budget, BudgetKind};
use crate::domain_model::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
use crate::error::{Fault, InvalidCode};
use crate::exact_constant::ExactConstant;
use crate::graph::{Cas, Draft, Handle, Reference, size};
use crate::model::{Environment, Operator};
use crate::number::Rational;

/// An exact rational inverse with its determinant and ordinary portable equality witness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AffineInverse {
    /// Reversed ordered map including the translated origin.
    map: AffineMap,
    /// Exact nonzero determinant from the shared producer.
    determinant: Rational,
    /// Complete portable right-inverse equality under the empty declaration context.
    bytes: Vec<u8>,
    /// Independently checked exact determinant certificate.
    nonsingularity: Vec<u8>,
}

impl AffineInverse {
    /// The exact map `z=T^(-1)*(y-d)`.
    #[must_use]
    pub const fn map(&self) -> &AffineMap {
        &self.map
    }

    /// Exact nonzero determinant of the original linear part.
    #[must_use]
    pub const fn determinant(&self) -> &Rational {
        &self.determinant
    }

    /// Portable exact determinant certificate with its strict nonzero premise.
    #[must_use]
    pub const fn nonsingularity_bytes(&self) -> &[u8] {
        self.nonsingularity.as_slice()
    }

    /// Ordinary witness bytes for independent expected-judgment replay.
    #[must_use]
    pub const fn witness_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

impl AffineMap {
    /// Invert a square rational map through the shared exact producer and proof kernel.
    ///
    /// Rational specialization retains the existing rational composition interface.
    /// Its witness proves the native homogeneous matrix right-inverse identity.
    /// # Errors
    /// Returns `ReadingShape` for nonsquare maps, `ReadingSingular` for singular
    /// maps, or the caller's cumulative arithmetic, Work or payload refusal.
    pub fn inverse(&self, budget: &mut Budget) -> Result<AffineInverse, Fault> {
        let retained = self.validate(budget)?;
        if self.inputs != self.outputs {
            return Err(invalid(InvalidCode::ReadingShape));
        }
        let previous = budget.reserve_temporary(retained, AT)?;
        let result = self.rational_inverse(budget);
        budget.restore_limits(previous);
        result
    }

    /// Only rational import and export surround the common nonsingularity operation.
    fn rational_inverse(&self, budget: &mut Budget) -> Result<AffineInverse, Fault> {
        let mut cas = Cas::new(budget.limits().clone())?;
        let context = empty_context(&mut cas, budget)?;
        let coefficients = constants(&self.coefficients, budget)?;
        let offsets = constants(&self.offsets, budget)?;
        let original = cas.constant_reading_map(
            &context,
            (self.inputs, self.outputs),
            &coefficients,
            &offsets,
            budget,
        )?;
        let inverse = cas.invert_reading_map(&context, &original, budget)?;
        let retained = cas
            .counts()
            .payload_bytes
            .saturating_add(size(inverse.witness().bytes.len(), AT)?.saturating_mul(4))
            .saturating_add(size(inverse.nonsingularity().bytes.len(), AT)?.saturating_mul(4));
        let previous = budget.reserve_temporary(retained, AT)?;
        let result = finish(&cas, &inverse, self.inputs, budget);
        budget.restore_limits(previous);
        result
    }
}

/// Export the rational specialization while the complete checked source store remains retained.
fn finish(
    cas: &Cas,
    inverse: &super::CheckedMapInverse,
    count: u32,
    budget: &mut Budget,
) -> Result<AffineInverse, Fault> {
    let determinant = rational(cas, inverse.determinant(), budget)?;
    let map = extract(cas, inverse.map().expression(), count, budget)?;
    let bytes = cas.export_witness(inverse.witness(), budget)?;
    let nonsingularity = cas.export_witness(inverse.nonsingularity(), budget)?;
    let map_bytes = map.validate(budget)?;
    let determinant_bytes =
        size(determinant.structural_bytes(budget)?.len(), AT)?.saturating_mul(4);
    let output = size(bytes.len().saturating_add(nonsingularity.len()), AT)?
        .saturating_add(map_bytes)
        .saturating_add(determinant_bytes);
    budget.check(BudgetKind::OutputBytes, output, AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, output, AT)?;
    Ok(AffineInverse {
        map,
        determinant,
        bytes,
        nonsingularity,
    })
}

/// No physical declaration or sample is introduced by rational specialization.
fn empty_context(cas: &mut Cas, budget: &mut Budget) -> Result<Handle, Fault> {
    cas.admit(
        &[
            Draft::Header(HeaderDraft::Core(Environment::default())),
            Draft::Region(RegionDraft {
                header: Reference::Local(0),
                scope: Vec::new(),
                ambient: AmbientDomain::Assignments,
                expression: RegionExpr::Ambient,
            }),
            Draft::Context(ContextDraft::Root {
                header: Reference::Local(0),
                admissible: Reference::Local(1),
                assumptions: Vec::new(),
            }),
        ],
        budget,
    )?
    .pop()
    .ok_or_else(|| invalid(InvalidCode::Reference))
}

/// Validated rational storage is the literal special case of exact constant storage.
fn constants(values: &[Rational], budget: &mut Budget) -> Result<Vec<ExactConstant>, Fault> {
    let mut result = reserve(values.len(), budget)?;
    result.extend(values.iter().cloned().map(ExactConstant::from));
    Ok(result)
}

/// The closed constant checker independently reconstructs each exported rational.
fn rational(cas: &Cas, value: &Handle, budget: &mut Budget) -> Result<Rational, Fault> {
    ExactConstant::from_expression(cas, value, budget)?
        .as_rational()
        .cloned()
        .ok_or_else(|| invalid(InvalidCode::ReadingDefinition))
}

/// Remove the homogeneous row only after checking its native representation.
fn extract(cas: &Cas, root: &Handle, count: u32, budget: &mut Budget) -> Result<AffineMap, Fault> {
    let term = cas.state.expression(cas.checked(root)?)?;
    if !matches!(term.operator, Operator::Matrix { .. }) {
        return Err(invalid(InvalidCode::ReadingDefinition));
    }
    let columns = super::exact_map::extent(count)?;
    let mut coefficients = reserve(length(count, count)?, budget)?;
    let mut offsets = reserve(length(count, 1)?, budget)?;
    for row in 0..count {
        for column in 0..columns {
            budget.work(1, AT)?;
            let id = *term
                .operands
                .get(length(row, columns)?.saturating_add(length(column, 1)?))
                .ok_or_else(|| invalid(InvalidCode::ReadingShape))?;
            let value = rational(cas, &cas.handle(id), budget)?;
            if column == count {
                offsets.push(value);
            } else {
                coefficients.push(value);
            }
        }
    }
    AffineMap::new(count, count, coefficients, offsets, budget)
}
