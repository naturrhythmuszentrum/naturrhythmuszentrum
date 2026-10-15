// SPDX-License-Identifier: AGPL-3.0-only
//! Internal deterministic structural encodings with metered writes.

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, Location};
use crate::model::{
    AtomicSort, Binding, Branch, Environment, Extent, Field, Operator, PredicateKind,
    RootSelection, Scope, Sort,
};

/// A bounded byte sink used for keys, never Rust `Debug` serialization.
#[derive(Debug)]
pub(crate) struct Encoder<'a> {
    /// Accumulated bytes.
    bytes: Vec<u8>,
    /// Shared call ledger.
    budget: &'a mut Budget,
    /// Operation location.
    at: Location,
    /// Byte ceiling appropriate to admission or public export.
    limit: BudgetKind,
}

impl<'a> Encoder<'a> {
    /// Current bounded payload length before a reference is appended.
    pub(crate) const fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Start an empty structural key.
    pub(crate) const fn new(budget: &'a mut Budget, at: Location) -> Self {
        Self {
            bytes: Vec::new(),
            budget,
            at,
            limit: BudgetKind::PayloadBytes,
        }
    }

    /// Start a public export whose allocations are capped before each write.
    pub(crate) const fn output(budget: &'a mut Budget, at: Location) -> Self {
        Self {
            bytes: Vec::new(),
            budget,
            at,
            limit: BudgetKind::OutputBytes,
        }
    }

    /// Start a portable proof export bounded before every allocation.
    pub(crate) const fn witness(budget: &'a mut Budget, at: Location) -> Self {
        Self {
            bytes: Vec::new(),
            budget,
            at,
            limit: BudgetKind::WitnessBytes,
        }
    }

    /// Append already encoded bytes after checking length and work.
    pub(crate) fn raw(&mut self, bytes: &[u8]) -> Result<(), Fault> {
        let next = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, self.at))?;
        let size =
            u64::try_from(next).map_err(|_| Fault::capacity(BudgetKind::PayloadBytes, self.at))?;
        self.budget.check(self.limit, size, self.at)?;
        self.budget
            .check(BudgetKind::StagedPayloadBytes, size, self.at)?;
        self.budget.work(
            u64::try_from(bytes.len()).map_err(|_| Fault::capacity(BudgetKind::Work, self.at))?,
            self.at,
        )?;
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|_| Fault::allocation(self.at))?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    /// Append one fixed tag.
    pub(crate) fn byte(&mut self, value: u8) -> Result<(), Fault> {
        self.raw(&[value])
    }

    /// Append a little-endian fixed-width unsigned value.
    pub(crate) fn unsigned(&mut self, value: u64) -> Result<(), Fault> {
        self.raw(&value.to_le_bytes())
    }

    /// Append a checked collection length.
    pub(crate) fn count(&mut self, value: usize) -> Result<(), Fault> {
        let value =
            u32::try_from(value).map_err(|_| Fault::capacity(BudgetKind::PayloadBytes, self.at))?;
        self.raw(&value.to_le_bytes())
    }

    /// Append one bounded declaration key.
    pub(crate) fn name(&mut self, value: &str) -> Result<(), Fault> {
        self.budget.check(
            BudgetKind::NameBytes,
            u64::try_from(value.len())
                .map_err(|_| Fault::capacity(BudgetKind::NameBytes, self.at))?,
            self.at,
        )?;
        self.count(value.len())?;
        self.raw(value.as_bytes())
    }

    /// Finish the immutable key.
    pub(crate) fn finish(self) -> Vec<u8> {
        self.bytes
    }

    /// Encode an exact number using the same canonical magnitude representation.
    pub(crate) fn number(&mut self, value: &crate::number::Rational) -> Result<(), Fault> {
        let bytes = value.structural_bytes(self.budget)?;
        self.raw(&bytes)
    }
}

/// Encode a complete field key.
pub(crate) fn field(value: &Field, out: &mut Encoder<'_>) -> Result<(), Fault> {
    match value {
        Field::Rational => out.byte(0),
        Field::Real => out.byte(1),
        Field::Complex => out.byte(2),
        Field::PositiveCharacteristic(p) => {
            out.byte(3)?;
            out.unsigned(u64::from(*p))
        }
        Field::Named(name) => {
            out.byte(4)?;
            out.name(name)
        }
    }
}

/// Encode a branch convention without conflating named policies.
fn branch(value: &Branch, out: &mut Encoder<'_>) -> Result<(), Fault> {
    match value {
        Branch::StrictReal => out.byte(0),
        Branch::PrincipalComplex => out.byte(1),
        Branch::Declared(name) => {
            out.byte(2)?;
            out.name(name)
        }
    }
}

/// Encode an extent parameter or literal.
fn extent(value: &Extent, out: &mut Encoder<'_>) -> Result<(), Fault> {
    match value {
        Extent::Literal(value) => {
            out.byte(0)?;
            out.unsigned(u64::from(*value))
        }
        Extent::Parameter(key) => {
            out.byte(1)?;
            out.name(key)
        }
    }
}

/// Encode one nonrecursive mathematical sort.
pub(crate) fn atomic(value: &AtomicSort, out: &mut Encoder<'_>) -> Result<(), Fault> {
    match value {
        AtomicSort::Integer => out.byte(0),
        AtomicSort::Scalar(value) => {
            out.byte(1)?;
            field(value, out)
        }
        AtomicSort::Boolean => out.byte(2),
        AtomicSort::Matrix {
            field: value,
            rows,
            columns,
        } => {
            out.byte(3)?;
            field(value, out)?;
            extent(rows, out)?;
            extent(columns, out)
        }
    }
}

/// Encode a flat sort and its ordered function signature.
pub(crate) fn sort(value: &Sort, out: &mut Encoder<'_>) -> Result<(), Fault> {
    match value {
        Sort::Test => out.byte(3),
        Sort::Distribution => out.byte(4),
        Sort::Value(value) => {
            out.byte(0)?;
            atomic(value, out)
        }
        Sort::Function { inputs, output } => {
            out.byte(1)?;
            out.count(inputs.len())?;
            for input in inputs {
                atomic(input, out)?;
            }
            atomic(output, out)
        }
        Sort::Set(value) => {
            out.byte(2)?;
            atomic(value, out)
        }
    }
}

/// Encode a context header whose declarations have already been sorted.
pub(crate) fn environment(value: &Environment, out: &mut Encoder<'_>) -> Result<(), Fault> {
    field(&value.field, out)?;
    branch(&value.branch, out)?;
    out.count(value.declarations.len())?;
    for declaration in &value.declarations {
        out.name(&declaration.key)?;
        out.name(&declaration.display_name)?;
        sort(&declaration.sort, out)?;
    }
    Ok(())
}

/// Encode all scope frames and binder slot sorts.
pub(crate) fn scope(value: &Scope, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.count(value.len())?;
    for frame in value {
        out.count(frame.len())?;
        for slot in frame {
            atomic(slot, out)?;
        }
    }
    Ok(())
}

/// Encode the semantic binder frame; occurrence display names are excluded.
pub(crate) fn binding(value: Option<&Binding>, out: &mut Encoder<'_>) -> Result<(), Fault> {
    let Some(value) = value else {
        return out.byte(0);
    };
    out.byte(1)?;
    out.count(value.sorts.len())?;
    for slot in &value.sorts {
        atomic(slot, out)?;
    }
    out.count(value.body_operands.len())?;
    for position in &value.body_operands {
        out.unsigned(u64::from(*position))?;
    }
    Ok(())
}

/// Encode a predicate's exact kind and any named key.
pub(crate) fn predicate(value: &PredicateKind, out: &mut Encoder<'_>) -> Result<(), Fault> {
    let tag = match value {
        PredicateKind::Defined => 0,
        PredicateKind::NonZero => 1,
        PredicateKind::Positive => 2,
        PredicateKind::NonNegative => 3,
        PredicateKind::IntegerRange => 4,
        PredicateKind::Differentiable => 5,
        PredicateKind::Equal => 6,
        PredicateKind::Symmetric => 7,
        PredicateKind::And => 8,
        PredicateKind::Or => 9,
        PredicateKind::Not => 10,
        PredicateKind::ForAll => 11,
        PredicateKind::Named(_) => 12,
    };
    out.byte(tag)?;
    if let PredicateKind::Named(name) = value {
        out.name(name)?;
    }
    Ok(())
}

/// Stable operator tags are independent of Rust's enum layout.
const fn operator_tag(value: &Operator) -> u8 {
    match value {
        Operator::Rational(_) => 0,
        Operator::Free(_) => 1,
        Operator::Bound { .. } => 2,
        Operator::Constant(_) => 3,
        Operator::Neg => 4,
        Operator::Add => 5,
        Operator::Sub => 6,
        Operator::Mul => 7,
        Operator::Div => 8,
        Operator::IntegerPower(_) => 9,
        Operator::GeneralPower(_) => 10,
        Operator::Call(_) => 11,
        Operator::Derivative(_) => 12,
        Operator::Integral { .. } => 13,
        Operator::Sum => 14,
        Operator::Series { .. } => 15,
        Operator::Limit(_) => 16,
        Operator::Predicate(_) => 17,
        Operator::Set { .. } => 18,
        Operator::Matrix { .. } => 19,
        Operator::Comprehension { .. } => 20,
        Operator::Transpose => 21,
        Operator::MatrixMultiply => 22,
        Operator::Spectral(_) => 23,
        Operator::AlgebraicRoot { .. } => 24,
        Operator::MatrixMap(_) => 25,
        Operator::IntegerEmbed => 26,
        Operator::Entrywise(_) => 27,
        Operator::Conjugate => 43,
        Operator::RealPart => 44,
        Operator::ImagPart => 45,
        Operator::RootValue { .. } => 46,
        Operator::ExactCoefficient => 47,
        Operator::Cases => 28,
        Operator::NamedCase(_) => 29,
        Operator::RegularEmbed => 30,
        Operator::DistributionZero => 31,
        Operator::Delta { .. } => 32,
        Operator::DistributionAdd => 33,
        Operator::DistributionScale => 34,
        Operator::DistributionDerivative { .. } => 35,
        Operator::SmoothMultiply => 36,
        Operator::Pair => 37,
        Operator::PairNearSupport => 38,
        Operator::TestDerivative { .. } => 39,
        Operator::TestMultiply => 40,
        Operator::TestApply => 41,
        Operator::RegularAction => 42,
    }
}

/// Encode every operator payload using explicit v1 discriminants.
pub(crate) fn operator(value: &Operator, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.byte(operator_tag(value))?;
    match value {
        Operator::RootValue { slot } => crate::wire::index(*slot, out),
        Operator::NamedCase(value) => crate::named_case::encode(value, out),
        Operator::Delta { order }
        | Operator::DistributionDerivative { order }
        | Operator::TestDerivative { order } => out.unsigned(u64::from(*order)),
        Operator::Rational(number) => out.number(number),
        Operator::Free(name) | Operator::Call(name) | Operator::Derivative(name) => out.name(name),
        Operator::Bound { depth, slot } => {
            out.unsigned(u64::from(*depth))?;
            out.unsigned(u64::from(*slot))
        }
        Operator::Constant(value) => out.byte(match value {
            crate::model::Constant::Pi => 0,
            crate::model::Constant::E => 1,
        }),
        Operator::IntegerPower(value) => {
            out.byte(u8::from(value.is_negative()))?;
            out.unsigned(value.unsigned_abs())
        }
        Operator::GeneralPower(value) => branch(value, out),
        Operator::Integral { definite } => out.byte(u8::from(*definite)),
        Operator::Series { order, analytic } => {
            out.unsigned(u64::from(*order))?;
            out.byte(u8::from(*analytic))
        }
        Operator::Limit(value) => out.byte(approach_tag(*value)),
        Operator::Predicate(value) => predicate(value, out),
        Operator::Set { complete } => out.byte(u8::from(*complete)),
        Operator::Matrix { rows, columns } => {
            out.unsigned(u64::from(*rows))?;
            out.unsigned(u64::from(*columns))
        }
        Operator::Comprehension { rows, columns } => {
            extent(rows, out)?;
            extent(columns, out)
        }
        Operator::Spectral(value) => out.byte(spectral_tag(*value)),
        Operator::AlgebraicRoot {
            variable,
            field: value,
            multiplicity,
            selection,
        } => {
            out.name(variable)?;
            field(value, out)?;
            out.byte(u8::from(multiplicity.is_some()))?;
            if let Some(multiplicity) = multiplicity {
                out.unsigned(u64::from(*multiplicity))?;
            }
            root_selection(selection, out)
        }
        Operator::MatrixMap(value) => out.byte(matrix_operation_tag(*value)),
        Operator::Entrywise(value) => out.byte(match value {
            crate::model::ScalarOperation::Add => 0,
            crate::model::ScalarOperation::Multiply => 1,
            crate::model::ScalarOperation::Divide => 2,
        }),
        Operator::Conjugate
        | Operator::RealPart
        | Operator::ImagPart
        | Operator::ExactCoefficient
        | Operator::Neg
        | Operator::Add
        | Operator::Sub
        | Operator::Mul
        | Operator::Div
        | Operator::Sum
        | Operator::Transpose
        | Operator::MatrixMultiply
        | Operator::IntegerEmbed
        | Operator::Cases
        | Operator::RegularEmbed
        | Operator::DistributionZero
        | Operator::DistributionAdd
        | Operator::DistributionScale
        | Operator::SmoothMultiply
        | Operator::Pair
        | Operator::PairNearSupport
        | Operator::TestMultiply
        | Operator::TestApply
        | Operator::RegularAction => Ok(()),
    }
}

/// Stable approach tags retain the two distinct infinities and explicit paths.
const fn approach_tag(value: crate::model::Approach) -> u8 {
    use crate::model::Approach;

    match value {
        Approach::Both => 0,
        Approach::Below => 1,
        Approach::Above => 2,
        Approach::PositiveInfinity => 3,
        Approach::NegativeInfinity => 4,
        Approach::Path => 5,
    }
}

/// Stable spectral tags preserve each requested mathematical relation.
const fn spectral_tag(value: crate::model::SpectralKind) -> u8 {
    use crate::model::SpectralKind;

    match value {
        SpectralKind::Least => 0,
        SpectralKind::Greatest => 1,
        SpectralKind::AlgebraicMultiplicity => 2,
        SpectralKind::GeometricMultiplicity => 3,
        SpectralKind::Eigenvectors => 4,
    }
}

/// Stable tags distinguish scalar matrix maps from matrix-valued maps.
const fn matrix_operation_tag(value: crate::model::MatrixOperation) -> u8 {
    use crate::model::MatrixOperation;

    match value {
        MatrixOperation::Determinant => 0,
        MatrixOperation::Trace => 1,
        MatrixOperation::Adjugate => 2,
        MatrixOperation::Inverse => 3,
    }
}

/// Encode the selected-root request independently of its defining polynomial.
fn root_selection(value: &RootSelection, out: &mut Encoder<'_>) -> Result<(), Fault> {
    match value {
        RootSelection::All => out.byte(0),
        RootSelection::Ordered(index) => {
            out.byte(1)?;
            out.unsigned(u64::from(*index))
        }
        RootSelection::Predicate(name) => {
            out.byte(2)?;
            out.name(name)
        }
    }
}
