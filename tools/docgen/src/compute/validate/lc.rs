// SPDX-License-Identifier: AGPL-3.0-only
//! Independent population-only validation of LC summary ledgers.

use super::super::schema::{Dependency, LcLedger, Operation, OperationCounts, Scope, SummaryCase};

/// Checked count arithmetic, independent of compute's implementation.
fn multiply(a: u64, b: u64) -> Result<u64, String> {
    a.checked_mul(b)
        .filter(|n| *n < (1_u64 << 53))
        .ok_or_else(|| "LC count overflow".to_owned())
}

/// Checked sums preserve the common wire domain.
fn add(a: u64, b: u64) -> Result<u64, String> {
    a.checked_add(b)
        .filter(|n| *n < (1_u64 << 53))
        .ok_or_else(|| "LC count overflow".to_owned())
}

/// Independently reconstruct every population-derived preparation and storage field.
fn expected(operation: Operation, ledger: &LcLedger) -> Result<LcLedger, String> {
    let p = &ledger.input_populations;
    if p.contains(&0) {
        return Err("empty LC population".to_owned());
    }
    let mut result = LcLedger {
        input_populations: p.clone(),
        cycle_population: ledger.cycle_population,
        preparation: OperationCounts::default(),
        traversal: OperationCounts::default(),
        input_bytes: multiply(p.iter().copied().try_fold(ledger.cycle_population, add)?, 8)?,
        prepared_bytes: 0,
        root_scratch_bytes: 0,
        output_bytes: 0,
        root_reuse: [0; 2],
        reciprocal_reuse: 0,
        preparation_bytes: 0,
        traversal_bytes: 0,
    };
    let (outputs, traffic) = match (operation, p.as_slice(), ledger.cycle_population) {
        (Operation::ForwardFused | Operation::ForwardSeparate, [l, c, times, phases], 0) => {
            let pairs = multiply(*l, *c)?;
            let repeats = multiply(*times, *phases)?;
            let outputs = multiply(pairs, repeats)?;
            result.preparation.roots = add(*l, *c)?;
            result.preparation.divisions = pairs;
            result.preparation.multiplications = pairs;
            if operation == Operation::ForwardFused {
                result.traversal.fmas = outputs;
            } else {
                result.traversal.multiplications = outputs;
                result.traversal.additions = outputs;
            }
            result.prepared_bytes = multiply(pairs, 8)?;
            result.root_scratch_bytes = multiply(add(*l, *c)?, 8)?;
            result.root_reuse = [*c, *l];
            result.reciprocal_reuse = repeats;
            result.preparation_bytes = multiply(add(add(*l, *c)?, pairs)?, 24)?;
            (outputs, 32)
        }
        (Operation::Inductance | Operation::Capacitance, [products, denominators], 0) => {
            let outputs = multiply(*products, *denominators)?;
            if *products > 1 {
                result.preparation.divisions = *denominators;
                result.traversal.multiplications = outputs;
                result.prepared_bytes = multiply(*denominators, 8)?;
                result.reciprocal_reuse = *products;
                result.preparation_bytes = multiply(*denominators, 16)?;
            } else {
                result.traversal.divisions = outputs;
            }
            (outputs, 24)
        }
        (Operation::InverseTime, [phases, 1, 1], cycles) if cycles > 0 => {
            let outputs = multiply(*phases, cycles)?;
            result.preparation.divisions = 1;
            result.traversal.multiplications = multiply(outputs, 3)?;
            result.traversal.additions = outputs;
            result.prepared_bytes = 8;
            result.reciprocal_reuse = outputs;
            result.preparation_bytes = 16;
            (outputs, 40)
        }
        _ => return Err("LC populations".to_owned()),
    };
    result.output_bytes = multiply(outputs, 8)?;
    result.preparation_bytes = add(result.preparation_bytes, result.output_bytes)?;
    result.traversal_bytes = multiply(outputs, traffic)?;
    Ok(result)
}

/// Unknown, partial and complete records all retain a valid static recipe ledger.
pub(super) fn identity(record: &SummaryCase) -> Result<(), String> {
    if record.recipe.dependency != Dependency::PreparedTraversal {
        return if record.recipe.lc_ledger.is_none() {
            Ok(())
        } else {
            Err("unexpected LC ledger".to_owned())
        };
    }
    let ledger = record
        .recipe
        .lc_ledger
        .as_ref()
        .ok_or_else(|| "missing LC ledger".to_owned())?;
    if *ledger != expected(record.recipe.operation, ledger)? {
        return Err("LC ledger".to_owned());
    }
    let shape_matches = if record.recipe.operation == Operation::InverseTime {
        record.recipe.shape.as_slice()
            == [
                ledger.input_populations.first().copied().unwrap_or(0),
                ledger.cycle_population,
            ]
    } else {
        record.recipe.shape == ledger.input_populations
    };
    if !shape_matches {
        return Err("LC shape".to_owned());
    }
    Ok(())
}

/// Named primitive counts supply the conventional FLOP numerator exactly once.
fn floating(counts: OperationCounts) -> Result<u64, String> {
    [
        counts.roots,
        counts.divisions,
        counts.multiplications,
        counts.additions,
        multiply(counts.fmas, 2)?,
    ]
    .into_iter()
    .try_fold(0, add)
}

/// Complete LC rate numerators follow the validated ledger and declared scope.
pub(super) fn numerators(
    record: &SummaryCase,
    analysis: &super::super::schema::SummaryAnalysis,
) -> Result<(), String> {
    identity(record)?;
    let ledger = record
        .recipe
        .lc_ledger
        .as_ref()
        .ok_or_else(|| "missing LC ledger".to_owned())?;
    let length = record
        .accepted_length
        .ok_or_else(|| "LC length".to_owned())?;
    let preparation = match record.recipe.scope {
        Scope::Prepared => false,
        Scope::PreparationInclusive => true,
        Scope::RestorationInclusiveRepeated => return Err("LC scope".to_owned()),
    };
    let operations = add(
        floating(ledger.traversal)?,
        if preparation {
            floating(ledger.preparation)?
        } else {
            0
        },
    )?;
    let bytes = add(
        ledger.traversal_bytes,
        if preparation {
            ledger.preparation_bytes
        } else {
            0
        },
    )?;
    if record.recipe.workers != 1
        || record.recipe.width != super::super::schema::Width::Scalar
        || analysis.elements_per_second.numerator != multiply(ledger.output_bytes / 8, length)?
        || analysis
            .floating_operations_per_second
            .as_ref()
            .map(|rate| rate.numerator)
            != Some(multiply(operations, length)?)
        || analysis
            .logical_bytes_per_second
            .as_ref()
            .map(|rate| rate.numerator)
            != Some(multiply(bytes, length)?)
        || analysis.vector_operations_per_second.is_some()
    {
        return Err("LC rate numerators".to_owned());
    }
    Ok(())
}
