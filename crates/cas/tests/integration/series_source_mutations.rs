// SPDX-License-Identifier: AGPL-3.0-only
//! Offset-asserted source framing, reserved capabilities and original expected identities.

use crate::testing::answer::{index, records};
use crate::testing::common::{TestResult, default_budget};
use crate::testing::series_source::{setup, statement};
use nrz_cas::{FaultKind, InvalidCode};

fn byte(bytes: &[u8], at: usize, old: u8, new: u8) -> Vec<u8> {
    assert_eq!(bytes[at], old);
    let mut changed = bytes.to_vec();
    changed[at] = new;
    changed
}

/// Every reserved selector and operation refuses inside a full valid ordinary/source closure.
/// ´test:integration:series-source-wire-mutations´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Offsets, typed refusals and unchanged positives remain adjacent"
)]
fn series_source_wire_mutations() -> TestResult {
    let bytes = super::series_source_golden::golden(0)?;
    let records = records(&bytes)?;
    let (_, claim) = records.iter().find(|(tag, _)| *tag == 39).ok_or("claim")?;
    let (_, certificate) = records
        .iter()
        .find(|(tag, _)| *tag == 40)
        .ok_or("certificate")?;
    let (_, conclusion) = records
        .iter()
        .find(|(tag, _)| *tag == 41)
        .ok_or("conclusion")?;
    let (_, ordinary) = records
        .iter()
        .find(|(tag, _)| *tag == 10)
        .ok_or("ordinary")?;
    let (_, ordinary_proof) = records
        .iter()
        .find(|(tag, _)| *tag == 11)
        .ok_or("ordinary proof")?;
    let mut cursor = claim.start + 10;
    assert_eq!(index(&bytes, &mut cursor)?, 13);
    assert_eq!(&bytes[cursor..cursor + 13], b"series.source");
    cursor += 13;
    assert_eq!(index(&bytes, &mut cursor)?, 1);
    index(&bytes, &mut cursor)?;
    assert_eq!(cursor, claim.start + 35);
    assert_eq!(&bytes[cursor..cursor + 3], &[10, 0, 3]);
    assert_eq!(
        &bytes[certificate.start + 10..certificate.start + 15],
        &[10, 1, 0, 0, 0]
    );
    assert_eq!(
        &bytes[certificate.start + 19..certificate.start + 22],
        &[10, 0, 3]
    );
    let mut mutations = Vec::new();
    let last = certificate.start + 57;
    assert_eq!(&bytes[last..last + 11], &[1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 6]);
    let mut shortened = byte(&bytes, certificate.start + 22, 4, 3);
    assert_eq!(
        index(&bytes, &mut (certificate.start - 4))?,
        u32::try_from(certificate.len())?
    );
    shortened[certificate.start - 4..certificate.start]
        .copy_from_slice(&u32::try_from(certificate.len() - 11)?.to_le_bytes());
    shortened.drain(last..last + 11);
    mutations.push((shortened, InvalidCode::Rule));
    let original = index(&bytes, &mut (cursor + 3))?;
    assert_eq!(records[usize::try_from(original)?].0, 3);
    let replacement = records
        .iter()
        .enumerate()
        .find(|(i, (tag, _))| *tag == 3 && u32::try_from(*i).ok() != Some(original))
        .ok_or("other original expression")?
        .0;
    let mut changed = bytes.clone();
    changed[cursor + 3..cursor + 7].copy_from_slice(&u32::try_from(replacement)?.to_le_bytes());
    mutations.push((changed, InvalidCode::Rule));
    let mut row = certificate.start + 22;
    assert_eq!(index(&bytes, &mut row)?, 4);
    for _ in 0..4 {
        row += 1;
        let n = usize::try_from(index(&bytes, &mut row)?)?;
        row += n;
        let n = usize::try_from(index(&bytes, &mut row)?)?;
        row += n;
    }
    assert_eq!(index(&bytes, &mut row)?, 4);
    // The first companion coefficient is exactly +1/1; its sign is independent of sine's output.
    assert_eq!(&bytes[row..row + 11], &[0, 1, 0, 0, 0, 1, 1, 0, 0, 0, 1]);
    mutations.push((byte(&bytes, row, 0, 1), InvalidCode::Rule));
    super::series_compose_golden::payload_control()?;
    super::series_residual_wire::payload_control()?;
    // The mode-eleven reinterpretation of this source payload is malformed, not unavailable.
    mutations.push((byte(&bytes, cursor, 10, 11), InvalidCode::SeriesUnsupported));
    mutations.push((byte(&bytes, cursor, 10, 7), InvalidCode::Encoding));
    for mode in [6, 255] {
        mutations.push((
            byte(&bytes, cursor, 10, mode),
            InvalidCode::SeriesUnsupported,
        ));
    }
    for law in [0, 1, 2, 6, 255] {
        mutations.push((
            byte(&bytes, cursor + 2, 3, law),
            InvalidCode::SeriesUnsupported,
        ));
        mutations.push((
            byte(&bytes, certificate.start + 21, 3, law),
            InvalidCode::SeriesUnsupported,
        ));
    }
    for form in [1, 2, 255] {
        mutations.push((
            byte(&bytes, cursor + 1, 0, form),
            InvalidCode::SeriesUnsupported,
        ));
    }
    for (mode, form) in [
        (1, 2),
        (2, 2),
        (3, 2),
        (4, 1),
        (4, 2),
        (8, 1),
        (8, 2),
        (9, 1),
        (9, 2),
    ] {
        let mut changed = byte(&bytes, cursor, 10, mode);
        assert_eq!(changed[cursor + 1], 0);
        changed[cursor + 1] = form;
        assert_eq!(changed[cursor + 2], 3);
        changed[cursor + 2] = 0;
        mutations.push((changed, InvalidCode::SeriesUnsupported));
    }
    for tag in [0, 1, 255] {
        mutations.push((
            byte(&bytes, cursor + 11, 2, tag),
            InvalidCode::SeriesUnsupported,
        ));
    }
    mutations.extend([
        (
            byte(&bytes, cursor + 11, 2, 3),
            InvalidCode::SeriesUnsupported,
        ),
        (
            byte(&bytes, claim.start, 1, 2),
            InvalidCode::UnsupportedVersion,
        ),
        (
            byte(&bytes, certificate.start, 1, 2),
            InvalidCode::UnsupportedVersion,
        ),
        (
            byte(&bytes, conclusion.start, 1, 2),
            InvalidCode::UnsupportedVersion,
        ),
        (
            byte(&bytes, certificate.start + 11, 1, 0),
            InvalidCode::Rule,
        ),
        (
            byte(&bytes, certificate.start + 11, 1, 2),
            InvalidCode::Rule,
        ),
        (byte(&bytes, conclusion.start + 14, 1, 0), InvalidCode::Rule),
        (byte(&bytes, conclusion.start + 14, 1, 2), InvalidCode::Rule),
        (
            byte(&bytes, ordinary.start + 10, 1, 32),
            InvalidCode::Encoding,
        ),
        (
            byte(&bytes, ordinary_proof.start + 22, 6, 20),
            InvalidCode::Encoding,
        ),
    ]);
    for reordered in [false, true] {
        let mut f = setup(0, 4, reordered, true)?;
        let expected = f.expected.as_ref().ok_or("expected")?;
        let positive = f
            .base
            .cas
            .replay(&bytes, expected, &mut default_budget()?)?;
        for (changed, code) in &mutations {
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            assert_eq!(
                f.base
                    .cas
                    .replay(changed, expected, &mut default_budget()?)
                    .expect_err("wire mutation")
                    .kind,
                FaultKind::Invalid(*code)
            );
            assert_eq!(
                f.base.cas.canonical_snapshot(&mut default_budget()?)?,
                before
            );
        }
        assert_eq!(
            f.base
                .cas
                .export_witness(&positive, &mut default_budget()?)?,
            bytes
        );
    }
    Ok(())
}

/// Unchanged portable bytes cannot prove another source or an enlarged/narrowed domain.
/// ´test:integration:series-source-expected-twins´
#[test]
fn series_source_expected_twins() -> TestResult {
    use nrz_cas::proof::Statement;
    use nrz_cas::series::SeriesStatement;
    use nrz_cas::syntax::Operator;
    for reordered in [false, true] {
        let bytes = super::series_source_golden::golden(0)?;
        let mut f = setup(0, 4, reordered, true)?;
        let expected = f.expected.as_ref().ok_or("expected")?.clone();
        let cosine = f.base.term(Operator::Cosine, &[f.base.x.clone()])?;
        let atan = f.base.term(Operator::Arctangent, &[f.base.x.clone()])?;
        for source in [cosine, atan, f.base.x.clone()] {
            let mut wrong = expected.clone();
            let mut claim = statement(&wrong)?.clone();
            claim.source = source;
            wrong.statement = Statement::Series(Box::new(SeriesStatement::Source(claim)));
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            assert!(
                f.base
                    .cas
                    .replay(&bytes, &wrong, &mut default_budget()?)
                    .is_err()
            );
            assert_eq!(
                f.base.cas.canonical_snapshot(&mut default_budget()?)?,
                before
            );
        }
        let predicate = f.base.predicate(
            nrz_cas::syntax::PredicateKind::Positive,
            &[f.base.x.clone()],
        )?;
        let domain = f.base.region(&predicate, true)?;
        let mut wrong = expected.clone();
        let mut claim = statement(&wrong)?.clone();
        claim.domain = domain;
        wrong.statement = Statement::Series(Box::new(SeriesStatement::Source(claim)));
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        assert!(
            f.base
                .cas
                .replay(&bytes, &wrong, &mut default_budget()?)
                .is_err()
        );
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
        f.base
            .cas
            .replay(&bytes, &expected, &mut default_budget()?)?;
    }
    Ok(())
}

/// A conditional lifted proof cannot drop or replace its qualified original-domain hypothesis.
/// ´test:integration:series-source-conditional-wire-closure´
#[test]
fn series_source_conditional_wire_closure() -> TestResult {
    let bytes = super::series_source_golden::lifted_golden(true)?;
    let records = records(&bytes)?;
    let (_, claim) = records
        .iter()
        .rev()
        .find(|(tag, _)| *tag == 39)
        .ok_or("lifted claim")?;
    let (_, certificate) = records
        .iter()
        .rev()
        .find(|(tag, _)| *tag == 40)
        .ok_or("lifted certificate")?;
    let mut cursor = claim.start + 10;
    let length = usize::try_from(index(&bytes, &mut cursor)?)?;
    cursor += length;
    assert_eq!(index(&bytes, &mut cursor)?, 1);
    index(&bytes, &mut cursor)?;
    assert_eq!(&bytes[cursor..cursor + 3], &[3, 1, 0]);
    let hypotheses = cursor + 16;
    assert_eq!(index(&bytes, &mut hypotheses.clone())?, 1);
    let slot = hypotheses + 8;
    assert_eq!(index(&bytes, &mut slot.clone())?, 0);
    assert_eq!(
        &bytes[certificate.start + 10..certificate.start + 15],
        &[3, 4, 0, 0, 0]
    );
    let left = index(&bytes, &mut (certificate.start + 15))?;
    let right = index(&bytes, &mut (certificate.start + 19))?;
    assert_eq!(left, right);
    assert_eq!(records[usize::try_from(left)?].0, 41);
    for reordered in [false, true] {
        let mut f = crate::testing::series_source::lifted(7, reordered)?;
        let expected = f.expected.as_ref().ok_or("expected")?;
        let positive = f
            .base
            .cas
            .replay(&bytes, expected, &mut default_budget()?)?;
        assert_eq!(positive.hypotheses().len(), 1);
        for changed in [byte(&bytes, slot, 0, 1), byte(&bytes, hypotheses, 1, 0)] {
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            assert!(matches!(
                f.base
                    .cas
                    .replay(&changed, expected, &mut default_budget()?)
                    .expect_err("conditional closure")
                    .kind,
                FaultKind::Invalid(_)
            ));
            assert_eq!(
                f.base.cas.canonical_snapshot(&mut default_budget()?)?,
                before
            );
        }
        assert_eq!(
            f.base
                .cas
                .export_witness(&positive, &mut default_budget()?)?,
            bytes
        );
    }
    Ok(())
}

/// Source publication and independent replay retain qualified hypotheses and closed residuals.
/// ´test:integration:series-source-residual-lifecycle´
#[test]
fn series_source_residual_lifecycle() -> TestResult {
    for residual in [false, true] {
        let mut f = crate::testing::series_source::conditioned(residual, false)?;
        let positive =
            f.base
                .cas
                .expand_series_source(&f.request, &f.proofs, &mut default_budget()?)?;
        assert_eq!(positive.judgment(), f.expected.as_ref().ok_or("expected")?);
        assert_eq!(positive.hypotheses().len(), usize::from(!residual));
        assert_eq!(positive.judgment().residuals.len(), usize::from(residual));
        let bytes = f
            .base
            .cas
            .export_witness(&positive, &mut default_budget()?)?;
        for reordered in [false, true] {
            let mut receiver = crate::testing::series_source::conditioned(residual, reordered)?;
            let expected = receiver.expected.as_ref().ok_or("expected")?;
            let before = receiver
                .base
                .cas
                .canonical_snapshot(&mut default_budget()?)?;
            if residual {
                let mut wrong = expected.clone();
                wrong.residuals.clear();
                assert!(
                    receiver
                        .base
                        .cas
                        .replay(&bytes, &wrong, &mut default_budget()?)
                        .is_err()
                );
                assert_eq!(
                    receiver
                        .base
                        .cas
                        .canonical_snapshot(&mut default_budget()?)?,
                    before
                );
                let mut wrong = expected.clone();
                wrong.residuals[0].predicate = receiver.base.predicate(
                    nrz_cas::syntax::PredicateKind::Positive,
                    &[receiver.base.one.clone()],
                )?;
                let before = receiver
                    .base
                    .cas
                    .canonical_snapshot(&mut default_budget()?)?;
                assert!(
                    receiver
                        .base
                        .cas
                        .replay(&bytes, &wrong, &mut default_budget()?)
                        .is_err()
                );
                assert_eq!(
                    receiver
                        .base
                        .cas
                        .canonical_snapshot(&mut default_budget()?)?,
                    before
                );
            }
            let checked = receiver
                .base
                .cas
                .replay(&bytes, expected, &mut default_budget()?)?;
            assert_eq!(checked.hypotheses().len(), usize::from(!residual));
            assert_eq!(checked.judgment().residuals.len(), usize::from(residual));
        }
    }
    Ok(())
}

/// An unchanged restricted-source witness cannot be widened to the ambient domain.
/// ´test:integration:series-source-domain-widening-refused´
#[test]
fn series_source_domain_widening_refused() -> TestResult {
    let (mut sender, _) = crate::testing::series_source::restricted(false)?;
    let witness = sender.base.cas.expand_series_source(
        &sender.request,
        &sender.proofs,
        &mut default_budget()?,
    )?;
    let bytes = sender
        .base
        .cas
        .export_witness(&witness, &mut default_budget()?)?;
    for reordered in [false, true] {
        let (mut f, ambient) = crate::testing::series_source::restricted(reordered)?;
        let expected = f.expected.as_ref().ok_or("expected")?;
        let mut wrong = expected.clone();
        let nrz_cas::proof::Statement::Series(value) = &mut wrong.statement else {
            return Err("series".into());
        };
        let nrz_cas::series::SeriesStatement::Source(value) = value.as_mut() else {
            return Err("source".into());
        };
        assert_ne!(value.domain, ambient);
        value.domain = ambient;
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        assert!(
            f.base
                .cas
                .replay(&bytes, &wrong, &mut default_budget()?)
                .is_err()
        );
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
        f.base
            .cas
            .replay(&bytes, expected, &mut default_budget()?)?;
    }
    Ok(())
}

/// A full sine/cosine mixed closure cannot turn a finite sum into the original tangent quotient.
/// ´test:integration:series-source-quotient-wire-refused´
#[test]
fn series_source_quotient_wire_refused() -> TestResult {
    let bytes = super::series_source_golden::lifted_golden(false)?;
    let records = records(&bytes)?;
    let (_, claim) = records
        .iter()
        .rev()
        .find(|(tag, _)| *tag == 39)
        .ok_or("claim")?;
    let mut cursor = claim.start + 10;
    let n = usize::try_from(index(&bytes, &mut cursor)?)?;
    cursor += n;
    assert_eq!(index(&bytes, &mut cursor)?, 1);
    index(&bytes, &mut cursor)?;
    assert_eq!(&bytes[cursor..cursor + 3], &[1, 1, 0]);
    let source = index(&bytes, &mut (cursor + 3))?;
    let (tag, source) = &records[usize::try_from(source)?];
    assert_eq!(*tag, 3);
    let quotient = byte(&bytes, source.start + 22, 5, 8);
    let mut mutations = vec![(quotient, InvalidCode::Rule)];
    mutations.push((byte(&bytes, cursor, 1, 5), InvalidCode::SeriesUnsupported));
    super::series_compose_golden::payload_control()?;
    super::series_residual_wire::payload_control()?;
    // A nonzero source form is still not a residual form.
    mutations.push((byte(&bytes, cursor, 1, 11), InvalidCode::SeriesUnsupported));
    mutations.push((byte(&bytes, cursor, 1, 7), InvalidCode::Rule));
    mutations.push((byte(&bytes, cursor, 1, 6), InvalidCode::SeriesUnsupported));
    for reordered in [false, true] {
        let mut f = crate::testing::series_source::lifted(1, reordered)?;
        let expected = f.expected.as_ref().ok_or("expected")?;
        f.base
            .cas
            .replay(&bytes, expected, &mut default_budget()?)?;
        for (changed, code) in &mutations {
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            assert_eq!(
                f.base
                    .cas
                    .replay(changed, expected, &mut default_budget()?)
                    .expect_err("deferred source assembly")
                    .kind,
                FaultKind::Invalid(*code)
            );
            assert_eq!(
                f.base.cas.canonical_snapshot(&mut default_budget()?)?,
                before
            );
        }
    }
    Ok(())
}
