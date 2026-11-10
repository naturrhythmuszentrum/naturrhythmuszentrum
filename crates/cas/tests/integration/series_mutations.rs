// SPDX-License-Identifier: AGPL-3.0-only
//! Full portable mutations retain typed record structure and assert every edited offset.

use crate::testing::answer::{index, records};
use crate::testing::common::{TestResult, default_budget};
use crate::testing::series_proof::fixture;
use nrz_cas::{FaultKind, InvalidCode};

/// Single-byte changes state their old value before touching the complete witness.
fn byte(bytes: &[u8], offset: usize, old: u8, new: u8) -> Vec<u8> {
    assert_eq!(bytes[offset], old);
    let mut result = bytes.to_vec();
    result[offset] = new;
    result
}

/// Reference edits must name an admitted earlier record of the intended semantic kind.
fn reference(bytes: &[u8], offset: usize, new: u32) -> TestResult<Vec<u8>> {
    let old = index(bytes, &mut offset.clone())?;
    assert_ne!(old, new);
    let mut result = bytes.to_vec();
    result
        .get_mut(offset..offset + 4)
        .ok_or("reference slot")?
        .copy_from_slice(&new.to_le_bytes());
    Ok(result)
}

/// Each mutation is checked in both orders, with full rollback and the unchanged positive proof.
/// ´test:integration:series-portable-mutation-controls´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Each asserted wire offset stays beside its full-witness positive and rollback controls"
)]
fn series_portable_mutation_controls() -> TestResult {
    super::series_compose_golden::payload_control()?;
    let mut sender = fixture(2, false)?;
    let witness =
        sender
            .base
            .cas
            .check_proof(&sender.base.context, &sender.steps, &mut default_budget()?)?;
    let bytes = sender
        .base
        .cas
        .export_witness(&witness, &mut default_budget()?)?;
    let records = records(&bytes)?;
    let (statement_index, (_, statement)) = records
        .iter()
        .enumerate()
        .find(|(_, (tag, _))| *tag == 39)
        .ok_or("series statement")?;
    let (_, certificate) = records
        .iter()
        .find(|(tag, _)| *tag == 40)
        .ok_or("series certificate")?;
    let (_, conclusion) = records
        .iter()
        .find(|(tag, _)| *tag == 41)
        .ok_or("series conclusion")?;
    for range in [statement, certificate, conclusion] {
        assert_eq!(&bytes[range.start..range.start + 6], &[1, 0, 0, 0, 0, 0]);
    }
    let mut cursor = statement.start + 10;
    assert_eq!(index(&bytes, &mut cursor)?, 12);
    assert_eq!(&bytes[cursor..cursor + 12], b"series.scale");
    cursor += 12;
    assert_eq!(index(&bytes, &mut cursor)?, 1);
    index(&bytes, &mut cursor)?;
    assert_eq!(cursor, statement.start + 34);
    assert_eq!(&bytes[cursor..cursor + 2], &[2, 0]);
    let input = index(&bytes, &mut (cursor + 2))?;
    let (input_tag, input_record) = records.get(usize::try_from(input)?).ok_or("input")?;
    assert_eq!(*input_tag, 38);
    assert_eq!(input_record.len(), 47);
    let first = index(&bytes, &mut (input_record.start + 35))?;
    assert_eq!(records[usize::try_from(first)?].0, 3);
    assert_eq!(
        &bytes[certificate.start + 10..certificate.start + 15],
        &[2, 1, 0, 0, 0]
    );
    assert_eq!(
        &bytes[certificate.start + 19..certificate.start + 25],
        &[2, 0, 3, 0, 0, 0]
    );
    assert_eq!(
        &bytes[certificate.start + 51..certificate.start + 62],
        &[1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 2]
    );
    let (_, predicate) = records
        .iter()
        .find(|(tag, range)| *tag == 7 && range.len() == 23)
        .ok_or("ordinary Defined")?;
    assert_eq!(
        &bytes[predicate.start + 10..predicate.start + 19],
        &[0, 0, 0, 0, 0, 1, 0, 0, 0]
    );
    let (_, ordinary_statement) = records
        .iter()
        .find(|(tag, _)| *tag == 10)
        .ok_or("ordinary statement")?;
    let (_, ordinary_application) = records
        .iter()
        .find(|(tag, _)| *tag == 11)
        .ok_or("ordinary application")?;
    assert_eq!(
        &bytes[ordinary_application.start + 14..ordinary_application.start + 22],
        &[0; 8]
    );
    let mut mutations = vec![
        (
            "generic statement discriminator",
            byte(&bytes, ordinary_statement.start + 10, 1, 32),
            Some(InvalidCode::Encoding),
        ),
        (
            "generic certificate discriminator",
            byte(&bytes, ordinary_application.start + 22, 6, 20),
            Some(InvalidCode::Encoding),
        ),
        (
            "object order mismatch",
            byte(&bytes, input_record.start + 27, 3, 2),
            Some(InvalidCode::Encoding),
        ),
        (
            "object coefficient count mismatch",
            byte(&bytes, input_record.start + 31, 3, 2),
            Some(InvalidCode::Encoding),
        ),
        (
            "certificate proof count",
            byte(&bytes, certificate.start + 11, 1, 0),
            Some(InvalidCode::Rule),
        ),
        (
            "conclusion proof count",
            byte(&bytes, conclusion.start + 14, 1, 0),
            Some(InvalidCode::Rule),
        ),
        (
            "deferred residual result",
            byte(&bytes, cursor + 10, 0, 3),
            Some(InvalidCode::SeriesUnsupported),
        ),
        (
            "source form",
            byte(&bytes, cursor + 1, 0, 1),
            Some(InvalidCode::SeriesUnsupported),
        ),
        (
            "deferred result",
            byte(&bytes, cursor + 10, 0, 2),
            Some(InvalidCode::SeriesUnsupported),
        ),
        (
            "certificate form",
            byte(&bytes, certificate.start + 20, 0, 1),
            Some(InvalidCode::SeriesUnsupported),
        ),
        (
            "deferred mode",
            byte(&bytes, certificate.start + 19, 2, 6),
            Some(InvalidCode::SeriesUnsupported),
        ),
        (
            "wrong exact product",
            byte(&bytes, certificate.start + 56, 1, 3),
            None,
        ),
        (
            "valid object as wrong output",
            reference(&bytes, cursor + 11, input)?,
            None,
        ),
        (
            "changed rational centre",
            reference(&bytes, input_record.start + 10, first)?,
            None,
        ),
        (
            "valid definedness of another scalar",
            reference(&bytes, predicate.start + 19, first)?,
            None,
        ),
        (
            "statement in proof slot",
            reference(
                &bytes,
                certificate.start + 15,
                u32::try_from(statement_index)?,
            )?,
            None,
        ),
        (
            "unchecked statement root",
            reference(&bytes, bytes.len() - 4, u32::try_from(statement_index)?)?,
            None,
        ),
        (
            "changed result schema",
            byte(&bytes, statement.start + 26, 1, 2),
            Some(InvalidCode::SeriesUnsupported),
        ),
    ];
    for mode in 6..=11 {
        if mode == 7 {
            super::series_reciprocal_golden::payload_control()?;
            continue;
        }
        if mode == 10 {
            let mut source = crate::testing::series_source::setup(0, 4, false, false)?;
            let checked = source.base.cas.expand_series_source(
                &source.request,
                &source.proofs,
                &mut default_budget()?,
            )?;
            let valid = source
                .base
                .cas
                .export_witness(&checked, &mut default_budget()?)?;
            for reordered in [false, true] {
                let mut receiver = crate::testing::series_source::receiver(0, 4, reordered)?;
                receiver.base.cas.replay(
                    &valid,
                    receiver.expected.as_ref().ok_or("source expectation")?,
                    &mut default_budget()?,
                )?;
            }
            continue;
        }
        if matches!(mode, 8 | 9) {
            let kind = if mode == 8 { 7 } else { 9 };
            let mut source = fixture(kind, false)?;
            let checked = source.base.cas.check_proof(
                &source.base.context,
                &source.steps,
                &mut default_budget()?,
            )?;
            let valid = source
                .base
                .cas
                .export_witness(&checked, &mut default_budget()?)?;
            for reordered in [false, true] {
                let mut receiver = fixture(kind, reordered)?;
                receiver
                    .base
                    .cas
                    .replay(&valid, &receiver.expected, &mut default_budget()?)?;
            }
            continue;
        }
        mutations.push((
            "deferred mode",
            byte(&bytes, certificate.start + 19, 2, mode),
            Some(InvalidCode::SeriesUnsupported),
        ));
    }
    for (name, changed, code) in mutations {
        for reordered in [false, true] {
            let mut receiver = fixture(2, reordered)?;
            let before = receiver
                .base
                .cas
                .canonical_snapshot(&mut default_budget()?)?;
            let fault = receiver
                .base
                .cas
                .replay(&changed, &receiver.expected, &mut default_budget()?)
                .expect_err(name);
            if let Some(code) = code {
                assert_eq!(fault.kind, FaultKind::Invalid(code), "{name}");
            } else {
                assert!(
                    matches!(fault.kind, FaultKind::Invalid(_)),
                    "{name}: {fault:?}"
                );
            }
            assert_eq!(
                receiver
                    .base
                    .cas
                    .canonical_snapshot(&mut default_budget()?)?,
                before,
                "{name}"
            );
            let positive =
                receiver
                    .base
                    .cas
                    .replay(&bytes, &receiver.expected, &mut default_budget()?)?;
            assert_eq!(
                receiver
                    .base
                    .cas
                    .export_witness(&positive, &mut default_budget()?)?,
                bytes
            );
        }
    }
    Ok(())
}

/// Arithmetic and framing edits assert offsets in valid mode-specific complete witnesses.
/// ´test:integration:series-calculus-portable-mutations´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Each complete mutation retains its setup, precise fault and rollback evidence together"
)]
fn series_calculus_portable_mutations() -> TestResult {
    for (kind, mode, key, slots) in [(7, 8, "series.derive", 0), (9, 9, "series.integrate", 1)] {
        let mut sender = fixture(kind, false)?;
        let witness = sender.base.cas.check_proof(
            &sender.base.context,
            &sender.steps,
            &mut default_budget()?,
        )?;
        let bytes = sender
            .base
            .cas
            .export_witness(&witness, &mut default_budget()?)?;
        let inventory = records(&bytes)?;
        let (_, statement) = inventory
            .iter()
            .find(|(tag, _)| *tag == 39)
            .ok_or("statement")?;
        let (_, certificate) = inventory
            .iter()
            .find(|(tag, _)| *tag == 40)
            .ok_or("certificate")?;
        let (_, conclusion) = inventory
            .iter()
            .find(|(tag, _)| *tag == 41)
            .ok_or("conclusion")?;
        let mut cursor = statement.start + 10;
        assert_eq!(usize::try_from(index(&bytes, &mut cursor)?)?, key.len());
        assert_eq!(&bytes[cursor..cursor + key.len()], key.as_bytes());
        cursor += key.len();
        let version = cursor;
        assert_eq!(index(&bytes, &mut cursor)?, 1);
        index(&bytes, &mut cursor)?;
        assert_eq!(&bytes[cursor..cursor + 2], &[mode, 0]);
        let input = index(&bytes, &mut (cursor + 2))?;
        let result_tag = cursor + 6 + slots * 4;
        assert_eq!(bytes[result_tag], 0);
        assert_eq!(bytes[certificate.start + 10], mode);
        assert_eq!(
            index(&bytes, &mut (certificate.start + 11))?,
            u32::try_from(slots)?
        );
        let rows = certificate.start + 15 + slots * 4;
        assert_eq!(&bytes[rows..rows + 6], &[mode, 0, 2, 0, 0, 0]);
        assert_eq!(index(&bytes, &mut (rows + 6))?, 0);
        // All first-row components have one-byte positive numerator and denominator.
        let altered_number = rows + 10;
        assert_eq!(&bytes[altered_number..altered_number + 5], &[0, 1, 0, 0, 0]);
        assert_eq!(bytes[altered_number + 5], if mode == 8 { 2 } else { 1 });
        let mutations = [
            (
                byte(
                    &bytes,
                    certificate.start + 10,
                    mode,
                    if mode == 8 { 9 } else { 8 },
                ),
                InvalidCode::Rule,
            ),
            (
                byte(
                    &bytes,
                    certificate.start + 11,
                    u8::try_from(slots)?,
                    u8::try_from(slots + 1)?,
                ),
                InvalidCode::Rule,
            ),
            (
                byte(&bytes, cursor + 1, 0, 2),
                InvalidCode::SeriesUnsupported,
            ),
            (
                byte(&bytes, result_tag, 0, 2),
                InvalidCode::SeriesUnsupported,
            ),
            (byte(&bytes, version, 1, 2), InvalidCode::SeriesUnsupported),
            (byte(&bytes, rows + 1, 0, 1), InvalidCode::SeriesUnsupported),
            (byte(&bytes, rows + 2, 2, 1), InvalidCode::Rule),
            (byte(&bytes, rows + 6, 0, 1), InvalidCode::Rule),
            (
                byte(&bytes, altered_number + 5, if mode == 8 { 2 } else { 1 }, 7),
                InvalidCode::Rule,
            ),
            (reference(&bytes, result_tag + 1, input)?, InvalidCode::Rule),
            (
                byte(
                    &bytes,
                    conclusion.start + 14,
                    u8::try_from(slots)?,
                    u8::try_from(slots + 1)?,
                ),
                InvalidCode::Rule,
            ),
        ];
        for (changed, code) in mutations {
            for reordered in [false, true] {
                let mut receiver = fixture(kind, reordered)?;
                let before = receiver
                    .base
                    .cas
                    .canonical_snapshot(&mut default_budget()?)?;
                assert_eq!(
                    receiver
                        .base
                        .cas
                        .replay(&changed, &receiver.expected, &mut default_budget()?)
                        .expect_err("portable mutation")
                        .kind,
                    FaultKind::Invalid(code)
                );
                assert_eq!(
                    receiver
                        .base
                        .cas
                        .canonical_snapshot(&mut default_budget()?)?,
                    before
                );
                receiver
                    .base
                    .cas
                    .replay(&bytes, &receiver.expected, &mut default_budget()?)?;
            }
        }
    }
    Ok(())
}

/// Valid unchanged witness bytes cannot prove a receiver's different finite endpoint.
/// ´test:integration:series-independent-wrong-expectation´
#[test]
fn series_independent_wrong_expectation() -> TestResult {
    for kind in [0, 2, 3, 5, 7, 9, 10, 13, 14, 15] {
        let mut source = fixture(kind, false)?;
        let witness = source.base.cas.check_proof(
            &source.base.context,
            &source.steps,
            &mut default_budget()?,
        )?;
        let bytes = source
            .base
            .cas
            .export_witness(&witness, &mut default_budget()?)?;
        for reordered in [false, true] {
            let mut receiver = fixture(kind, reordered)?;
            let mut expected = receiver.expected.clone();
            let nrz_cas::proof::Statement::Series(value) = &mut expected.statement else {
                return Err("series".into());
            };
            match value.as_mut() {
                nrz_cas::series::SeriesStatement::Source(_)
                | nrz_cas::series::SeriesStatement::EvaluatedResidual { .. } => {
                    return Err("object fixture".into());
                }
                nrz_cas::series::SeriesStatement::Reciprocal { input, output }
                | nrz_cas::series::SeriesStatement::Truncate { input, output }
                | nrz_cas::series::SeriesStatement::Scale { input, output, .. }
                | nrz_cas::series::SeriesStatement::Derive { input, output }
                | nrz_cas::series::SeriesStatement::Integrate { input, output, .. } => {
                    *output = input.clone();
                }
                nrz_cas::series::SeriesStatement::Mul { left, output, .. } => {
                    *output = left.clone();
                }
                nrz_cas::series::SeriesStatement::Coefficient { value, .. } => {
                    *value = receiver.base.zero.clone();
                }
                nrz_cas::series::SeriesStatement::Compose { right, output, .. } => {
                    *output = right.clone();
                }
                nrz_cas::series::SeriesStatement::Add { .. } => return Err("fixture".into()),
            }
            let before = receiver
                .base
                .cas
                .canonical_snapshot(&mut default_budget()?)?;
            assert_eq!(
                receiver
                    .base
                    .cas
                    .replay(&bytes, &expected, &mut default_budget()?)
                    .expect_err("different expectation")
                    .kind,
                FaultKind::Invalid(InvalidCode::EndpointMismatch)
            );
            assert_eq!(
                receiver
                    .base
                    .cas
                    .canonical_snapshot(&mut default_budget()?)?,
                before
            );
            receiver
                .base
                .cas
                .replay(&bytes, &receiver.expected, &mut default_budget()?)?;
        }
    }
    Ok(())
}

/// A well-typed portable claim cannot omit its retained scalar hypothesis or residual.
/// ´test:integration:series-portable-condition-closure´
#[test]
fn series_portable_condition_closure() -> TestResult {
    for (integral, residual) in [(false, false), (false, true), (true, false), (true, true)] {
        let make = if integral {
            crate::testing::series_proof::conditioned_integral
        } else {
            crate::testing::series_proof::conditioned
        };
        let mut source = make(residual, false)?;
        let witness = source.base.cas.check_proof(
            &source.base.context,
            &source.steps,
            &mut default_budget()?,
        )?;
        let bytes = source
            .base
            .cas
            .export_witness(&witness, &mut default_budget()?)?;
        let inventory = records(&bytes)?;
        let (_, record) = inventory
            .iter()
            .find(|(tag, _)| *tag == 39)
            .ok_or("series declaration")?;
        let shift = if integral { 4 } else { 0 };
        let mut count_at = record.start + shift + 49;
        assert_eq!(
            &bytes[record.start + shift + 34..record.start + shift + 36],
            &[if integral { 9 } else { 2 }, 0]
        );
        assert_eq!(index(&bytes, &mut count_at)?, u32::from(!residual));
        let (count_at, width) = if residual {
            (record.start + shift + 53, 4)
        } else {
            (record.start + shift + 49, 8)
        };
        assert_eq!(index(&bytes, &mut count_at.clone())?, 1);
        let encoded_len = index(&bytes, &mut (record.start - 4))?;
        assert_eq!(usize::try_from(encoded_len)?, record.len());
        let mut changed = bytes.clone();
        changed[count_at..count_at + 4].copy_from_slice(&0_u32.to_le_bytes());
        changed.drain(count_at + 4..count_at + 4 + width);
        changed[record.start - 4..record.start]
            .copy_from_slice(&(encoded_len - u32::try_from(width)?).to_le_bytes());
        for reordered in [false, true] {
            let mut receiver = make(residual, reordered)?;
            let before = receiver
                .base
                .cas
                .canonical_snapshot(&mut default_budget()?)?;
            assert_eq!(
                receiver
                    .base
                    .cas
                    .replay(&changed, &receiver.expected, &mut default_budget()?)
                    .expect_err("changed condition closure")
                    .kind,
                FaultKind::Invalid(InvalidCode::Rule)
            );
            assert_eq!(
                receiver
                    .base
                    .cas
                    .canonical_snapshot(&mut default_budget()?)?,
                before
            );
            let positive =
                receiver
                    .base
                    .cas
                    .replay(&bytes, &receiver.expected, &mut default_budget()?)?;
            assert_eq!(
                receiver
                    .base
                    .cas
                    .export_witness(&positive, &mut default_budget()?)?,
                bytes
            );
        }
    }
    Ok(())
}
