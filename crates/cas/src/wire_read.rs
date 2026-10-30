// SPDX-License-Identifier: AGPL-3.0-only
//! Checked finite binary reads matching the explicit structural encoding tags.

use num_bigint::BigUint;

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::size;
use crate::model::{
    Approach, AtomicSort, Binding, Branch, Constant, Declaration, Environment, Extent, Field,
    MatrixOperation, Operator, PredicateKind, RootSelection, ScalarOperation, Scope, Sort,
    SpectralKind,
};
use crate::number::Rational;

/// Decoder location, never interpolating hostile bytes into an error.
const AT: Location = Location::new("witness decoding");

/// Malformed or nonminimal input is never normalized into acceptance.
pub(crate) const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Encoding, AT)
}

/// A reader with checked offsets and a cumulative allocation bound.
pub(crate) struct Reader<'a, 'b> {
    /// Complete finite input.
    bytes: &'a [u8],
    /// Next byte position.
    position: usize,
    /// Shared charged call ledger.
    pub(crate) budget: &'b mut Budget,
    /// Conservative decoded record allocation estimate.
    allocated: u64,
}

impl<'a, 'b> Reader<'a, 'b> {
    /// Bound the whole stream before any decoding allocation.
    pub(crate) fn new(bytes: &'a [u8], budget: &'b mut Budget) -> Result<Self, Fault> {
        budget.check(BudgetKind::WitnessBytes, size(bytes.len(), AT)?, AT)?;
        Ok(Self {
            bytes,
            position: 0,
            budget,
            allocated: 0,
        })
    }

    /// Consume an exact byte span after checked addition and range validation.
    pub(crate) fn take(&mut self, count: usize) -> Result<&'a [u8], Fault> {
        let end = self.position.checked_add(count).ok_or_else(invalid)?;
        let value = self.bytes.get(self.position..end).ok_or_else(invalid)?;
        self.budget.work(size(count, AT)?, AT)?;
        self.position = end;
        Ok(value)
    }

    /// Read one byte.
    pub(crate) fn byte(&mut self) -> Result<u8, Fault> {
        self.take(1)?.first().copied().ok_or_else(invalid)
    }

    /// Read a strict Boolean/optional tag.
    pub(crate) fn boolean(&mut self) -> Result<bool, Fault> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(invalid()),
        }
    }

    /// Read a fixed-width local index or count.
    pub(crate) fn index(&mut self) -> Result<u32, Fault> {
        let bytes: [u8; 4] = self.take(4)?.try_into().map_err(|_| invalid())?;
        Ok(u32::from_le_bytes(bytes))
    }

    /// Read an unsigned mathematical payload.
    pub(crate) fn unsigned(&mut self) -> Result<u64, Fault> {
        let bytes: [u8; 8] = self.take(8)?.try_into().map_err(|_| invalid())?;
        Ok(u64::from_le_bytes(bytes))
    }

    /// Structural encodings use u64 for bounded u32 mathematical payloads.
    pub(crate) fn small(&mut self) -> Result<u32, Fault> {
        u32::try_from(self.unsigned()?).map_err(|_| invalid())
    }

    /// Reserve cumulative decoded memory before building a vector or owned string.
    pub(crate) fn allocation(&mut self, bytes: u64) -> Result<(), Fault> {
        self.allocated = self
            .allocated
            .checked_add(bytes)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        self.budget
            .check(BudgetKind::StagedPayloadBytes, self.allocated, AT)?;
        self.budget.work(bytes, AT)
    }

    /// Check a vector's count against both a semantic ceiling and available input bytes.
    pub(crate) fn count(
        &mut self,
        kind: BudgetKind,
        minimum: u64,
        item_bytes: u64,
    ) -> Result<usize, Fault> {
        let count = u64::from(self.index()?);
        if kind.solving_cumulative() == Some(false) {
            self.budget.solving(kind, count, AT)?;
        } else {
            self.budget.check(kind, count, AT)?;
        }
        let needed = count.checked_mul(minimum).ok_or_else(invalid)?;
        let remaining = self
            .bytes
            .len()
            .checked_sub(self.position)
            .ok_or_else(invalid)?;
        if needed > size(remaining, AT)? {
            return Err(invalid());
        }
        self.allocation(
            count
                .checked_mul(item_bytes)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?,
        )?;
        usize::try_from(count).map_err(|_| invalid())
    }

    /// Read a bounded UTF-8 declaration key before copying it.
    pub(crate) fn name(&mut self) -> Result<String, Fault> {
        let count = self.count(BudgetKind::NameBytes, 1, 2)?;
        let value = std::str::from_utf8(self.take(count)?).map_err(|_| invalid())?;
        let mut result = String::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| Fault::allocation(AT))?;
        result.push_str(value);
        Ok(result)
    }

    /// Refuse trailing bytes even after a valid prefix.
    pub(crate) const fn finished(&self) -> Result<(), Fault> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(invalid())
        }
    }

    /// Read and verify reduced canonical magnitudes before constructing a rational.
    pub(crate) fn rational(&mut self) -> Result<Rational, Fault> {
        let start = self.position;
        let negative = self.boolean()?;
        let numerator = self.magnitude()?;
        let denominator = self.magnitude()?;
        let value = Rational::new(negative, &numerator, &denominator, self.budget)?;
        let canonical = value.structural_bytes(self.budget)?;
        if self.bytes.get(start..self.position) != Some(canonical.as_slice()) {
            return Err(invalid());
        }
        Ok(value)
    }

    /// Check minimal byte length and component bits before bigint allocation.
    fn magnitude(&mut self) -> Result<BigUint, Fault> {
        let count = self.count(BudgetKind::WitnessBytes, 1, 2)?;
        let bytes = self.take(count)?;
        let bits = if let Some(last) = bytes.last() {
            if *last == 0 {
                return Err(invalid());
            }
            size(count.checked_sub(1).ok_or_else(invalid)?, AT)?
                .checked_mul(8)
                .and_then(|n| n.checked_add(u64::from(8_u32.checked_sub(last.leading_zeros())?)))
                .ok_or_else(invalid)?
        } else {
            0
        };
        self.budget.check(BudgetKind::ComponentBits, bits, AT)?;
        self.budget.check(BudgetKind::IntermediateBits, bits, AT)?;
        Ok(BigUint::from_bytes_le(bytes))
    }

    /// Decode a complete field convention.
    pub(crate) fn field(&mut self) -> Result<Field, Fault> {
        match self.byte()? {
            0 => Ok(Field::Rational),
            1 => Ok(Field::Real),
            2 => Ok(Field::Complex),
            3 => Ok(Field::PositiveCharacteristic(self.small()?)),
            4 => Ok(Field::Named(self.name()?)),
            _ => Err(invalid()),
        }
    }

    /// Decode explicit branch policy tags.
    fn branch(&mut self) -> Result<Branch, Fault> {
        match self.byte()? {
            0 => Ok(Branch::StrictReal),
            1 => Ok(Branch::PrincipalComplex),
            2 => Ok(Branch::Declared(self.name()?)),
            _ => Err(invalid()),
        }
    }

    /// Decode literal or declaration-based extents.
    fn extent(&mut self) -> Result<Extent, Fault> {
        match self.byte()? {
            0 => Ok(Extent::Literal(self.small()?)),
            1 => Ok(Extent::Parameter(self.name()?)),
            _ => Err(invalid()),
        }
    }

    /// Decode a flat atomic sort, avoiding recursive type syntax.
    pub(crate) fn atomic(&mut self) -> Result<AtomicSort, Fault> {
        match self.byte()? {
            0 => Ok(AtomicSort::Integer),
            1 => Ok(AtomicSort::Scalar(self.field()?)),
            2 => Ok(AtomicSort::Boolean),
            3 => Ok(AtomicSort::Matrix {
                field: self.field()?,
                rows: self.extent()?,
                columns: self.extent()?,
            }),
            _ => Err(invalid()),
        }
    }

    /// Decode a finite signature or value/set sort.
    pub(crate) fn sort(&mut self) -> Result<Sort, Fault> {
        match self.byte()? {
            0 => Ok(Sort::Value(self.atomic()?)),
            1 => Ok(Sort::Function {
                inputs: self.atomics()?,
                output: self.atomic()?,
            }),
            2 => Ok(Sort::Set(self.atomic()?)),
            3 => Ok(Sort::Test),
            4 => Ok(Sort::Distribution),
            _ => Err(invalid()),
        }
    }

    /// Decode a bounded list of atomic slots.
    fn atomics(&mut self) -> Result<Vec<AtomicSort>, Fault> {
        let count = self.count(BudgetKind::Vertices, 1, 128)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| Fault::allocation(AT))?;
        for _ in 0..count {
            result.push(self.atomic()?);
        }
        Ok(result)
    }

    /// Decode declarations before any context-dependent expression is admitted.
    pub(crate) fn environment(&mut self) -> Result<Environment, Fault> {
        let field = self.field()?;
        let branch = self.branch()?;
        let count = self.count(BudgetKind::Vertices, 9, 256)?;
        let mut declarations = Vec::new();
        declarations
            .try_reserve_exact(count)
            .map_err(|_| Fault::allocation(AT))?;
        for _ in 0..count {
            declarations.push(Declaration {
                key: self.name()?,
                display_name: self.name()?,
                sort: self.sort()?,
            });
        }
        Ok(Environment {
            field,
            branch,
            declarations,
        })
    }

    /// Decode outer-to-inner frames with an explicit depth ceiling.
    pub(crate) fn scope(&mut self) -> Result<Scope, Fault> {
        let count = self.count(BudgetKind::Depth, 4, 32)?;
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(count)
            .map_err(|_| Fault::allocation(AT))?;
        for _ in 0..count {
            frames.push(self.atomics()?);
        }
        Ok(frames)
    }

    /// Decode the optional binder frame and ordered body positions.
    pub(crate) fn binding(&mut self) -> Result<Option<Binding>, Fault> {
        if !self.boolean()? {
            return Ok(None);
        }
        let sorts = self.atomics()?;
        let count = self.count(BudgetKind::Edges, 8, 8)?;
        let mut body_operands = Vec::new();
        body_operands
            .try_reserve_exact(count)
            .map_err(|_| Fault::allocation(AT))?;
        for _ in 0..count {
            body_operands.push(self.small()?);
        }
        Ok(Some(Binding {
            sorts,
            body_operands,
        }))
    }

    /// Decode all represented predicate discriminants exhaustively.
    pub(crate) fn predicate(&mut self) -> Result<PredicateKind, Fault> {
        match self.byte()? {
            0 => Ok(PredicateKind::Defined),
            1 => Ok(PredicateKind::NonZero),
            2 => Ok(PredicateKind::Positive),
            3 => Ok(PredicateKind::NonNegative),
            4 => Ok(PredicateKind::IntegerRange),
            5 => Ok(PredicateKind::Differentiable),
            6 => Ok(PredicateKind::Equal),
            7 => Ok(PredicateKind::Symmetric),
            8 => Ok(PredicateKind::And),
            9 => Ok(PredicateKind::Or),
            10 => Ok(PredicateKind::Not),
            11 => Ok(PredicateKind::ForAll),
            12 => Ok(PredicateKind::Named(self.name()?)),
            _ => Err(invalid()),
        }
    }

    /// Read sign/magnitude i64 including its minimum, while rejecting negative zero.
    fn exponent(&mut self) -> Result<i64, Fault> {
        let negative = self.boolean()?;
        let magnitude = self.unsigned()?;
        if negative {
            if magnitude == 0 {
                return Err(invalid());
            }
            if magnitude == i64::MIN.unsigned_abs() {
                return Ok(i64::MIN);
            }
            i64::try_from(magnitude)
                .ok()
                .and_then(i64::checked_neg)
                .ok_or_else(invalid)
        } else {
            i64::try_from(magnitude).map_err(|_| invalid())
        }
    }

    /// Decode every represented operator without relying on Rust enum layout.
    pub(crate) fn operator(&mut self) -> Result<Operator, Fault> {
        match self.byte()? {
            49 => Ok(Operator::Select(Box::new(
                crate::readings::wire::read_selection(self)?,
            ))),
            50 => Ok(Operator::AffineReading(Box::new(
                crate::readings::wire::read_affine(self)?,
            ))),
            48 => Ok(Operator::SymbolRelation(match self.byte()? {
                0 => crate::model::SymbolRelation::Copy,
                1 => crate::model::SymbolRelation::DeterministicBound,
                2 => crate::model::SymbolRelation::NumericalError,
                _ => return Err(invalid()),
            })),
            43 => Ok(Operator::Conjugate),
            44 => Ok(Operator::RealPart),
            45 => Ok(Operator::ImagPart),
            46 => Ok(Operator::RootValue {
                slot: self.index()?,
            }),
            47 => Ok(Operator::ExactCoefficient),
            0 => Ok(Operator::Rational(self.rational()?)),
            1 => Ok(Operator::Free(self.name()?)),
            2 => Ok(Operator::Bound {
                depth: self.small()?,
                slot: self.small()?,
            }),
            3 => Ok(Operator::Constant(match self.byte()? {
                0 => Constant::Pi,
                1 => Constant::E,
                _ => return Err(invalid()),
            })),
            4 => Ok(Operator::Neg),
            5 => Ok(Operator::Add),
            6 => Ok(Operator::Sub),
            7 => Ok(Operator::Mul),
            8 => Ok(Operator::Div),
            9 => Ok(Operator::IntegerPower(self.exponent()?)),
            tag => self.extended_operator(tag),
        }
    }

    /// Decode represented calculus, matrix and predicate operator payloads separately.
    fn extended_operator(&mut self, tag: u8) -> Result<Operator, Fault> {
        match tag {
            30 => Ok(Operator::RegularEmbed),
            31 => Ok(Operator::DistributionZero),
            32 => Ok(Operator::Delta {
                order: self.small()?,
            }),
            33 => Ok(Operator::DistributionAdd),
            34 => Ok(Operator::DistributionScale),
            35 => Ok(Operator::DistributionDerivative {
                order: self.small()?,
            }),
            36 => Ok(Operator::SmoothMultiply),
            37 => Ok(Operator::Pair),
            38 => Ok(Operator::PairNearSupport),
            39 => Ok(Operator::TestDerivative {
                order: self.small()?,
            }),
            40 => Ok(Operator::TestMultiply),
            41 => Ok(Operator::TestApply),
            42 => Ok(Operator::RegularAction),
            28 => Ok(Operator::Cases),
            29 => Ok(Operator::NamedCase(Box::new(crate::named_case::decode(
                self,
            )?))),
            51 => Ok(Operator::NaturalLog),
            52 => Ok(Operator::Exponential),
            53 => Ok(Operator::Sigmoid),
            10 => Ok(Operator::GeneralPower(self.branch()?)),
            11 => Ok(Operator::Call(self.name()?)),
            12 => Ok(Operator::Derivative(self.name()?)),
            13 => Ok(Operator::Integral {
                definite: self.boolean()?,
            }),
            14 => Ok(Operator::Sum),
            15 => Ok(Operator::Series {
                order: self.small()?,
                analytic: self.boolean()?,
            }),
            16 => Ok(Operator::Limit(self.approach()?)),
            17 => Ok(Operator::Predicate(self.predicate()?)),
            18 => Ok(Operator::Set {
                complete: self.boolean()?,
            }),
            19 => Ok(Operator::Matrix {
                rows: self.small()?,
                columns: self.small()?,
            }),
            20 => Ok(Operator::Comprehension {
                rows: self.extent()?,
                columns: self.extent()?,
            }),
            21 => Ok(Operator::Transpose),
            22 => Ok(Operator::MatrixMultiply),
            23 => Ok(Operator::Spectral(self.spectral()?)),
            24 => self.algebraic_root(),
            25 => Ok(Operator::MatrixMap(self.matrix_operation()?)),
            26 => Ok(Operator::IntegerEmbed),
            27 => Ok(Operator::Entrywise(match self.byte()? {
                0 => ScalarOperation::Add,
                1 => ScalarOperation::Multiply,
                2 => ScalarOperation::Divide,
                _ => return Err(invalid()),
            })),
            _ => Err(invalid()),
        }
    }

    /// Preserve finite directions, signed infinities and explicit paths.
    fn approach(&mut self) -> Result<Approach, Fault> {
        match self.byte()? {
            0 => Ok(Approach::Both),
            1 => Ok(Approach::Below),
            2 => Ok(Approach::Above),
            3 => Ok(Approach::PositiveInfinity),
            4 => Ok(Approach::NegativeInfinity),
            5 => Ok(Approach::Path),
            _ => Err(invalid()),
        }
    }

    /// Decode every spectral request independently of whether its algorithm exists.
    fn spectral(&mut self) -> Result<SpectralKind, Fault> {
        match self.byte()? {
            0 => Ok(SpectralKind::Least),
            1 => Ok(SpectralKind::Greatest),
            2 => Ok(SpectralKind::AlgebraicMultiplicity),
            3 => Ok(SpectralKind::GeometricMultiplicity),
            4 => Ok(SpectralKind::Eigenvectors),
            _ => Err(invalid()),
        }
    }

    /// Decode matrix maps whose result sorts are checked during admission.
    fn matrix_operation(&mut self) -> Result<MatrixOperation, Fault> {
        match self.byte()? {
            0 => Ok(MatrixOperation::Determinant),
            1 => Ok(MatrixOperation::Trace),
            2 => Ok(MatrixOperation::Adjugate),
            3 => Ok(MatrixOperation::Inverse),
            _ => Err(invalid()),
        }
    }

    /// Decode the complete algebraic-root selection payload.
    fn algebraic_root(&mut self) -> Result<Operator, Fault> {
        let variable = self.name()?;
        let field = self.field()?;
        let multiplicity = if self.boolean()? {
            Some(self.small()?)
        } else {
            None
        };
        let selection = match self.byte()? {
            0 => RootSelection::All,
            1 => RootSelection::Ordered(self.small()?),
            2 => RootSelection::Predicate(self.name()?),
            _ => return Err(invalid()),
        };
        Ok(Operator::AlgebraicRoot {
            variable,
            field,
            multiplicity,
            selection,
        })
    }
}
