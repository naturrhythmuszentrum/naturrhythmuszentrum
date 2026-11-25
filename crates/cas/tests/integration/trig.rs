// SPDX-License-Identifier: AGPL-3.0-only
//! Original syntax and explicitly assumed claims survive fresh portable replay.

use crate::testing::common::{TestResult, default_budget};
use crate::testing::trig::{Fixture, operators};
use crate::testing::trig_replay::{prepared, witness};
use nrz_cas::domain::ContextDraft;
use nrz_cas::proof::{Judgment, Primitive};
use nrz_cas::source::WalkEvent;
use nrz_cas::syntax::{Draft, Operator, Substitution};
use nrz_cas::{Cas, FaultKind, InvalidCode, Reference};

/// Round trips retain checked projections and explicit hypotheses without proving a new law.
/// ´test:integration:trig-fresh-replay´
#[test]
fn trig_fresh_replay() -> TestResult {
    for (operator, _) in operators() {
        let (origin, proof) = witness(operator.clone())?;
        let bytes = origin.export_witness(&proof, &mut default_budget()?)?;
        assert_eq!(bytes.get(..8), Some(b"NRZCAS05".as_slice()));
        let (mut destination, expected) = prepared(operator, true)?;
        let replayed = destination.replay(&bytes, &expected, &mut default_budget()?)?;
        assert_eq!(replayed.judgment(), &expected);
        assert_eq!(replayed.hypotheses().len(), 1);
        assert_eq!(
            destination.export_witness(&replayed, &mut default_budget()?)?,
            bytes
        );
    }
    Ok(())
}

/// Shared unary syntax keeps every original use, including an unreduced zero divisor.
/// ´test:integration:trig-original-occurrences´
#[test]
fn trig_original_occurrences() -> TestResult {
    let mut f = Fixture::new()?;
    for (operator, name) in operators() {
        let zero = f.term(Operator::Rational(nrz_cas::Rational::zero()), &[])?;
        let quotient = f.term(Operator::Div, &[f.x.clone(), zero.clone()])?;
        let node = f.term(operator.clone(), &[quotient])?;
        hidden_zero(&mut f, &node, &zero)?;
        let sum = f.term(Operator::Add, &[node.clone(), node.clone()])?;
        let source = f
            .cas
            .admit(
                &[Draft::Source {
                    key: name.into(),
                    text: "original".into(),
                }],
                &mut default_budget()?,
            )?
            .pop()
            .ok_or("source")?;
        let root = f
            .cas
            .materialize_original(&sum, &source, &mut default_budget()?)?;
        let before = f.cas.walk_original(&root, &mut default_budget()?)?;
        let visits: Vec<_> = before
            .events
            .iter()
            .filter_map(|event| match event {
                WalkEvent::Visit(visit) if visit.expression == node => Some(visit),
                _ => None,
            })
            .collect();
        assert_eq!(visits.len(), 2);
        assert_ne!(visits[0].occurrence, visits[1].occurrence);
        let y = f.term(Operator::Free("y".into()), &[])?;
        let output = f.cas.substitute(
            &sum,
            &[
                Substitution {
                    key: "x".into(),
                    replacement: y.clone(),
                },
                Substitution {
                    key: "y".into(),
                    replacement: f.x.clone(),
                },
            ],
            &mut default_budget()?,
        )?;
        let nodes = f.cas.operands(&output, &mut default_budget()?)?;
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0], nodes[1]);
        assert_eq!(
            f.cas
                .expression(&nodes[0], &mut crate::testing::budgets::graph_budget()?)?
                .0,
            &operator
        );
        let quotient = f
            .cas
            .operands(&nodes[0], &mut default_budget()?)?
            .pop()
            .ok_or("quotient")?;
        assert_eq!(
            f.cas.operands(&quotient, &mut default_budget()?)?.first(),
            Some(&y)
        );
        assert_eq!(
            format!("{:?}", before.events),
            format!(
                "{:?}",
                f.cas.walk_original(&root, &mut default_budget()?)?.events
            )
        );
    }
    Ok(())
}

/// Mutations of complete accepted witnesses fail before any imported prefix is published.
/// ´test:integration:trig-replay-mutations´
#[test]
fn trig_replay_mutations() -> TestResult {
    let (origin, proof) = witness(Operator::Sine)?;
    let bytes = origin.export_witness(&proof, &mut default_budget()?)?;
    let records = crate::testing::answer::records(&bytes)?;
    // Empty scope, absent space, present ambient and scalar sort put the operator at byte 22.
    let expressions: Vec<_> = records
        .iter()
        .enumerate()
        .filter(|(_, (tag, _))| *tag == 3)
        .collect();
    let (_, (_, sine)) = expressions
        .iter()
        .find(|(_, (_, range))| bytes.get(range.start + 22) == Some(&58))
        .ok_or("sine record")?;
    let (complex, (_, source)) = expressions
        .iter()
        .find(|(_, (_, range))| bytes.get(range.start + 22) == Some(&1))
        .ok_or("Complex source")?;
    assert_eq!(
        bytes.get(source.start + 19..source.start + 22),
        Some([0, 1, 2].as_slice())
    );
    assert_eq!(
        bytes.get(sine.start + 23..sine.start + 28),
        Some([0, 1, 0, 0, 0].as_slice())
    );
    let mut unknown = bytes.clone();
    *unknown.get_mut(sine.start + 22).ok_or("operator")? = 61;
    let mut changed = bytes.clone();
    changed
        .get_mut(sine.start + 28..sine.start + 32)
        .ok_or("operand")?
        .copy_from_slice(&u32::try_from(*complex)?.to_le_bytes());
    let (cosine_store, cosine) = witness(Operator::Cosine)?;
    let cosine = cosine_store.export_witness(&cosine, &mut default_budget()?)?;
    // The changed known constructor remains valid for its own independent expectation.
    let (mut control, expected) = prepared(Operator::Cosine, true)?;
    control.replay(&cosine, &expected, &mut default_budget()?)?;
    for reordered in [false, true] {
        let (mut destination, expected) = prepared(Operator::Sine, reordered)?;
        refused(&mut destination, &expected, &unknown, InvalidCode::Encoding)?;
        refused(
            &mut destination,
            &expected,
            &cosine,
            InvalidCode::EndpointMismatch,
        )?;
        refused(&mut destination, &expected, &changed, InvalidCode::Sort)?;
        destination.replay(&bytes, &expected, &mut default_budget()?)?;
    }
    Ok(())
}

fn refused(cas: &mut Cas, expected: &Judgment, bytes: &[u8], code: InvalidCode) -> TestResult {
    let before = cas.canonical_snapshot(&mut default_budget()?)?;
    let fault = cas
        .replay(bytes, expected, &mut default_budget()?)
        .err()
        .ok_or("accepted changed witness")?;
    assert_eq!(fault.kind, FaultKind::Invalid(code));
    assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, before);
    Ok(())
}

fn hidden_zero(f: &mut Fixture, node: &nrz_cas::Handle, zero: &nrz_cas::Handle) -> TestResult {
    let product = f.term(Operator::Mul, &[zero.clone(), node.clone()])?;
    let domain = crate::testing::trig_replay::region(f)?;
    let context = f
        .cas
        .admit(
            &[Draft::Context(ContextDraft::Root {
                header: Reference::Existing(f.header.clone()),
                admissible: Reference::Existing(domain.clone()),
                assumptions: Vec::new(),
            })],
            &mut default_budget()?,
        )?
        .pop()
        .ok_or("context")?;
    let step =
        crate::testing::witness::equality(&context, &domain, Primitive::MulZero, &product, zero);
    let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
    let fault = f
        .cas
        .check_proof(&context, &[step], &mut default_budget()?)
        .err()
        .ok_or("erased original divisor")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::RefutedDomain));
    assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
    Ok(())
}
