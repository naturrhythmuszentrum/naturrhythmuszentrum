// SPDX-License-Identifier: AGPL-3.0-only
//! Literal fresh-store caller boundaries retain consumed prefixes and publish no refusal prefix.

use super::{TestResult, convert, fixture};
use nrz_bindings::{Budget, Limits, readings::algebra::Location};
use nrz_scalar::Scalar;

/// Work and storage ceilings have both-format N/N-1 and already-consumed-prefix twins.
/// ´test:integration:quantity-frequency-resource-boundaries´
#[test]
fn quantity_frequency_resource_boundaries() -> TestResult {
    boundaries::<f32>(82_168, 176, 96_596)?;
    boundaries::<f64>(82_232, 184, 3)
}

fn boundaries<P: Scalar + 'static>(staged: u64, output: u64, output_prefix: u64) -> TestResult {
    for (kind, required, prefix) in [
        (0, 96_596, 96_595),
        (1, staged, 3),
        (2, output, output_prefix),
    ] {
        attempt::<P>(kind, required, 0, (true, 96_596))?;
        attempt::<P>(kind, required - 1, 0, (false, prefix))?;
    }
    attempt::<P>(0, 96_596, 1, (false, 96_596))
}

/// Every candidate owns a fresh schema; refusal leaves values and graph accounting unchanged.
fn attempt<P: Scalar + 'static>(
    kind: u8,
    available: u64,
    prior: u64,
    expected: (bool, u64),
) -> TestResult {
    let mut limits = Limits::default();
    match kind {
        0 => limits.work = available,
        1 => limits.staged_payload_bytes = available,
        2 => limits.output_bytes = available,
        _ => return Err("boundary kind".into()),
    }
    let mut case = fixture::<P>(true)?;
    let before = case.heap.provenance_usage();
    let stamp = case.input.view(&case.heap)?.stamp();
    let mut budget = Budget::new(limits)?;
    budget.work(prior, Location::new("prior frequency caller"))?;
    let result = convert(&mut case, &mut budget);
    assert_eq!(result.is_ok(), expected.0, "{result:?}");
    assert_eq!(budget.usage().work, expected.1);
    if expected.0 {
        result?;
        assert_eq!(case.heap.provenance_usage().bytes(), 1_433);
    } else {
        assert_eq!(case.heap.provenance_usage(), before);
        let mut retry = Budget::new(Limits::default())?;
        convert(&mut case, &mut retry)?;
        assert_eq!(retry.usage().work, 96_596);
    }
    assert_eq!(case.input.view(&case.heap)?.stamp(), stamp);
    Ok(())
}

/// Final provenance admission refuses a whole result even after coefficient and symbol preparation.
/// ´test:integration:quantity-frequency-provenance-boundaries´
#[test]
fn quantity_frequency_provenance_boundaries() -> TestResult {
    provenance::<f32>()?;
    provenance::<f64>()
}

fn provenance<P: Scalar + 'static>() -> TestResult {
    for admitted in [false, true] {
        let mut case = fixture::<P>(true)?;
        case.heap
            .set_provenance_limits(nrz_matrix::ProvenanceLimits::new(
                1024,
                16_384,
                65_536,
                if admitted { 1_433 } else { 1_432 },
                1_048_576,
            ))?;
        let before = case.heap.provenance_usage();
        let stamp = case.input.view(&case.heap)?.stamp();
        let mut budget = Budget::new(Limits::default())?;
        let result = convert(&mut case, &mut budget);
        assert_eq!(result.is_ok(), admitted, "{result:?}");
        assert_eq!(budget.usage().work, 96_596);
        if admitted {
            result?;
            assert_eq!(case.heap.provenance_usage().bytes(), 1_433);
        } else {
            assert_eq!(case.heap.provenance_usage(), before);
        }
        assert_eq!(case.input.view(&case.heap)?.stamp(), stamp);
    }
    Ok(())
}
