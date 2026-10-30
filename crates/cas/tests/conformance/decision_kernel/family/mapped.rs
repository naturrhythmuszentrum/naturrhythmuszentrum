// SPDX-License-Identifier: AGPL-3.0-only
//! Exact affine definitions transport a complete source family and reject changed coefficients.

use super::{Fixture, TestResult, certificate, family, ledger, rational};
use nrz_cas::{
    FaultKind, InvalidCode, Location,
    decision::{RationalEnclosure, ScalarKernelLaw, ScalarKernelReading, ScalarKernelRecord},
    readings::AffineMap,
    syntax::Operator,
};

/// An actual scalar reading definition keeps its original free source and explicit family premises.
fn mapped(scale: i64) -> TestResult<(Fixture, RationalEnclosure)> {
    let (mut source, mut request) = family()?;
    let native = certificate(&mut request)?.records[0].clone();
    let ScalarKernelLaw::Family(input) = &native.law else {
        return Err("family input".into());
    };
    let result = source.term(Operator::Free("b".into()), &[])?;
    let scale = rational(scale, 1)?;
    let offset = rational(1, 1)?;
    let map = AffineMap::new(
        1,
        1,
        vec![scale.clone()],
        vec![offset.clone()],
        &mut ledger()?,
    )?;
    let definition =
        source
            .cas
            .define_reading(&result, &input.family.source, &map, &mut ledger()?)?;
    let endpoints = [
        scale
            .mul(&rational(1, 1)?, &mut ledger()?)?
            .add(&offset, &mut ledger()?)?,
        scale
            .mul(&rational(2, 1)?, &mut ledger()?)?
            .add(&offset, &mut ledger()?)?,
    ];
    let [lower, upper] = if scale.is_negative() {
        [endpoints[1].clone(), endpoints[0].clone()]
    } else {
        endpoints
    };
    request.target = definition.clone();
    request.lower = lower.clone();
    request.upper = upper.clone();
    let candidate = certificate(&mut request)?;
    candidate.version = 4;
    candidate.expression = definition.clone();
    candidate.records = vec![
        native,
        ScalarKernelRecord {
            law: ScalarKernelLaw::Reading(Box::new(ScalarKernelReading {
                definition,
                input: 0,
                scale,
                offset,
            })),
            lower,
            upper,
        },
    ];
    Ok((source, request))
}

/// A positive scale transports every member and retains both original assumptions.
/// ´test:integration:decision-kernel-mapped-family-receipt´
/// ´claim:cas:decision-kernel-mapped-family-receipt´
#[test]
fn decision_kernel_mapped_family_receipt() -> TestResult {
    receipt(2, [10_233, 10_232])
}

/// A negative scale reverses the exact endpoint images while preserving complete family coverage.
/// ´test:integration:decision-kernel-mapped-family-negative-scale´
/// ´claim:cas:decision-kernel-mapped-family-negative-scale´
#[test]
fn decision_kernel_mapped_family_negative_scale() -> TestResult {
    receipt(-2, [10_297, 10_296])
}

/// Independent unchanged-limit ledgers measure exact affine receipt ownership.
fn receipt(scale: i64, boundaries: [u64; 2]) -> TestResult {
    for work in boundaries {
        let (mut source, request) = mapped(scale)?;
        let before = source.cas.counts();
        let mut budget = nrz_cas::limits::Budget::new(nrz_cas::limits::Limits {
            work,
            ..nrz_cas::limits::Limits::default()
        })?;
        let limits = budget.limits().clone();
        budget.work(17, Location::new("prior mapped receipt"))?;
        let result = source.cas.check_rational_enclosure(&request, &mut budget);
        if work == boundaries[0] {
            let receipt = result?;
            assert_eq!(receipt.contract(), &request);
            assert_eq!(receipt.input_premises().count(), 2);
            assert_eq!(budget.usage().work, work);
        } else {
            let Err(fault) = result else {
                panic!("one-below mapped receipt must exhaust Work");
            };
            let FaultKind::Exhausted {
                budget: kind,
                limit,
                needed,
                ..
            } = fault.kind
            else {
                panic!("expected mapped receipt Work exhaustion");
            };
            assert_eq!(kind, nrz_cas::limits::BudgetKind::Work);
            assert_eq!(limit, work);
            assert_eq!(needed, boundaries[0]);
            assert_eq!(fault.location, Location::new("exact rational enclosure"));
            assert!(budget.usage().work >= 17);
        }
        assert_eq!(source.cas.counts(), before);
        assert_eq!(budget.limits(), &limits);
    }
    Ok(())
}

/// A changed scale refuses against its retained definition before affine arithmetic replay.
/// ´test:integration:decision-kernel-mapped-family-scale-forgery´
/// ´claim:cas:decision-kernel-mapped-family-scale-forgery´
#[test]
fn decision_kernel_mapped_family_scale_forgery() -> TestResult {
    forgery(false)
}

/// A changed offset refuses against the original declared map even when the source family agrees.
/// ´test:integration:decision-kernel-mapped-family-offset-forgery´
/// ´claim:cas:decision-kernel-mapped-family-offset-forgery´
#[test]
fn decision_kernel_mapped_family_offset_forgery() -> TestResult {
    forgery(true)
}

/// Mutating only one coefficient must name the map checker and preserve the original graph.
fn forgery(offset: bool) -> TestResult {
    let (mut source, mut request) = mapped(2)?;
    let ScalarKernelLaw::Reading(value) = &mut certificate(&mut request)?.records[1].law else {
        return Err("reading law".into());
    };
    if offset {
        value.offset = rational(2, 1)?;
    } else {
        value.scale = rational(3, 1)?;
    }
    let before = source.cas.counts();
    let mut budget = ledger()?;
    budget.work(17, Location::new("prior mapped forgery"))?;
    let Err(fault) = source.cas.check_rational_enclosure(&request, &mut budget) else {
        panic!("a changed declared map coefficient must refuse");
    };
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(fault.location, Location::new("scalar kernel reading map"));
    assert_eq!(source.cas.counts(), before);
    assert_eq!(budget.usage().work, 4688);
    Ok(())
}
