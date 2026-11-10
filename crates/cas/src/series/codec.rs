// SPDX-License-Identifier: AGPL-3.0-only
//! Complete finite payloads, with bounded exact rows and distinct result tags.

use super::check::{AT, invalid, unsupported};
use super::{
    SeriesBinaryRow, SeriesCertificate, SeriesDerivativeRow, SeriesIntegralRow, SeriesProductRow,
    SeriesStatement,
};
use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::error::Fault;
use crate::graph::{Handle, size};
use crate::wire_read::Reader;

/// Reserve source candidate ownership independently of generic supplied-proof behavior.
pub(crate) fn source_workspace(
    value: &crate::proof_model::Certificate,
    budget: &mut Budget,
) -> Result<u64, Fault> {
    let crate::proof_model::Certificate::Series(value) = value else {
        return Err(invalid());
    };
    let SeriesCertificate::Source(value) = value.as_ref() else {
        return Err(invalid());
    };
    super::source::evidence_bytes(value, budget)
}

/// Complete local or portable statement payload, independent of its enclosing record.
#[expect(
    clippy::too_many_lines,
    reason = "the closed statement wire layouts remain adjacent"
)]
pub(crate) fn statement(
    value: &SeriesStatement,
    out: &mut Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    use SeriesStatement as S;
    if let S::EvaluatedResidual {
        subject,
        expression,
        dependent,
        output,
        problem,
        derivative_order,
        order,
    } = value
    {
        for byte in [11, 0, 3, 0] {
            out.byte(byte)?;
        }
        statement(&subject.statement(), out, write)?;
        for reference in [expression, dependent, output, problem] {
            write(reference, out)?;
        }
        crate::wire::index(*derivative_order, out)?;
        crate::wire::index(*order, out)?;
        return Ok(());
    }
    if let S::Source(value) = value {
        out.byte(value.clause.mode())?;
        out.byte(value.clause.form())?;
        out.byte(value.clause.selector())?;
        write(&value.source, out)?;
        write(&value.domain, out)?;
        out.byte(2)?;
        write(&value.output, out)?;
        return Ok(());
    }
    out.byte(match value {
        S::Source(_) | S::EvaluatedResidual { .. } => return Err(invalid()),
        S::Truncate { .. } => 0,
        S::Add { .. } => 1,
        S::Scale { .. } => 2,
        S::Mul { .. } => 3,
        S::Compose { .. } => 5,
        S::Coefficient { .. } => 4,
        S::Reciprocal { .. } => 7,
        S::Derive { .. } => 8,
        S::Integrate { .. } => 9,
    })?;
    out.byte(0)?; // Object form, never a source congruence.
    match value {
        S::Source(_) | S::EvaluatedResidual { .. } => return Err(invalid()),
        S::Reciprocal { input, output }
        | S::Truncate { input, output }
        | S::Derive { input, output } => {
            write(input, out)?;
            out.byte(0)?;
            write(output, out)?;
        }
        S::Add {
            left,
            right,
            output,
            order,
            default_order,
        }
        | S::Compose {
            left,
            right,
            output,
            order,
            default_order,
        }
        | S::Mul {
            left,
            right,
            output,
            order,
            default_order,
        } => {
            write(left, out)?;
            write(right, out)?;
            out.byte(0)?;
            write(output, out)?;
            crate::wire::index(*order, out)?;
            out.byte(u8::from(*default_order))?;
        }
        S::Scale {
            input,
            scalar,
            output,
        }
        | S::Integrate {
            input,
            constant: scalar,
            output,
        } => {
            write(input, out)?;
            write(scalar, out)?;
            out.byte(0)?;
            write(output, out)?;
        }
        S::Coefficient {
            input,
            index,
            value,
        } => {
            write(input, out)?;
            out.byte(1)?;
            write(value, out)?;
            crate::wire::index(*index, out)?;
        }
    }
    Ok(())
}

/// Read only supported forms; result kind is checked before reading a result reference.
#[expect(
    clippy::too_many_lines,
    reason = "the closed mode dispatch retains discriminator order visibly"
)]
pub(crate) fn read_statement(
    reader: &mut Reader<'_, '_>,
    records: &[crate::wire_decode::Decoded],
) -> Result<SeriesStatement, Fault> {
    use crate::wire_decode::object;
    let mode = reader.byte()?;
    let form = reader.byte()?;
    if mode == 11 {
        if form != 0 || reader.byte()? != 3 || reader.byte()? != 0 {
            return Err(unsupported());
        }
        let mode = reader.byte()?;
        let form = reader.byte()?;
        let subject = if mode == 5 && form == 0 {
            let left = object(reader, records)?;
            let right = object(reader, records)?;
            if reader.byte()? != 0 {
                return Err(unsupported());
            }
            super::ResidualSubject::Compose {
                left,
                right,
                output: object(reader, records)?,
                order: reader.index()?,
                default_order: reader.boolean()?,
            }
        } else {
            let clause = super::SourceClause::read(mode, form, reader.byte()?)?;
            let source = object(reader, records)?;
            let domain = object(reader, records)?;
            if reader.byte()? != 2 {
                return Err(unsupported());
            }
            super::ResidualSubject::Source(super::SourceStatement {
                clause,
                source,
                domain,
                output: object(reader, records)?,
            })
        };
        return Ok(SeriesStatement::EvaluatedResidual {
            subject,
            expression: object(reader, records)?,
            dependent: object(reader, records)?,
            output: object(reader, records)?,
            problem: object(reader, records)?,
            derivative_order: reader.index()?,
            order: reader.index()?,
        });
    }
    if mode == 10 || form != 0 {
        let clause = super::SourceClause::read(mode, form, reader.byte()?)?;
        let source = object(reader, records)?;
        let domain = object(reader, records)?;
        if reader.byte()? != 2 {
            return Err(unsupported());
        }
        let output = object(reader, records)?;
        return Ok(SeriesStatement::Source(super::SourceStatement {
            clause,
            source,
            domain,
            output,
        }));
    }
    if !matches!(mode, 0..=5 | 7 | 8 | 9) {
        return Err(unsupported());
    }
    let input = object(reader, records)?;
    let second = if matches!(mode, 1..=3 | 5 | 9) {
        Some(object(reader, records)?)
    } else {
        None
    };
    let tag = reader.byte()?;
    if tag > 1 {
        return Err(unsupported());
    }
    if tag != u8::from(mode == 4) {
        return Err(invalid());
    }
    let output = object(reader, records)?;
    Ok(match mode {
        0 => SeriesStatement::Truncate { input, output },
        1 | 3 | 5 => {
            let right = second.ok_or_else(invalid)?;
            let order = reader.index()?;
            let default_order = reader.boolean()?;
            if mode == 1 {
                SeriesStatement::Add {
                    left: input,
                    right,
                    output,
                    order,
                    default_order,
                }
            } else if mode == 5 {
                SeriesStatement::Compose {
                    left: input,
                    right,
                    output,
                    order,
                    default_order,
                }
            } else {
                SeriesStatement::Mul {
                    left: input,
                    right,
                    output,
                    order,
                    default_order,
                }
            }
        }
        2 => SeriesStatement::Scale {
            input,
            scalar: second.ok_or_else(invalid)?,
            output,
        },
        4 => SeriesStatement::Coefficient {
            input,
            index: reader.index()?,
            value: output,
        },
        7 => SeriesStatement::Reciprocal { input, output },
        8 => SeriesStatement::Derive { input, output },
        9 => SeriesStatement::Integrate {
            input,
            constant: second.ok_or_else(invalid)?,
            output,
        },
        _ => return Err(unsupported()),
    })
}

/// Every row operand is retained; no omitted zero or partial convolution has authority.
#[expect(
    clippy::too_many_lines,
    reason = "the closed certificate layouts remain adjacent"
)]
pub(crate) fn certificate(value: &SeriesCertificate, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.byte(value.mode())?;
    out.byte(value.form())?;
    match value {
        SeriesCertificate::EvaluatedResidual { stage, rows, .. } => {
            out.byte(*stage)?;
            out.count(rows.len())?;
            for row in rows {
                crate::wire::index(row.ordinal, out)?;
                crate::split_wire::numbers(&row.coefficients, out)?;
            }
        }
        SeriesCertificate::Source(value) => {
            out.byte(value.clause.selector())?;
            crate::split_wire::numbers(&value.coefficients, out)?;
            crate::split_wire::numbers(&value.companion, out)?;
            out.count(value.units.len())?;
            for row in &value.units {
                crate::wire::index(row.index, out)?;
                out.number(&row.integer)?;
                out.number(&row.inverse)?;
            }
        }
        SeriesCertificate::Reciprocal {
            constant,
            inverse,
            rows,
            ..
        } => {
            out.number(constant)?;
            out.number(inverse)?;
            out.count(rows.len())?;
            for row in rows {
                product_row(row, out)?;
            }
        }
        SeriesCertificate::Derive { rows, .. } => {
            out.count(rows.len())?;
            for row in rows {
                crate::wire::index(row.index, out)?;
                for value in [&row.input, &row.multiplier, &row.result] {
                    out.number(value)?;
                }
            }
        }
        SeriesCertificate::Integrate { rows, .. } => {
            out.count(rows.len())?;
            for row in rows {
                crate::wire::index(row.index, out)?;
                for value in [&row.integer, &row.inverse, &row.input, &row.result] {
                    out.number(value)?;
                }
            }
        }
        SeriesCertificate::Truncate { coefficients, .. } => {
            crate::split_wire::numbers(coefficients, out)?;
        }
        SeriesCertificate::Add {
            order,
            default_order,
            rows,
            ..
        } => {
            crate::wire::index(*order, out)?;
            out.byte(u8::from(*default_order))?;
            binary_rows(rows, out)?;
        }
        SeriesCertificate::Scale { rows, .. } => binary_rows(rows, out)?,
        SeriesCertificate::Mul {
            order,
            default_order,
            rows,
            ..
        } => {
            crate::wire::index(*order, out)?;
            out.byte(u8::from(*default_order))?;
            out.count(rows.len())?;
            for row in rows {
                crate::wire::index(row.degree, out)?;
                crate::wire::index(row.left_index, out)?;
                crate::wire::index(row.right_index, out)?;
                for value in [&row.left, &row.right, &row.product, &row.sum] {
                    out.number(value)?;
                }
            }
        }
        SeriesCertificate::Compose {
            order,
            default_order,
            zero,
            seed,
            powers,
            sums,
            ..
        } => {
            crate::wire::index(*order, out)?;
            out.byte(u8::from(*default_order))?;
            out.number(zero)?;
            crate::split_wire::numbers(seed, out)?;
            out.count(powers.len())?;
            for power in powers {
                crate::wire::index(power.exponent, out)?;
                product_row(&power.row, out)?;
            }
            out.count(sums.len())?;
            for row in sums {
                product_row(row, out)?;
            }
        }
        SeriesCertificate::Coefficient { index, value, .. } => {
            crate::wire::index(*index, out)?;
            out.number(value)?;
        }
    }
    Ok(())
}

/// Ordered coefficientwise rows share the exact same closed field sequence.
fn binary_rows(rows: &[SeriesBinaryRow], out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.count(rows.len())?;
    for row in rows {
        crate::wire::index(row.index, out)?;
        for value in [&row.left, &row.right, &row.result] {
            out.number(value)?;
        }
    }
    Ok(())
}

/// Bound row counts before allocation using the existing cumulative coefficient dimension.
#[expect(
    clippy::too_many_lines,
    reason = "The closed mode decoder keeps every admitted payload layout together"
)]
pub(crate) fn read_certificate(reader: &mut Reader<'_, '_>) -> Result<SeriesCertificate, Fault> {
    let mode = reader.byte()?;
    let form = reader.byte()?;
    if mode == 11 {
        if form != 0 || reader.byte()? != 0 {
            return Err(unsupported());
        }
        let count = reader.count(BudgetKind::JetEntries, 8, 32)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(count)
            .map_err(|_| Fault::allocation(AT))?;
        for _ in 0..count {
            let ordinal = reader.index()?;
            let count = reader.count(BudgetKind::JetEntries, 9, 96)?;
            reader
                .budget
                .specialized(BudgetKind::JetEntries, size(count, AT)?, true, AT)?;
            let mut coefficients = Vec::new();
            coefficients
                .try_reserve_exact(count)
                .map_err(|_| Fault::allocation(AT))?;
            for _ in 0..count {
                coefficients.push(reader.rational()?);
            }
            rows.push(super::ResidualRow {
                ordinal,
                coefficients,
            });
        }
        return Ok(SeriesCertificate::EvaluatedResidual {
            form,
            stage: 0,
            rows,
        });
    }
    if mode == 10 || form != 0 {
        let clause = super::SourceClause::read(mode, form, reader.byte()?)?;
        let coefficients = read_coefficients(reader)?;
        let companion = read_coefficients(reader)?;
        let count = reader.count(BudgetKind::JetEntries, 22, 192)?;
        let mut units = Vec::new();
        units
            .try_reserve_exact(count)
            .map_err(|_| Fault::allocation(AT))?;
        for _ in 0..count {
            units.push(super::SourceUnitRow {
                index: reader.index()?,
                integer: reader.rational()?,
                inverse: reader.rational()?,
            });
        }
        return Ok(SeriesCertificate::Source(super::SourceCertificate {
            clause,
            coefficients,
            companion,
            units,
        }));
    }
    if !matches!(mode, 0..=5 | 7 | 8 | 9) || form != 0 {
        return Err(unsupported());
    }
    Ok(match mode {
        0 => SeriesCertificate::Truncate {
            form,
            coefficients: read_coefficients(reader)?,
        },
        1 => {
            let order = reader.index()?;
            let default_order = reader.boolean()?;
            SeriesCertificate::Add {
                form,
                order,
                default_order,
                rows: read_binary(reader)?,
            }
        }
        2 => SeriesCertificate::Scale {
            form,
            rows: read_binary(reader)?,
        },
        3 => {
            let order = reader.index()?;
            let default_order = reader.boolean()?;
            let count = reader.count(BudgetKind::JetEntries, 48, 256)?;
            let mut rows = Vec::new();
            rows.try_reserve_exact(count)
                .map_err(|_| Fault::allocation(AT))?;
            for _ in 0..count {
                rows.push(SeriesProductRow {
                    degree: reader.index()?,
                    left_index: reader.index()?,
                    right_index: reader.index()?,
                    left: reader.rational()?,
                    right: reader.rational()?,
                    product: reader.rational()?,
                    sum: reader.rational()?,
                });
            }
            SeriesCertificate::Mul {
                form,
                order,
                default_order,
                rows,
            }
        }
        4 => SeriesCertificate::Coefficient {
            form,
            index: reader.index()?,
            value: reader.rational()?,
        },
        5 => {
            let order = reader.index()?;
            let default_order = reader.boolean()?;
            let zero = reader.rational()?;
            let seed = read_coefficients(reader)?;
            let count = reader.count(BudgetKind::JetEntries, 52, 320)?;
            let mut powers = Vec::new();
            powers
                .try_reserve_exact(count)
                .map_err(|_| Fault::allocation(AT))?;
            for _ in 0..count {
                powers.push(super::SeriesPowerRow {
                    exponent: reader.index()?,
                    row: read_product(reader)?,
                });
            }
            let count = reader.count(BudgetKind::JetEntries, 48, 256)?;
            let mut sums = Vec::new();
            sums.try_reserve_exact(count)
                .map_err(|_| Fault::allocation(AT))?;
            for _ in 0..count {
                sums.push(read_product(reader)?);
            }
            SeriesCertificate::Compose {
                form,
                order,
                default_order,
                zero,
                seed,
                powers,
                sums,
            }
        }
        7 => {
            let constant = reader.rational()?;
            let inverse = reader.rational()?;
            let count = reader.count(BudgetKind::JetEntries, 48, 256)?;
            let mut rows = Vec::new();
            rows.try_reserve_exact(count)
                .map_err(|_| Fault::allocation(AT))?;
            for _ in 0..count {
                rows.push(read_product(reader)?);
            }
            SeriesCertificate::Reciprocal {
                form,
                constant,
                inverse,
                rows,
            }
        }
        8 => {
            let count = reader.count(BudgetKind::JetEntries, 31, 192)?;
            let mut rows = Vec::new();
            rows.try_reserve_exact(count)
                .map_err(|_| Fault::allocation(AT))?;
            for _ in 0..count {
                rows.push(SeriesDerivativeRow {
                    index: reader.index()?,
                    input: reader.rational()?,
                    multiplier: reader.rational()?,
                    result: reader.rational()?,
                });
            }
            SeriesCertificate::Derive { form, rows }
        }
        9 => {
            let count = reader.count(BudgetKind::JetEntries, 40, 256)?;
            let mut rows = Vec::new();
            rows.try_reserve_exact(count)
                .map_err(|_| Fault::allocation(AT))?;
            for _ in 0..count {
                rows.push(SeriesIntegralRow {
                    index: reader.index()?,
                    integer: reader.rational()?,
                    inverse: reader.rational()?,
                    input: reader.rational()?,
                    result: reader.rational()?,
                });
            }
            SeriesCertificate::Integrate { form, rows }
        }
        _ => return Err(unsupported()),
    })
}

/// Reserve all row containers before importing any variable-size rational components.
fn read_binary(reader: &mut Reader<'_, '_>) -> Result<Vec<SeriesBinaryRow>, Fault> {
    let count = reader.count(BudgetKind::JetEntries, 31, 192)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        rows.push(SeriesBinaryRow {
            index: reader.index()?,
            left: reader.rational()?,
            right: reader.rational()?,
            result: reader.rational()?,
        });
    }
    Ok(rows)
}

/// Meter the full owned row copy before generic proof admission clones it.
pub(crate) fn measure(value: &SeriesCertificate, budget: &mut Budget) -> Result<(), Fault> {
    let count = match value {
        SeriesCertificate::EvaluatedResidual { rows, .. } => {
            // Empty coefficient vectors still require one visit per supplied row.
            budget.work(size(rows.len(), AT)?, AT)?;
            rows.iter().try_fold(0_usize, |total, row| {
                total
                    .checked_add(row.coefficients.len())
                    .ok_or_else(invalid)
            })?
        }
        SeriesCertificate::Source(value) => value
            .coefficients
            .len()
            .checked_add(value.companion.len())
            .and_then(|n| n.checked_add(value.units.len()))
            .ok_or_else(invalid)?,
        SeriesCertificate::Truncate { coefficients, .. } => coefficients.len(),
        SeriesCertificate::Add { rows, .. } | SeriesCertificate::Scale { rows, .. } => rows.len(),
        SeriesCertificate::Mul { rows, .. } => rows.len(),
        SeriesCertificate::Reciprocal { rows, .. } => {
            rows.len().checked_add(2).ok_or_else(invalid)?
        }
        SeriesCertificate::Compose {
            seed, powers, sums, ..
        } => seed
            .len()
            .checked_add(powers.len())
            .and_then(|n| n.checked_add(sums.len()))
            .ok_or_else(invalid)?,
        SeriesCertificate::Coefficient { .. } => 1,
        SeriesCertificate::Derive { rows, .. } => rows.len(),
        SeriesCertificate::Integrate { rows, .. } => rows.len(),
    };
    budget.check(BudgetKind::JetEntries, size(count, AT)?, AT)?;
    let mut out = Encoder::new(budget, AT);
    certificate(value, &mut out)?;
    let bytes = size(out.finish().len(), AT)?
        .checked_mul(3)
        .and_then(|n| n.checked_add(size(count, AT).ok()?.checked_mul(256)?))
        .ok_or_else(invalid)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(bytes, AT)
}

/// Prepay the finite coefficient vector with the series entry dimension.
fn read_coefficients(reader: &mut Reader<'_, '_>) -> Result<Vec<crate::number::Rational>, Fault> {
    let count = reader.count(BudgetKind::JetEntries, 9, 96)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        values.push(reader.rational()?);
    }
    Ok(values)
}

/// Exact ordered fields of a composition multiplication and partial sum.
fn product_row(row: &SeriesProductRow, out: &mut Encoder<'_>) -> Result<(), Fault> {
    crate::wire::index(row.degree, out)?;
    crate::wire::index(row.left_index, out)?;
    crate::wire::index(row.right_index, out)?;
    for value in [&row.left, &row.right, &row.product, &row.sum] {
        out.number(value)?;
    }
    Ok(())
}

/// Decode one row after its enclosing vector has been prepaid.
fn read_product(reader: &mut Reader<'_, '_>) -> Result<SeriesProductRow, Fault> {
    Ok(SeriesProductRow {
        degree: reader.index()?,
        left_index: reader.index()?,
        right_index: reader.index()?,
        left: reader.rational()?,
        right: reader.rational()?,
        product: reader.rational()?,
        sum: reader.rational()?,
    })
}
