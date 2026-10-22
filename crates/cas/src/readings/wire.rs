// SPDX-License-Identifier: AGPL-3.0-only
//! Existing canonical operator records carry all selection and map payloads.

use super::{AffineMap, EntryIndex, EntrySelection};
use crate::budget::BudgetKind;
use crate::encoding::Encoder;
use crate::error::Fault;
use crate::wire_read::Reader;

/// Preserve the source rectangle, result shape and every original index.
pub(crate) fn selection(value: &EntrySelection, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.unsigned(u64::from(value.source_rows))?;
    out.unsigned(u64::from(value.source_columns))?;
    out.byte(u8::from(value.shape.is_some()))?;
    if let Some((rows, columns)) = value.shape {
        out.unsigned(u64::from(rows))?;
        out.unsigned(u64::from(columns))?;
    }
    out.count(value.indices.len())?;
    for index in &value.indices {
        out.unsigned(u64::from(index.row))?;
        out.unsigned(u64::from(index.column))?;
    }
    Ok(())
}

/// The ordinary admission checker revalidates these untrusted decoded indices.
pub(crate) fn read_selection(reader: &mut Reader<'_, '_>) -> Result<EntrySelection, Fault> {
    let source_rows = reader.small()?;
    let source_columns = reader.small()?;
    let shape = if reader.boolean()? {
        Some((reader.small()?, reader.small()?))
    } else {
        None
    };
    let count = reader.count(BudgetKind::MatrixEntries, 16, 8)?;
    let mut indices = Vec::new();
    indices
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(super::AT))?;
    for _ in 0..count {
        indices.push(EntryIndex {
            row: reader.small()?,
            column: reader.small()?,
        });
    }
    Ok(EntrySelection {
        source_rows,
        source_columns,
        indices,
        shape,
    })
}

/// Rational encodings retain every zero, sign and positional coefficient.
pub(crate) fn affine(value: &AffineMap, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.unsigned(u64::from(value.inputs))?;
    out.unsigned(u64::from(value.outputs))?;
    for vector in [&value.coefficients, &value.offsets] {
        out.count(vector.len())?;
        for number in vector {
            out.number(number)?;
        }
    }
    Ok(())
}

/// Decode complete original vectors before any algebra or zero elimination.
pub(crate) fn read_affine(reader: &mut Reader<'_, '_>) -> Result<AffineMap, Fault> {
    let inputs = reader.small()?;
    let outputs = reader.small()?;
    let coefficients = numbers(reader)?;
    let offsets = numbers(reader)?;
    Ok(AffineMap {
        inputs,
        outputs,
        coefficients,
        offsets,
    })
}

/// Each vector is bounded by the existing wire allocation and matrix-entry limits.
fn numbers(reader: &mut Reader<'_, '_>) -> Result<Vec<crate::number::Rational>, Fault> {
    let count = reader.count(BudgetKind::MatrixEntries, 1, 64)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(super::AT))?;
    for _ in 0..count {
        result.push(reader.rational()?);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::{Budget, Limits};
    use crate::model::{AtomicSort, Extent, Field, Operator, Sort};
    use crate::number::Rational;
    use crate::readings::SelectionSpec;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// Mathematical extents and indices consume their full eight-byte structural fields.
    /// ´test:unit:selection-operator-wire-round-trip´
    #[test]
    fn selection_operator_wire_round_trip() -> TestResult {
        let source = Sort::Value(AtomicSort::Matrix {
            field: Field::Real,
            rows: Extent::Literal(2),
            columns: Extent::Literal(3),
        });
        for spec in [
            SelectionSpec::Entry(EntryIndex { row: 1, column: 2 }),
            SelectionSpec::Column(1),
            SelectionSpec::Row(0),
        ] {
            let mut budget = Budget::new(Limits::default())?;
            let value = EntrySelection::new(&source, &spec, &mut budget)?;
            let original = Operator::Select(Box::new(value));
            let mut out = Encoder::new(&mut budget, super::super::AT);
            crate::encoding::operator(&original, &mut out)?;
            let bytes = out.finish();
            let mut reader = Reader::new(&bytes, &mut budget)?;
            assert_eq!(reader.operator()?, original);
            reader.finished()?;
        }
        Ok(())
    }

    /// The private numeric-vector reader and map operator agree on exact field boundaries.
    /// ´test:unit:affine-operator-wire-round-trip´
    #[test]
    fn affine_operator_wire_round_trip() -> TestResult {
        let mut budget = Budget::new(Limits::default())?;
        let negative = Rational::from_ratio(-3, 2, &mut budget)?;
        let value = AffineMap::new(
            2,
            1,
            vec![negative, Rational::one()],
            vec![Rational::zero()],
            &mut budget,
        )?;
        let original = Operator::AffineReading(Box::new(value.clone()));
        let mut out = Encoder::new(&mut budget, super::super::AT);
        crate::encoding::operator(&original, &mut out)?;
        let bytes = out.finish();
        let mut reader = Reader::new(&bytes, &mut budget)?;
        assert_eq!(reader.operator()?, original);
        reader.finished()?;
        let mut vectors = Reader::new(bytes.get(17..).ok_or("coefficient vectors")?, &mut budget)?;
        assert_eq!(numbers(&mut vectors)?, value.coefficients);
        assert_eq!(numbers(&mut vectors)?, value.offsets);
        vectors.finished()?;
        Ok(())
    }
}
