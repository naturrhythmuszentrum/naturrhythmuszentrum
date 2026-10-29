// SPDX-License-Identifier: AGPL-3.0-only
//! Orthogonal nearest proofs retain whole boxes, exact ties and distinct convention identity.

use super::{Selection, TestResult, exact, ledger, number, prepared, skew};
use nrz_cas::lattice::{
    LatticeConvention, LatticeEnclosure, LatticeSelectionOutcome, LatticeUndecidedReason,
};
use nrz_cas::limits::{Budget, Limits};
use nrz_cas::proof::{Judgment, Statement, VerifiedWitness};
use nrz_cas::{Cas, FaultKind, InvalidCode, Rational};

pub(super) fn selected(source: &mut Selection, budget: &mut Budget) -> TestResult<VerifiedWitness> {
    Ok(source.source.frame.cas.prove_lattice_nearest_selection(
        &source.cover,
        &source.expected,
        &source.enclosure,
        budget,
    )?)
}

/// Grid half-ties have rational original targets and exact independent basis coordinates.
pub(super) fn tie(simultaneous: bool) -> TestResult<Selection> {
    let mut source = prepared(5)?;
    let ratios = [(5, 16), if simultaneous { (7, 32) } else { (1, 5) }];
    let mut coordinates = Vec::new();
    let mut entries = Vec::new();
    for ratio in ratios {
        let target = number(&mut source.source, ratio.0, ratio.1, false)?;
        coordinates.push(exact(&source, target.clone(), ratio)?);
        entries.push(target);
    }
    source.expected.target = super::column(&mut source.source, &entries)?;
    let mut values = Vec::new();
    for value in [3, if simultaneous { 4 } else { 3 }] {
        values.push(number(&mut source.source, value, 1, true)?);
    }
    source.expected.values = values;
    let LatticeEnclosure::Original(enclosure) = &mut source.enclosure else {
        return Err("missing grid box".into());
    };
    enclosure
        .original
        .target
        .clone_from(&source.expected.target);
    enclosure.original.coordinates = coordinates;
    enclosure.bounds.clear();
    for (n, d) in [(5, 2), if simultaneous { (7, 2) } else { (16, 5) }] {
        let value = Rational::from_ratio(n, d, &mut ledger()?)?;
        enclosure.bounds.push((value.clone(), value));
    }
    Ok(source)
}

/// Grid floor and nearest agree on the tuple while portable expected claims remain distinct.
/// ´test:integration:lattice-nearest-convention´
#[test]
fn lattice_nearest_convention() -> TestResult {
    let mut source = prepared(5)?;
    let floor = super::super::selected(&mut source, &mut ledger()?)?;
    let nearest = selected(&mut source, &mut ledger()?)?;
    assert_ne!(floor.judgment().statement, nearest.judgment().statement);
    assert_eq!(
        nearest.judgment().statement,
        Statement::LatticeNearestSelection(Box::new(source.expected.clone()))
    );
    let bytes = source
        .source
        .frame
        .cas
        .export_witness(&nearest, &mut ledger()?)?;
    let mut receiver = Cas::new(Limits::default())?;
    let inspected = receiver.inspect(&bytes, &mut ledger()?)?;
    assert!(inspected.is_discharged());
    assert_eq!(receiver.export_witness(&inspected, &mut ledger()?)?, bytes);
    let citations = receiver.proof_citations(inspected.proof(), &mut ledger()?)?;
    let schema = citations
        .last()
        .ok_or("missing nearest citation")?
        .auxiliary_schema(&receiver)?
        .ok_or("missing nearest schema")?;
    assert_eq!(
        (schema.name, schema.version),
        ("decision.lattice-orthogonal-nearest-selection", 1)
    );
    let expected = Judgment {
        context: source.expected.cover.index.basis.context.clone(),
        layer: nearest.judgment().layer,
        space: None,
        residuals: Vec::new(),
        statement: floor.judgment().statement.clone(),
    };
    let before = source.source.frame.cas.counts();
    let fault = source
        .source
        .frame
        .cas
        .replay(&bytes, &expected, &mut ledger()?)
        .err()
        .ok_or("nearest replayed as floor")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(fault.location.operation, "typed proof admission");
    assert_eq!(source.source.frame.cas.counts(), before);
    Ok(())
}

/// Included half-integer faces and simultaneous ties select the greater original indices.
/// ´test:integration:lattice-nearest-ties´
#[test]
fn lattice_nearest_ties() -> TestResult {
    for simultaneous in [false, true] {
        let mut source = tie(simultaneous)?;
        let witness = selected(&mut source, &mut ledger()?)?;
        let output = source
            .source
            .frame
            .cas
            .lattice_selection_outputs(&witness, &mut ledger()?)?;
        assert_eq!(output.tuple(), source.expected.values);
        assert!(output.identity().is_discharged());
        assert_eq!(
            source
                .source
                .frame
                .cas
                .materialize_lattice_count(&witness, 0, 3, &mut ledger()?)?,
            3
        );
        assert_eq!(
            source
                .source
                .frame
                .cas
                .materialize_lattice_count(&witness, 1, 4, &mut ledger()?)?,
            if simultaneous { 4 } else { 3 }
        );
        source.expected.values[0] = number(&mut source.source, 2, 1, true)?;
        let before = source.source.frame.cas.counts();
        let fault = selected(&mut source, &mut ledger()?)
            .err()
            .ok_or("smaller tie owner accepted")?;
        let fault = fault
            .downcast_ref::<nrz_cas::Fault>()
            .ok_or("wrong fault type")?;
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::DecisionBoundaryIntersection)
        );
        assert_eq!(fault.location.operation, "lattice tuple selection");
        assert_eq!(source.source.frame.cas.counts(), before);
    }
    Ok(())
}

/// Skew coordinate rounding has no authority without a proved nearest route.
/// ´test:integration:lattice-nearest-skew-refusal´
#[test]
fn lattice_nearest_skew_refusal() -> TestResult {
    let source = skew()?;
    let before = source.source.frame.cas.counts();
    let mut budget = ledger()?;
    let fault = source
        .source
        .frame
        .cas
        .prepare_lattice_nearest_selection(
            &source.cover,
            &source.expected,
            &source.enclosure,
            &mut budget,
        )
        .err()
        .ok_or("skew rounding admitted")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedCertificate)
    );
    assert_eq!(fault.location.operation, "lattice orthogonal nearest");
    assert!(budget.usage().work > 0);
    assert_eq!(source.source.frame.cas.counts(), before);
    Ok(())
}

/// A valid original box crossing a nearest face remains undecided with its retained evidence.
/// ´test:integration:lattice-nearest-undecided´
#[test]
fn lattice_nearest_undecided() -> TestResult {
    let mut source = tie(false)?;
    let LatticeEnclosure::Original(enclosure) = &mut source.enclosure else {
        return Err("missing grid enclosure".into());
    };
    enclosure.original.coordinates[0].lower = Rational::from_ratio(3, 10, &mut ledger()?)?;
    enclosure.bounds[0].0 = Rational::from_ratio(12, 5, &mut ledger()?)?;
    let attempt = source.source.frame.cas.prepare_lattice_nearest_selection(
        &source.cover,
        &source.expected,
        &source.enclosure,
        &mut ledger()?,
    )?;
    assert_eq!(attempt.convention(), LatticeConvention::Nearest);
    assert!(attempt.cut(0).is_none());
    let before = source.source.frame.cas.counts();
    let LatticeSelectionOutcome::Undecided(value) = source
        .source
        .frame
        .cas
        .select_lattice_attempt(&attempt, &mut ledger()?)?
    else {
        return Err("nearest straddle selected".into());
    };
    assert_eq!(value.reason, LatticeUndecidedReason::BoundaryIntersection);
    assert_eq!(value.evidence.candidate(), &source.expected);
    assert_eq!(source.source.frame.cas.counts(), before);
    Ok(())
}
