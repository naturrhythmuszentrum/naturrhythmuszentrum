// SPDX-License-Identifier: AGPL-3.0-only
//! Owned premise inventories and typed analytic evidence require kernel-visible inspection.

use crate::budget::{Budget, BudgetKind, Limits};
use crate::error::{FaultKind, InvalidCode};
use crate::graph::{Draft, Handle, Reference};
use crate::proof_model::{ApplicationDraft, VerifiedWitness};
use crate::series::SeriesOperation as O;
use crate::testing::common::TestResult;
use crate::testing::native::{Fixture as Base, admit, fresh_budget as default_budget, term};

struct Fixture {
    base: Base,
    field: Handle,
}

/// Every ordered owned restriction slot is prepaid, including identical supplied handles.
/// ´test:crate:compose-multiple-slot-reservation´
#[test]
fn compose_multiple_slot_reservation() -> TestResult {
    let (source, original) = large()?;
    let bytes = source
        .base
        .cas
        .export_witness(&original, &mut default_budget()?)?;
    let mut f = fixture(false)?;
    let proof = f
        .base
        .cas
        .inspect(&bytes, &mut default_budget()?)?
        .proof()
        .clone();
    let input = object(&mut f.base, &f.field, &["0", "1"])?;
    let payload = crate::proof_view::application_bytes(
        &f.base.cas,
        proof.logical_id(),
        &mut default_budget()?,
    )?;
    let mut probe = default_budget()?;
    let outer = probe.limits().staged_payload_bytes;
    let effective = f
        .base
        .cas
        .transaction(&mut probe, |_, b| Ok(b.limits().staged_payload_bytes))?;
    let enclosing = outer - effective;
    // At n=1, two evidence rows plus one output, one-bit coefficients and eight scope bytes.
    let workspace = 3 * ((4 + 8) * 16 + 512) + 8 * 4;
    let needed = workspace + 2 * payload;
    let allowance = enclosing + needed - 1;
    assert!(workspace + payload < needed);
    let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
    let mut b = Budget::new(Limits {
        staged_payload_bytes: allowance,
        ..default_budget()?.limits().clone()
    })?;
    let fault = f
        .base
        .cas
        .compute_series(
            &O::Compose {
                left: input.clone(),
                right: input,
                order: Some(1),
            },
            &[proof.clone(), proof],
            &mut b,
        )
        .expect_err("both owned slots must be reserved");
    assert_eq!(
        (
            enclosing,
            workspace,
            payload,
            allowance,
            needed,
            b.usage().work
        ),
        (640, 2144, 30_612, 64_007, 63_368, 179_407)
    );
    assert_eq!(
        fault,
        crate::Fault::exhausted(
            BudgetKind::StagedPayloadBytes,
            needed - 1,
            needed,
            crate::Location::new("formal series arithmetic")
        )
    );
    assert_eq!(
        before,
        f.base.cas.canonical_snapshot(&mut default_budget()?)?
    );
    Ok(())
}

/// The complete seed certificate remains reserved through publication at order one.
/// ´test:crate:series-source-seed-publication-reservation´
#[test]
fn series_source_seed_publication_reservation() -> TestResult {
    // With ComponentBits=32, x^12 at centre two has 13 complete coefficients.
    // Their 171 encoded bytes and 13 rows reserve 4*171 + 256*13 = 4012 bytes.
    // The measured released-certificate minimum is 28256; keeping the evidence
    // through sealing needs 28256 + 4012 = 32268. Thus N-1 would otherwise pass.
    assert_eq!(seed_publication_attempt(32_268)?, (true, 367_165, None));
    assert_eq!(seed_publication_attempt(32_332)?, (true, 367_165, None));
    assert_eq!(
        seed_publication_attempt(32_267)?,
        (
            false,
            363_374,
            Some(crate::Fault::exhausted(
                BudgetKind::StagedPayloadBytes,
                24_655,
                24_656,
                crate::Location::new("typed witness DAG"),
            )),
        ),
    );
    Ok(())
}

/// Admit only the order-one object; seed production starts with a fresh ledger.
fn seed_publication_fixture() -> TestResult<(Fixture, crate::series::SourceRequest, VerifiedWitness)>
{
    use crate::series::{SourceClause, SourceRequest};
    let mut f = fixture(false)?;
    f.base.cas.limits.output_bytes = 16_777_216;
    let source = term(
        &mut f.base.cas,
        &f.base.header,
        crate::model::Operator::IntegerPower(12),
        &[f.base.x.clone()],
    )?;
    let proof = source_defined(&mut f, &source)?;
    let centre = term(
        &mut f.base.cas,
        &f.base.header,
        crate::model::Operator::Rational(crate::Rational::parse("2", &mut default_budget()?)?),
        &[],
    )?;
    let coefficient = term(
        &mut f.base.cas,
        &f.base.header,
        crate::model::Operator::Rational(crate::Rational::parse("4096", &mut default_budget()?)?),
        &[],
    )?;
    admit(
        &mut f.base.cas,
        Draft::SeriesObject(crate::series::SeriesObject {
            field: Reference::Existing(f.field.clone()),
            centre: Reference::Existing(centre.clone()),
            variable: Reference::Existing(f.base.x.clone()),
            scope: Vec::new(),
            context: Reference::Existing(f.base.context.clone()),
            order: 1,
            coefficients: vec![Reference::Existing(coefficient)],
        }),
    )?;
    let request = SourceRequest {
        clause: SourceClause::PolynomialSeed,
        source,
        domain: f.base.domain.clone(),
        field: f.field.clone(),
        centre,
        variable: f.base.x.clone(),
        scope: Vec::new(),
        context: f.base.context.clone(),
        order: 1,
    };
    Ok((f, request, proof))
}

/// Each allowance starts from identical syntax and checks rollback on refusal.
fn seed_publication_attempt(allowance: u64) -> TestResult<(bool, u64, Option<crate::Fault>)> {
    let (mut f, request, proof) = seed_publication_fixture()?;
    let before = f
        .base
        .cas
        .canonical_snapshot(&mut source_retention_budget()?)?;
    let mut b = Budget::new(Limits {
        staged_payload_bytes: allowance,
        component_bits: 32,
        ..source_retention_budget()?.limits().clone()
    })?;
    let result = f
        .base
        .cas
        .expand_series_source(&request, &[proof.proof().clone()], &mut b);
    assert_eq!(b.checked_records.bytes, 0);
    match result {
        Ok(witness) => {
            let application = crate::proof_view::application(
                &f.base.cas,
                witness.proof().logical_id(),
                &mut default_budget()?,
            )?;
            let evidence = crate::series::codec::source_workspace(
                &application.certificate,
                &mut default_budget()?,
            )?;
            assert_eq!(evidence, 4 * 171 + 256 * 13);
            assert_eq!(source_parts(&application)?.1.coefficients.len(), 13);
            let bytes = f
                .base
                .cas
                .export_witness(&witness, &mut source_retention_budget()?)?;
            f.base
                .cas
                .replay(&bytes, witness.judgment(), &mut source_retention_budget()?)?;
            Ok((true, b.usage().work, None))
        }
        Err(fault) => {
            assert!(matches!(
                fault.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::StagedPayloadBytes,
                    ..
                }
            ));
            assert_eq!(
                f.base
                    .cas
                    .canonical_snapshot(&mut source_retention_budget()?)?,
                before
            );
            Ok((false, b.usage().work, Some(fault)))
        }
    }
}

/// A large unrelated retained store cannot disappear from a fresh source caller's ledger.
/// ´test:crate:series-source-fresh-judgment-reservation´
#[test]
fn series_source_fresh_judgment_reservation() -> TestResult {
    source_judgment_reservation(false, false)
}

/// An existing owner is refreshed and charged once beside source arithmetic.
/// ´test:crate:series-source-registered-judgment-reservation´
#[test]
fn series_source_registered_judgment_reservation() -> TestResult {
    source_judgment_reservation(true, false)
}

/// Fresh production releases its candidate reservation before independent publication checking.
/// ´test:crate:series-source-fresh-production-single-reservation´
#[test]
fn series_source_fresh_production_single_reservation() -> TestResult {
    source_judgment_reservation(false, true)
}

/// Keep the complete original polynomial and unrelated anonymous judgments beside each route.
#[expect(
    clippy::too_many_lines,
    reason = "Both ledgers share identical retained syntax and rollback assertions"
)]
fn source_judgment_reservation(registered: bool, fresh_production: bool) -> TestResult {
    use crate::series::{SourceCertificate, SourceClause, SourceRequest, SourceStatement};
    let mut f = fixture(false)?;
    f.base.cas.limits.output_bytes = 16_777_216;
    let source = term(
        &mut f.base.cas,
        &f.base.header,
        crate::model::Operator::IntegerPower(12),
        &[f.base.x.clone()],
    )?;
    let proof = source_defined(&mut f, &source)?;
    let mut proofs = vec![proof.proof().clone()];
    let centre = term(
        &mut f.base.cas,
        &f.base.header,
        crate::model::Operator::Rational(crate::Rational::parse(
            "2",
            &mut source_retention_budget()?,
        )?),
        &[],
    )?;
    let mut request = SourceRequest {
        clause: SourceClause::PolynomialSeed,
        source: source.clone(),
        domain: f.base.domain.clone(),
        field: f.field.clone(),
        centre: centre.clone(),
        variable: f.base.x.clone(),
        scope: Vec::new(),
        context: f.base.context.clone(),
        order: 1,
    };
    let value = crate::Rational::parse("4096", &mut source_retention_budget()?)?;
    let coefficient = term(
        &mut f.base.cas,
        &f.base.header,
        crate::model::Operator::Rational(value.clone()),
        &[],
    )?;
    let output = admit(
        &mut f.base.cas,
        Draft::SeriesObject(crate::series::SeriesObject {
            field: Reference::Existing(f.field.clone()),
            centre: Reference::Existing(centre),
            variable: Reference::Existing(f.base.x.clone()),
            scope: Vec::new(),
            context: Reference::Existing(f.base.context.clone()),
            order: 1,
            coefficients: vec![Reference::Existing(coefficient)],
        }),
    )?;
    let statement = SourceStatement {
        clause: request.clause,
        source,
        domain: request.domain.clone(),
        output,
    };
    let certificate = SourceCertificate {
        clause: request.clause,
        coefficients: vec![value],
        companion: Vec::new(),
        units: Vec::new(),
    };
    if fresh_production {
        request.order = 2;
        let source =
            f.base
                .cas
                .expand_series_source(&request, &proofs, &mut source_retention_budget()?)?;
        let application = crate::proof_view::application(
            &f.base.cas,
            source.proof().logical_id(),
            &mut source_retention_budget()?,
        )?;
        let restriction = f.base.cas.compute_series(
            &O::Truncate {
                input: source_parts(&application)?.0.output.clone(),
                order: 1,
            },
            &[],
            &mut source_retention_budget()?,
        )?;
        request.clause = SourceClause::Truncate;
        request.order = 1;
        proofs = vec![source.proof().clone(), restriction.proof().clone()];
    }
    for index in 0..2100 {
        let value = term(
            &mut f.base.cas,
            &f.base.header,
            crate::model::Operator::Rational(crate::Rational::parse(
                &(index + 10).to_string(),
                &mut source_retention_budget()?,
            )?),
            &[],
        )?;
        f.base
            .cas
            .prepare_constant(&f.base.context, &value, &mut source_retention_budget()?)?;
    }
    let retained = f.base.cas.retained_judgment_bytes();
    assert!(retained > 1_048_576);
    let mut observations = Vec::new();
    let routes: &[(bool, u64)] = if fresh_production {
        // R + measured complete scratch + a 64-byte margin stays below 2R.
        assert!(retained + 1_027_792 < 2 * retained);
        &[(false, 1_027_727), (false, 1_027_792)]
    } else {
        &[(true, 200_000), (true, 100_000), (false, 4_000_000)]
    };
    for &(supplied, scratch) in routes {
        if !registered && scratch == 100_000 {
            continue;
        }
        let mut b = Budget::new(Limits {
            staged_payload_bytes: if fresh_production {
                retained + scratch
            } else if registered {
                retained + 64 + scratch
            } else {
                200_000
            },
            component_bits: 4096,
            ..source_retention_budget()?.limits().clone()
        })?;
        if registered {
            crate::decision_kernel::cache::retained(&f.base.cas, &mut b, |_| Ok(()))?;
            assert_eq!(b.checked_records.bytes, retained + 64);
        }
        let before = f
            .base
            .cas
            .canonical_snapshot(&mut source_retention_budget()?)?;
        let result = if supplied {
            f.base
                .cas
                .check_series_source(&statement, &certificate, &proofs, &mut b)
        } else {
            f.base.cas.expand_series_source(&request, &proofs, &mut b)
        };
        let succeeds = !supplied && (registered || (fresh_production && scratch == 1_027_792));
        if succeeds {
            let positive = result?;
            let bytes = f
                .base
                .cas
                .export_witness(&positive, &mut source_retention_budget()?)?;
            f.base
                .cas
                .replay(&bytes, positive.judgment(), &mut source_retention_budget()?)?;
            observations.push((
                None,
                b.usage().work,
                b.checked_records.bytes,
                b.usage().jet_entries,
            ));
        } else {
            let fault = result.expect_err("retained payload or incomplete polynomial");
            if registered && scratch == 200_000 {
                assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Rule));
            } else if registered || fresh_production {
                assert!(matches!(
                    fault.kind,
                    FaultKind::Exhausted {
                        budget: BudgetKind::StagedPayloadBytes,
                        ..
                    }
                ));
            } else {
                assert!(
                    matches!(fault.kind, FaultKind::Exhausted { budget: BudgetKind::StagedPayloadBytes, needed, .. } if needed == retained)
                );
            }
            observations.push((
                Some(fault),
                b.usage().work,
                b.checked_records.bytes,
                b.usage().jet_entries,
            ));
            assert_eq!(
                f.base
                    .cas
                    .canonical_snapshot(&mut source_retention_budget()?)?,
                before
            );
        }
        assert_eq!(
            f.base.cas.retained_judgment_bytes(),
            retained
                + if !succeeds {
                    0
                } else if fresh_production {
                    879
                } else {
                    1294
                }
        );
    }
    assert_eq!(
        retained,
        if fresh_production {
            1_101_418
        } else {
            1_099_416
        }
    );
    let at = crate::Location::new("formal series arithmetic");
    let exhausted = |limit, needed| {
        Some(crate::Fault::exhausted(
            BudgetKind::StagedPayloadBytes,
            limit,
            needed,
            at,
        ))
    };
    let expected = if fresh_production {
        vec![
            (
                Some(crate::Fault::exhausted(
                    BudgetKind::StagedPayloadBytes,
                    2_120_181,
                    2_120_182,
                    crate::Location::new("vertex admission"),
                )),
                14_956,
                0,
                0,
            ),
            (None, 115_551, 0, 0),
        ]
    } else if registered {
        assert_eq!(observations[0].3 * (4096 / 4 + 128), 107_136);
        vec![
            (
                Some(crate::Fault::invalid(InvalidCode::Rule, at)),
                240_729,
                0,
                93,
            ),
            (exhausted(1_105_380, 1_114_456), 220_761, 0, 93),
            (None, 560_209, 1_100_774, 188),
        ]
    } else {
        vec![
            (exhausted(198_060, 1_099_416), 3_286, 0, 0),
            (exhausted(192_272, 1_099_416), 7_356, 0, 0),
        ]
    };
    assert_eq!(observations, expected);
    Ok(())
}

/// Large setup and output ceilings leave the measured staged allowance as the only constraint.
fn source_retention_budget() -> TestResult<Budget> {
    Ok(Budget::new(Limits {
        output_bytes: 16_777_216,
        ..default_budget()?.limits().clone()
    })?)
}

fn fixture(reordered: bool) -> TestResult<Fixture> {
    let mut base = Base::new()?;
    for value in if reordered { ["3", "2"] } else { ["2", "3"] } {
        term(
            &mut base.cas,
            &base.header,
            crate::model::Operator::Rational(crate::Rational::parse(
                value,
                &mut default_budget()?,
            )?),
            &[],
        )?;
    }
    let field = admit(
        &mut base.cas,
        Draft::Solve(crate::solving::SolveDraft::CoefficientDomain(
            crate::solving::CoefficientDomainData::Rational,
        )),
    )?;
    Ok(Fixture { base, field })
}

fn object(base: &mut Base, field: &Handle, values: &[&str]) -> TestResult<Handle> {
    let mut coefficients = Vec::new();
    for text in values {
        let value = crate::Rational::parse(text, &mut default_budget()?)?;
        coefficients.push(Reference::Existing(term(
            &mut base.cas,
            &base.header,
            crate::model::Operator::Rational(value),
            &[],
        )?));
    }
    admit(
        &mut base.cas,
        Draft::SeriesObject(crate::series::SeriesObject {
            field: Reference::Existing(field.clone()),
            centre: Reference::Existing(base.zero.clone()),
            variable: Reference::Existing(base.x.clone()),
            scope: Vec::new(),
            context: Reference::Existing(base.context.clone()),
            order: u32::try_from(values.len())?,
            coefficients,
        }),
    )
}

fn index(bytes: &[u8], offset: &mut usize) -> TestResult<u32> {
    let value = u32::from_le_bytes(bytes.get(*offset..*offset + 4).ok_or("index")?.try_into()?);
    *offset += 4;
    Ok(value)
}

fn records(bytes: &[u8]) -> TestResult<Vec<(u8, std::ops::Range<usize>)>> {
    assert_eq!(&bytes[..8], b"NRZCAS05");
    let mut at = 8;
    let count = index(bytes, &mut at)?;
    for _ in 0..count {
        let len = index(bytes, &mut at)?;
        at += usize::try_from(len)? + 4;
    }
    let count = index(bytes, &mut at)?;
    let mut result = Vec::new();
    for _ in 0..count {
        let tag = *bytes.get(at).ok_or("tag")?;
        at += 1;
        let len = usize::try_from(index(bytes, &mut at)?)?;
        result.push((tag, at..at + len));
        at += len;
    }
    assert_eq!(at + 4, bytes.len());
    Ok(result)
}

/// A complete valid product retains many large rational recurrence rows.
fn large() -> TestResult<(Fixture, VerifiedWitness)> {
    let mut f = fixture(false)?;
    let text = "123456789012345678901234567890123456789012345678901234567890123";
    let input = object(&mut f.base, &f.field, &[text; 8])?;
    let witness = f.base.cas.compute_series(
        &O::Mul {
            left: input.clone(),
            right: input,
            order: None,
        },
        &[],
        &mut default_budget()?,
    )?;
    Ok((f, witness))
}

/// Wrong arity stops before proof sizing; correct arity reserves all owned copies together.
/// ´test:crate:series-live-premise-payload´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Both allocation orders keep exact aggregate arithmetic beside arity, rollback and valid construction controls"
)]
fn series_live_premise_payload() -> TestResult {
    let (source, original) = large()?;
    let bytes = source
        .base
        .cas
        .export_witness(&original, &mut default_budget()?)?;
    for reordered in [false, true] {
        let mut f = fixture(reordered)?;
        let witness = f.base.cas.inspect(&bytes, &mut default_budget()?)?;
        let proof = witness.proof().clone();
        // The operation's small coefficients keep its workspace independent of the
        // unrelated large proofs supplied in its two typed restriction slots.
        let input = object(&mut f.base, &f.field, &["1", "2"])?;
        let restriction = f.base.cas.compute_series(
            &O::Truncate {
                input: input.clone(),
                order: 1,
            },
            &[],
            &mut default_budget()?,
        )?;
        let payload = crate::proof_view::application_bytes(
            &f.base.cas,
            proof.logical_id(),
            &mut default_budget()?,
        )?;
        assert_eq!(payload, 30_612);
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        let mut budget = Budget::new(Limits {
            work: 257,
            ..Limits::default()
        })?;
        assert_eq!(
            f.base
                .cas
                .compute_series(
                    &O::Coefficient {
                        input: input.clone(),
                        index: 0
                    },
                    &[proof.clone(), proof.clone()],
                    &mut budget,
                )
                .expect_err("zero typed slots")
                .kind,
            FaultKind::Invalid(InvalidCode::Rule)
        );
        assert_eq!(budget.usage().work, 257);
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );

        // Measure the enclosing checkpoint on this exact admitted store.
        let mut probe = default_budget()?;
        let outer = probe.limits().staged_payload_bytes;
        let effective = f.base.cas.transaction(&mut probe, |_, budget| {
            Ok(budget.limits().staged_payload_bytes)
        })?;
        let checkpoint = outer - effective;
        assert_eq!(checkpoint, 640);
        // Order one, two empty scopes (eight encoded bytes), maximum component
        // width two: W = 2 * ((2 * 4 + 8) * 16 + 512) + 8 * 4.
        let workspace = 1_568;
        let needed = workspace + payload * 2;
        let effective = needed - 1;
        let allowance = checkpoint + effective;
        assert_eq!((allowance, effective, needed), (63_431, 62_791, 62_792));
        assert!(workspace + payload <= effective);
        assert!(effective < workspace + payload * 2);
        let operation = O::Add {
            left: input.clone(),
            right: input,
            order: Some(1),
        };
        let mut budget = Budget::new(Limits {
            staged_payload_bytes: allowance,
            ..default_budget()?.limits().clone()
        })?;
        let fault = f
            .base
            .cas
            .compute_series(&operation, &[proof.clone(), proof.clone()], &mut budget)
            .expect_err("the second live premise exceeds the combined allowance");
        assert_eq!(
            fault,
            crate::Fault::exhausted(
                BudgetKind::StagedPayloadBytes,
                effective,
                needed,
                crate::error::Location::new("formal series arithmetic"),
            )
        );
        assert_eq!(budget.usage().work, 179_311);
        assert_eq!(budget.limits().staged_payload_bytes, allowance);
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
        assert_eq!(
            f.base
                .cas
                .compute_series(&operation, &[proof.clone(), proof], &mut default_budget()?,)
                .expect_err("valid unrelated product is not a truncation")
                .kind,
            FaultKind::Invalid(InvalidCode::Rule)
        );
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
        let positive = f
            .base
            .cas
            .replay(&bytes, witness.judgment(), &mut default_budget()?)?;
        assert_eq!(
            f.base
                .cas
                .export_witness(&positive, &mut default_budget()?)?,
            bytes
        );
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
        // The same small Add has a valid checked construction with its actual restrictions.
        let positive = f.base.cas.compute_series(
            &operation,
            &[restriction.proof().clone(), restriction.proof().clone()],
            &mut default_budget()?,
        )?;
        let bytes = f
            .base
            .cas
            .export_witness(&positive, &mut default_budget()?)?;
        f.base
            .cas
            .replay(&bytes, positive.judgment(), &mut default_budget()?)?;
    }
    Ok(())
}

/// An isolated record-40 invocation retains the same checked earlier records as a full replay.
struct PortableCertificate {
    bytes: Vec<u8>,
    records: Vec<crate::wire_decode::Decoded>,
    manifest: Vec<crate::rule::Primitive>,
}

fn portable_certificate(
    cas: &mut crate::Cas,
    witness: &VerifiedWitness,
) -> TestResult<PortableCertificate> {
    use crate::domain_model::Layer;
    use crate::series::portable::{decode, describe};
    use crate::wire_build::Entity;
    use crate::wire_decode::Decoded;
    use crate::wire_read::Reader;
    let id = witness.proof().logical_id();
    let application = crate::proof_view::application(cas, id, &mut default_budget()?)?;
    let mut manifest = vec![application.rule];
    for citation in cas.proof_citations(witness.proof(), &mut default_budget()?)? {
        if !manifest.contains(&citation.primitive) {
            manifest.push(citation.primitive);
        }
    }
    let mut claim = describe(cas, &Entity::SeriesStatement(id), &mut default_budget()?)?;
    let mut records = Vec::new();
    for (offset, entity) in claim.references {
        let decoded = match entity {
            Entity::Object(id) => Decoded::Object(cas.handle(id)),
            Entity::Guard(guard) => Decoded::Guard(*guard),
            _ => return Err("claim dependency".into()),
        };
        assert_eq!(&claim.payload[offset..offset + 4], &0_u32.to_le_bytes());
        claim.payload[offset..offset + 4]
            .copy_from_slice(&u32::try_from(records.len())?.to_le_bytes());
        records.push(decoded);
    }
    let mut budget = default_budget()?;
    let mut reader = Reader::new(&claim.payload, &mut budget)?;
    let claim = decode(
        cas,
        39,
        Layer::Logic,
        None,
        &mut reader,
        &records,
        &manifest,
    )?;
    reader.finished()?;
    let mut records = vec![claim];
    let mut certificate = describe(cas, &Entity::SeriesCertificate(id), &mut default_budget()?)?;
    for (offset, entity) in certificate.references {
        let index = match entity {
            Entity::SeriesStatement(proof) if proof == id => 0,
            Entity::Object(proof) => {
                let index = u32::try_from(records.len())?;
                records.push(Decoded::Application(cas.handle(proof)));
                index
            }
            _ => return Err("certificate dependency".into()),
        };
        assert_eq!(
            &certificate.payload[offset..offset + 4],
            &0_u32.to_le_bytes()
        );
        certificate.payload[offset..offset + 4].copy_from_slice(&index.to_le_bytes());
    }
    Ok(PortableCertificate {
        bytes: certificate.payload,
        records,
        manifest,
    })
}

fn replay_certificate(
    cas: &mut crate::Cas,
    certificate: &PortableCertificate,
    budget: &mut Budget,
) -> Result<(), crate::Fault> {
    let mut reader = crate::wire_read::Reader::new(&certificate.bytes, budget)?;
    crate::series::portable::decode(
        cas,
        40,
        crate::domain_model::Layer::Logic,
        None,
        &mut reader,
        &certificate.records,
        &certificate.manifest,
    )?;
    reader.finished()
}

/// Earlier valid large proofs cannot be repeated into a certificate with no typed slots.
/// ´test:crate:series-repeated-portable-premise-payload´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "The full witness mutation and isolated consumed prefix retain their shared positive and rollback controls"
)]
fn series_repeated_portable_premise_payload() -> TestResult {
    let (source, witness) = large()?;
    let payload = crate::proof_view::application_bytes(
        &source.base.cas,
        witness.proof().logical_id(),
        &mut default_budget()?,
    )?;
    let bytes = source
        .base
        .cas
        .export_witness(&witness, &mut default_budget()?)?;
    let records = records(&bytes)?;
    let (_, certificate) = records
        .iter()
        .find(|(tag, _)| *tag == 40)
        .ok_or("certificate")?;
    assert_eq!(
        &bytes[certificate.start + 10..certificate.start + 15],
        &[3, 0, 0, 0, 0]
    );
    let root = index(&bytes, &mut (bytes.len() - 4))?;
    assert_eq!(records[usize::try_from(root)?].0, 41);
    let count_at = records.first().ok_or("first record")?.1.start - 9;
    assert_eq!(
        index(&bytes, &mut count_at.clone())?,
        u32::try_from(records.len())?
    );
    let mut added = bytes[certificate.clone()].to_vec();
    added[11..15].copy_from_slice(&64_u32.to_le_bytes());
    added.splice(15..15, (0..64).flat_map(|_| root.to_le_bytes()));
    let mut changed = bytes[..bytes.len() - 4].to_vec();
    changed[count_at..count_at + 4]
        .copy_from_slice(&(u32::try_from(records.len())? + 1).to_le_bytes());
    changed.push(40);
    changed.extend_from_slice(&u32::try_from(added.len())?.to_le_bytes());
    changed.extend_from_slice(&added);
    changed.extend_from_slice(&u32::try_from(records.len())?.to_le_bytes());
    for reordered in [false, true] {
        let mut receiver = fixture(reordered)?;
        let expected = receiver
            .base
            .cas
            .inspect(&bytes, &mut default_budget()?)?
            .judgment()
            .clone();
        let before = receiver
            .base
            .cas
            .canonical_snapshot(&mut default_budget()?)?;
        let allowance = payload * 8;
        assert!(512 * 64 < allowance && payload * 64 > allowance);
        let mut budget = Budget::new(Limits {
            staged_payload_bytes: allowance,
            ..default_budget()?.limits().clone()
        })?;
        assert_eq!(
            receiver
                .base
                .cas
                .replay(&changed, &expected, &mut budget)
                .expect_err("repeated references exceed typed arity")
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
        // Isolate the same record-40 boundary from the already checked prefix.
        // Only the reference, mode, count and fixed slot inventory may be charged.
        let positive = receiver.base.cas.inspect(&bytes, &mut default_budget()?)?;
        let mut isolated = portable_certificate(&mut receiver.base.cas, &positive)?;
        assert_eq!(&isolated.bytes[..9], &[0, 0, 0, 0, 3, 0, 0, 0, 0]);
        let reference = u32::try_from(isolated.records.len())?;
        isolated
            .records
            .push(crate::wire_decode::Decoded::Application(
                positive.proof().clone(),
            ));
        isolated.bytes[5..9].copy_from_slice(&64_u32.to_le_bytes());
        isolated
            .bytes
            .splice(9..9, (0..64).flat_map(|_| reference.to_le_bytes()));
        let mut budget = Budget::new(Limits {
            work: 32_814,
            staged_payload_bytes: allowance,
            ..default_budget()?.limits().clone()
        })?;
        let at = crate::error::Location::new("portable arity control");
        budget.work(37, at)?;
        let fault = replay_certificate(&mut receiver.base.cas, &isolated, &mut budget)
            .expect_err("arity precedes every dynamic premise inspection");
        assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Rule));
        // Measured 37 prior Work + 9 header bytes + 32,768 fixed slot bytes.
        // No Work remains even to read the first premise reference, let alone size it.
        assert_eq!(budget.usage().work, 32_814);
        assert_eq!(budget.limits().staged_payload_bytes, allowance);
        assert_eq!(
            receiver
                .base
                .cas
                .canonical_snapshot(&mut default_budget()?)?,
            before
        );

        let mut budget = Budget::new(Limits {
            staged_payload_bytes: allowance,
            ..default_budget()?.limits().clone()
        })?;
        let positive = receiver.base.cas.replay(&bytes, &expected, &mut budget)?;
        assert_eq!(
            receiver
                .base
                .cas
                .export_witness(&positive, &mut default_budget()?)?,
            bytes
        );
    }
    Ok(())
}

/// Correct typed arity still requires the simultaneous payloads of both restriction proofs.
/// ´test:crate:series-portable-aggregate-premise-payload´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Both allocation orders keep individual-copy controls beside aggregate exhaustion and unchanged full replay"
)]
fn series_portable_aggregate_premise_payload() -> TestResult {
    let mut source = fixture(false)?;
    let text = "123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789";
    let input = object(&mut source.base, &source.field, &[text; 9])?;
    let restriction = source.base.cas.compute_series(
        &O::Truncate {
            input: input.clone(),
            order: 8,
        },
        &[],
        &mut default_budget()?,
    )?;
    let witness = source.base.cas.compute_series(
        &O::Add {
            left: input.clone(),
            right: input,
            order: Some(8),
        },
        &[restriction.proof().clone(), restriction.proof().clone()],
        &mut default_budget()?,
    )?;
    let bytes = source
        .base
        .cas
        .export_witness(&witness, &mut default_budget()?)?;
    for reordered in [false, true] {
        let mut receiver = fixture(reordered)?;
        let positive = receiver.base.cas.inspect(&bytes, &mut default_budget()?)?;
        let certificate = portable_certificate(&mut receiver.base.cas, &positive)?;
        assert_eq!(&certificate.bytes[..9], &[0, 0, 0, 0, 1, 2, 0, 0, 0]);
        let application = crate::proof_view::application(
            &receiver.base.cas,
            positive.proof().logical_id(),
            &mut default_budget()?,
        )?;
        assert_eq!(application.premises.len(), 2);
        assert_eq!(application.premises[0], application.premises[1]);
        let mut aggregate = 0;
        for proof in &application.premises {
            aggregate += crate::proof_view::application_bytes(
                &receiver.base.cas,
                *proof,
                &mut default_budget()?,
            )?;
        }
        assert_eq!(aggregate, 8_504);
        let allowance = aggregate - 1;
        // Each copy actually succeeds in isolation under the same reduced allowance.
        for proof in &application.premises {
            let mut budget = Budget::new(Limits {
                staged_payload_bytes: allowance,
                ..default_budget()?.limits().clone()
            })?;
            crate::proof_view::application(&receiver.base.cas, *proof, &mut budget)?;
        }
        let before = receiver
            .base
            .cas
            .canonical_snapshot(&mut default_budget()?)?;
        let mut budget = Budget::new(Limits {
            staged_payload_bytes: allowance + 113,
            ..default_budget()?.limits().clone()
        })?;
        let at = crate::error::Location::new("portable premise control");
        budget.work(37, at)?;
        let previous = budget.reserve_temporary(113, at)?;
        let fault = replay_certificate(&mut receiver.base.cas, &certificate, &mut budget)
            .expect_err("both live restriction payloads exceed the allowance");
        assert_eq!(
            fault,
            crate::Fault::exhausted(
                BudgetKind::StagedPayloadBytes,
                allowance,
                aggregate,
                crate::error::Location::new("formal series arithmetic"),
            )
        );
        assert_eq!(budget.usage().work, 197_319);
        assert_eq!(budget.limits().staged_payload_bytes, allowance);
        budget.restore_limits(previous);
        assert_eq!(
            receiver
                .base
                .cas
                .canonical_snapshot(&mut default_budget()?)?,
            before
        );
        replay_certificate(&mut receiver.base.cas, &certificate, &mut default_budget()?)?;
        let unchanged =
            receiver
                .base
                .cas
                .replay(&bytes, positive.judgment(), &mut default_budget()?)?;
        assert_eq!(
            receiver
                .base
                .cas
                .export_witness(&unchanged, &mut default_budget()?)?,
            bytes
        );
        assert_eq!(
            receiver
                .base
                .cas
                .canonical_snapshot(&mut default_budget()?)?,
            before
        );
    }
    Ok(())
}

/// A checked finite congruence cannot replace an analytic side-jet premise.
/// ´test:crate:series-as-germ-premise-refused´
#[test]
fn series_as_germ_premise_refused() -> TestResult {
    for kind in 0..4 {
        calculus_as_germ_premise(kind)?;
    }
    Ok(())
}

/// Each finite producer is refused in the analytic jet slot while the original theorem remains valid.
#[expect(
    clippy::too_many_lines,
    reason = "The bridge control retains setup, valid evidence and rollback in one place"
)]
fn calculus_as_germ_premise(kind: usize) -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    let mut f = fixture(false)?;
    let input = object(&mut f.base, &f.field, &["1", "2"])?;
    let operation = match kind {
        0 => O::Truncate { input, order: 1 },
        1 => O::Derive { input },
        _ => O::Integrate {
            input,
            constant: f.base.one.clone(),
        },
    };
    let mut proofs = Vec::new();
    if kind == 2 {
        let predicate = admit(
            &mut f.base.cas,
            Draft::Condition {
                header: Reference::Existing(f.base.header.clone()),
                scope: Vec::new(),
                kind: crate::model::PredicateKind::Defined,
                arguments: vec![Reference::Existing(f.base.one.clone())],
            },
        )?;
        let mut draft = ApplicationDraft {
            rule: crate::rule::Primitive::CaseDomain,
            context: f.base.context.clone(),
            conclusion: crate::proof_model::Statement::PredicateFact {
                region: f.base.domain.clone(),
                predicate,
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: crate::proof_model::Certificate::None,
        };
        let (certificate, _) =
            crate::case_check::reconstruct(&mut f.base.cas, &draft, &[], &mut default_budget()?)?;
        draft.certificate = crate::proof_model::Certificate::Cases(Box::new(certificate));
        proofs.push(
            f.base
                .cas
                .check_proof(&f.base.context, &[draft], &mut default_budget()?)?
                .proof()
                .clone(),
        );
    }
    let source_request = crate::series::SourceRequest {
        clause: crate::series::SourceClause::PolynomialSeed,
        source: f.base.x.clone(),
        domain: f.base.domain.clone(),
        field: f.field.clone(),
        centre: f.base.zero.clone(),
        variable: f.base.x.clone(),
        scope: Vec::new(),
        context: f.base.context.clone(),
        order: 2,
    };
    let series = if kind == 3 {
        let x = f.base.x.clone();
        let ordinary = source_defined(&mut f, &x)?;
        f.base.cas.expand_series_source(
            &source_request,
            &[ordinary.proof().clone()],
            &mut default_budget()?,
        )?
    } else {
        f.base
            .cas
            .compute_series(&operation, &proofs, &mut default_budget()?)?
    };
    let point = crate::boundary_regions::boundary(
        &mut f.base.cas,
        &f.base.domain,
        &crate::Rational::zero(),
        &mut default_budget()?,
    )?;
    let request = crate::request_model::Request {
        task: crate::task::Task::Series(Box::new(crate::germ_model::SeriesAt {
            at: crate::germ_model::GermAt {
                coordinate: "x".into(),
                point,
                side: None,
            },
            order: 1,
            analytic: true,
        })),
        context: f.base.context.clone(),
        inputs: Vec::new(),
        premises: Vec::new(),
        limits: crate::budget::LimitsOverride::default(),
    };
    let result = crate::germ_producer::execute(
        &mut f.base.cas,
        &request,
        &[f.base.x.logical_id()],
        &crate::search::SearchPolicy::default(),
        &mut default_budget()?,
        &mut default_budget()?,
    )?;
    let application = crate::proof_view::application(
        &f.base.cas,
        result.witness.proof().logical_id(),
        &mut default_budget()?,
    )?;
    if kind == 3 {
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.base
                .cas
                .expand_series_source(
                    &source_request,
                    &[result.witness.proof().clone()],
                    &mut default_budget()?
                )
                .expect_err("germ is not source domain evidence")
                .kind,
            FaultKind::Invalid(InvalidCode::Rule)
        );
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
        let crate::proof_model::Statement::Series(value) = &series.judgment().statement else {
            return Err("source statement".into());
        };
        let crate::series::SeriesStatement::Source(value) = value.as_ref() else {
            return Err("source result".into());
        };
        let object_restriction = f.base.cas.compute_series(
            &O::Truncate {
                input: value.output.clone(),
                order: 1,
            },
            &[],
            &mut default_budget()?,
        )?;
        let mut restriction = source_request;
        restriction.clause = crate::series::SourceClause::Truncate;
        restriction.order = 1;
        let positive = f.base.cas.expand_series_source(
            &restriction,
            &[series.proof().clone(), object_restriction.proof().clone()],
            &mut default_budget()?,
        )?;
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.base
                .cas
                .expand_series_source(
                    &restriction,
                    &[
                        result.witness.proof().clone(),
                        object_restriction.proof().clone()
                    ],
                    &mut default_budget()?,
                )
                .expect_err("valid germ is not a source-congruence premise")
                .kind,
            FaultKind::Invalid(InvalidCode::Rule),
        );
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
        f.base
            .cas
            .export_witness(&positive, &mut default_budget()?)?;
    }
    let mut premises = application
        .premises
        .iter()
        .map(|id| {
            Ok(crate::proof_view::application(
                &f.base.cas,
                *id,
                &mut default_budget()?,
            )?)
        })
        .collect::<TestResult<Vec<_>>>()?;
    let draft = ApplicationDraft {
        context: application.judgment.context,
        rule: application.rule,
        conclusion: application.judgment.statement,
        premises: (0..premises.len())
            .map(u32::try_from)
            .collect::<Result<_, _>>()?,
        substitutions: application.substitutions,
        certificate: application.certificate,
    };
    let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
    crate::scoped_check::check_application(
        &mut f.base.cas,
        &draft,
        &premises,
        &mut default_budget()?,
    )?;
    let original = premises.first().ok_or("analytic jet")?.clone();
    premises[0] = crate::proof_view::application(
        &f.base.cas,
        series.proof().logical_id(),
        &mut default_budget()?,
    )?;
    assert_eq!(
        f.base
            .cas
            .checked(&premises[0].judgment.context, &mut budget)?,
        f.base.cas.checked(&draft.context, &mut budget)?
    );
    let fault = crate::scoped_check::check_application(
        &mut f.base.cas,
        &draft,
        &premises,
        &mut default_budget()?,
    )
    .expect_err("finite congruence is not an analytic jet");
    assert_eq!(fault.kind, crate::case_syntax::invalid().kind);
    assert_eq!(
        f.base.cas.canonical_snapshot(&mut default_budget()?)?,
        before
    );
    premises[0] = original;
    crate::scoped_check::check_application(
        &mut f.base.cas,
        &draft,
        &premises,
        &mut default_budget()?,
    )?;
    Ok(())
}

/// A complex original region enlarges ordinary definedness evidence while coefficients stay one.
fn large_ordinary_constant() -> TestResult<(Fixture, VerifiedWitness)> {
    use crate::domain_model::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
    use crate::model::{Operator, PredicateKind};
    use crate::proof_model::{Certificate, Statement};
    let mut f = fixture(false)?;
    let mut members = Vec::new();
    for index in 0..8 {
        let prefix = "1234567890".repeat(24);
        let value = term(
            &mut f.base.cas,
            &f.base.header,
            Operator::Rational(crate::Rational::parse(
                &format!("{prefix}{index}"),
                &mut default_budget()?,
            )?),
            &[],
        )?;
        let difference = term(
            &mut f.base.cas,
            &f.base.header,
            Operator::Sub,
            &[f.base.x.clone(), value],
        )?;
        let predicate = admit(
            &mut f.base.cas,
            Draft::Condition {
                header: Reference::Existing(f.base.header.clone()),
                scope: Vec::new(),
                kind: PredicateKind::Positive,
                arguments: vec![Reference::Existing(difference)],
            },
        )?;
        members.push(Reference::Existing(admit(
            &mut f.base.cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(f.base.header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Real("x".into()),
                expression: RegionExpr::GuardTrue {
                    base: Reference::Existing(f.base.domain.clone()),
                    predicate: Reference::Existing(predicate),
                },
            }),
        )?));
    }
    f.base.domain = admit(
        &mut f.base.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(f.base.header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Real("x".into()),
            expression: RegionExpr::Union(members),
        }),
    )?;
    f.base.context = admit(
        &mut f.base.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(f.base.header.clone()),
            admissible: Reference::Existing(f.base.domain.clone()),
            assumptions: Vec::new(),
        }),
    )?;
    let predicate = admit(
        &mut f.base.cas,
        Draft::Condition {
            header: Reference::Existing(f.base.header.clone()),
            scope: Vec::new(),
            kind: PredicateKind::Defined,
            arguments: vec![Reference::Existing(f.base.one.clone())],
        },
    )?;
    let mut draft = ApplicationDraft {
        rule: crate::rule::Primitive::CaseDomain,
        context: f.base.context.clone(),
        conclusion: Statement::PredicateFact {
            region: f.base.domain.clone(),
            predicate,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    };
    let (certificate, conditions) =
        crate::case_check::reconstruct(&mut f.base.cas, &draft, &[], &mut default_budget()?)?;
    assert_eq!(conditions.requirements, []);
    assert_eq!(conditions.residuals, []);
    assert_eq!(conditions.hypotheses, []);
    assert!(certificate.cells.iter().any(|cells| cells.cuts.len() == 8));
    draft.certificate = Certificate::Cases(Box::new(certificate));
    let witness = f
        .base
        .cas
        .check_proof(&f.base.context, &[draft], &mut default_budget()?)?;
    assert!(witness.is_discharged());
    Ok((f, witness))
}

/// One ordinary slot reserves its full payload beside candidate workspace; this is not two-slot evidence.
/// ´test:crate:series-integral-ordinary-premise-payload´
#[test]
fn series_integral_ordinary_premise_payload() -> TestResult {
    let (mut f, ordinary) = large_ordinary_constant()?;
    let input = object(&mut f.base, &f.field, &["1"])?;
    let operation = O::Integrate {
        input,
        constant: f.base.one.clone(),
    };
    let proof = ordinary.proof().clone();
    let payload = crate::proof_view::application_bytes(
        &f.base.cas,
        proof.logical_id(),
        &mut default_budget()?,
    )?;
    let mut probe = default_budget()?;
    let outer = probe.limits().staged_payload_bytes;
    let effective = f.base.cas.transaction(&mut probe, |_, budget| {
        Ok(budget.limits().staged_payload_bytes)
    })?;
    let checkpoint = outer - effective;
    // Output count two, maximum width two (integer workspace), one four-byte empty scope.
    let workspace = 3_600;
    let needed = workspace + payload;
    let effective = needed - 1;
    let allowance = checkpoint + effective;
    assert!(payload <= effective && workspace <= effective);
    let mut copy_budget = Budget::new(Limits {
        staged_payload_bytes: effective,
        ..default_budget()?.limits().clone()
    })?;
    crate::proof_view::application(&f.base.cas, proof.logical_id(), &mut copy_budget)?;
    let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
    let mut budget = Budget::new(Limits {
        staged_payload_bytes: allowance,
        ..default_budget()?.limits().clone()
    })?;
    let fault = f
        .base
        .cas
        .compute_series(&operation, std::slice::from_ref(&proof), &mut budget)
        .expect_err("ordinary payload coexists with candidate workspace");
    assert_eq!(
        fault,
        crate::Fault::exhausted(
            BudgetKind::StagedPayloadBytes,
            effective,
            needed,
            crate::Location::new("formal series arithmetic")
        )
    );
    assert_eq!((payload, checkpoint, workspace), (4408, 704, 3600));
    assert_eq!((allowance, effective, needed), (8711, 8007, 8008));
    assert_eq!(budget.usage().work, 18_465);
    assert_eq!(budget.limits().staged_payload_bytes, allowance);
    assert_eq!(
        f.base.cas.canonical_snapshot(&mut default_budget()?)?,
        before
    );
    let mut early = Budget::new(Limits {
        work: 257,
        ..default_budget()?.limits().clone()
    })?;
    assert_eq!(
        f.base
            .cas
            .compute_series(&operation, &[proof.clone(), proof.clone()], &mut early)
            .expect_err("surplus slots precede dynamic sizing")
            .kind,
        FaultKind::Invalid(InvalidCode::Rule)
    );
    assert_eq!(early.usage().work, 257);
    assert_eq!(
        f.base.cas.canonical_snapshot(&mut default_budget()?)?,
        before
    );
    f.base
        .cas
        .compute_series(&operation, &[proof], &mut default_budget()?)?;
    f.base
        .cas
        .export_witness(&ordinary, &mut default_budget()?)?;
    Ok(())
}

/// Reconstruct ordinary definedness on the original large region, without an assumed predicate.
fn source_defined(f: &mut Fixture, source: &Handle) -> TestResult<VerifiedWitness> {
    use crate::proof_model::{Certificate, Statement};
    let predicate = admit(
        &mut f.base.cas,
        Draft::Condition {
            header: Reference::Existing(f.base.header.clone()),
            scope: Vec::new(),
            kind: crate::model::PredicateKind::Defined,
            arguments: vec![Reference::Existing(source.clone())],
        },
    )?;
    let mut draft = ApplicationDraft {
        rule: crate::rule::Primitive::CaseDomain,
        context: f.base.context.clone(),
        conclusion: Statement::PredicateFact {
            region: f.base.domain.clone(),
            predicate,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    };
    let (certificate, _) =
        crate::case_check::reconstruct(&mut f.base.cas, &draft, &[], &mut default_budget()?)?;
    draft.certificate = Certificate::Cases(Box::new(certificate));
    Ok(f.base
        .cas
        .check_proof(&f.base.context, &[draft], &mut default_budget()?)?)
}

/// A valid retained numerical enclosure is not an exact source or domain application.
/// ´test:crate:series-source-enclosure-slot-refused´
#[test]
fn series_source_enclosure_slot_refused() -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    use crate::enclosure::{EnclosureGoal, EnclosureRequest, RefinementPolicy};
    let mut f = fixture(false)?;
    let request = EnclosureRequest {
        target: f.base.one.clone(),
        context: f.base.context.clone(),
        coordinates: Vec::new(),
        goal: EnclosureGoal::Width,
    };
    let policy = RefinementPolicy {
        version: 1,
        precision: 4,
        grid_ceiling: 64,
        rounds: 2,
        reduction_ceiling: 16,
        term_ceiling: 64,
        checking_work: 50_000,
    };
    let report = f
        .base
        .cas
        .enclose(&request, policy, &mut default_budget()?, &mut |_| false)?;
    assert!(report.checked.is_some());
    let ids: Vec<_> = f
        .base
        .cas
        .state
        .indices
        .keys()
        .filter(|id| {
            matches!(
                f.base.cas.state.node(**id, &mut budget),
                Ok(crate::graph::Vertex::AnonymousLemma(_))
            )
        })
        .copied()
        .collect();
    assert_eq!(ids.len(), 1);
    let id = ids[0];
    let source = crate::series::SourceRequest {
        clause: crate::series::SourceClause::PolynomialSeed,
        source: f.base.one.clone(),
        domain: f.base.domain.clone(),
        field: f.field,
        centre: f.base.zero.clone(),
        variable: f.base.x.clone(),
        scope: Vec::new(),
        context: f.base.context.clone(),
        order: 1,
    };
    let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
    assert!(matches!(
        f.base
            .cas
            .expand_series_source(&source, &[f.base.cas.handle(id)], &mut default_budget()?)
            .expect_err("enclosure is not an exact proof slot")
            .kind,
        FaultKind::Invalid(InvalidCode::UnsupportedFragment)
    ));
    assert_eq!(
        f.base.cas.canonical_snapshot(&mut default_budget()?)?,
        before
    );
    f.base.cas.check_enclosure(
        &request,
        policy,
        report
            .checked
            .as_ref()
            .ok_or("checked enclosure")?
            .certificate(),
        &mut default_budget()?,
        &mut |_| false,
    )?;
    Ok(())
}

/// Seed, sine origin or the repeated sine product over the large ordinary region, with ordered live slots.
#[expect(
    clippy::too_many_lines,
    reason = "The repeated product retains its origin, object product and composite hypothesis in one closed fixture"
)]
fn source_aggregate_fixture(
    kind: usize,
) -> TestResult<(Fixture, crate::series::SourceRequest, Vec<Handle>)> {
    use crate::model::Operator;
    use crate::series::{SourceClause, SourceRequest};
    let (mut f, ordinary) = large_ordinary_constant()?;
    let mut source = f.base.one.clone();
    let mut proofs = vec![ordinary.proof().clone()];
    let mut clause = SourceClause::PolynomialSeed;
    let mut composite_guard = None;
    if kind != 0 {
        let x = f.base.x.clone();
        source = term(
            &mut f.base.cas,
            &f.base.header,
            Operator::Sine,
            std::slice::from_ref(&x),
        )?;
        if kind == 2 {
            use crate::domain_model::{ContextDraft, HypothesisRef};
            use crate::graph::AssumptionDraft;
            use crate::proof_model::{Certificate, Statement};
            let composite = term(
                &mut f.base.cas,
                &f.base.header,
                Operator::Mul,
                &[source.clone(), source.clone()],
            )?;
            let predicate = admit(
                &mut f.base.cas,
                Draft::Condition {
                    header: Reference::Existing(f.base.header.clone()),
                    scope: Vec::new(),
                    kind: crate::model::PredicateKind::Defined,
                    arguments: vec![Reference::Existing(composite)],
                },
            )?;
            f.base.context = admit(
                &mut f.base.cas,
                Draft::Context(ContextDraft::Root {
                    header: Reference::Existing(f.base.header.clone()),
                    admissible: Reference::Existing(f.base.domain.clone()),
                    assumptions: vec![AssumptionDraft {
                        key: "original-composite-defined".into(),
                        condition: Reference::Existing(predicate.clone()),
                    }],
                }),
            )?;
            let draft = ApplicationDraft {
                rule: crate::rule::Primitive::ContextHypothesis,
                context: f.base.context.clone(),
                conclusion: Statement::PredicateFact {
                    region: f.base.domain.clone(),
                    predicate,
                },
                premises: Vec::new(),
                substitutions: Vec::new(),
                certificate: Certificate::Hypothesis(HypothesisRef {
                    context: f.base.context.clone(),
                    slot: 0,
                }),
            };
            composite_guard = Some(f.base.cas.check_proof(
                &f.base.context,
                &[draft],
                &mut default_budget()?,
            )?);
        }
        let coordinate = source_defined(&mut f, &x)?;
        let total = f
            .base
            .cas
            .trig_defined(&source, coordinate.proof(), &mut default_budget()?)?;
        proofs = vec![total.proof().clone()];
        clause = SourceClause::SineOrigin;
    }
    let mut request = SourceRequest {
        clause,
        source,
        domain: f.base.domain.clone(),
        field: f.field.clone(),
        centre: f.base.zero.clone(),
        variable: f.base.x.clone(),
        scope: Vec::new(),
        context: f.base.context.clone(),
        order: 2,
    };
    if kind == 2 {
        let sine = f
            .base
            .cas
            .expand_series_source(&request, &proofs, &mut default_budget()?)?;
        let crate::proof_model::Statement::Series(value) = &sine.judgment().statement else {
            return Err("source statement".into());
        };
        let crate::series::SeriesStatement::Source(value) = value.as_ref() else {
            return Err("source result".into());
        };
        let input = value.output.clone();
        let product = f.base.cas.compute_series(
            &O::Mul {
                left: input.clone(),
                right: input,
                order: Some(2),
            },
            &[],
            &mut default_budget()?,
        )?;
        let composite = term(
            &mut f.base.cas,
            &f.base.header,
            Operator::Mul,
            &[request.source.clone(), request.source.clone()],
        )?;
        let composite_defined = composite_guard.as_ref().ok_or("composite hypothesis")?;
        request.source = composite;
        request.clause = SourceClause::Mul;
        proofs = vec![
            sine.proof().clone(),
            sine.proof().clone(),
            product.proof().clone(),
            composite_defined.proof().clone(),
        ];
    }
    Ok((f, request, proofs))
}

/// One origin or seed slot and four mixed slots retain every simultaneous occurrence.
/// ´test:crate:series-source-aggregate-premise-payload´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Measured reservations, repeated slots and rollback share the same complete source fixture"
)]
fn series_source_aggregate_premise_payload() -> TestResult {
    for kind in 0..3 {
        let (mut f, request, proofs) = source_aggregate_fixture(kind)?;
        let mut payloads = Vec::new();
        for proof in &proofs {
            payloads.push(crate::proof_view::application_bytes(
                &f.base.cas,
                proof.logical_id(),
                &mut default_budget()?,
            )?);
        }
        let mut probe = default_budget()?;
        let outer = probe.limits().staged_payload_bytes;
        let effective = f
            .base
            .cas
            .transaction(&mut probe, |_, b| Ok(b.limits().staged_payload_bytes))?;
        let enclosing = outer - effective;
        let workspace =
            u64::from(request.order) * 4 * (default_budget()?.limits().component_bits / 4 + 512);
        let needed = workspace + payloads.iter().sum::<u64>();
        let effective = needed - 1;
        let allowance = enclosing + effective;
        for (proof, payload) in proofs.iter().zip(&payloads) {
            assert!(*payload <= effective);
            let mut b = Budget::new(Limits {
                staged_payload_bytes: effective,
                ..default_budget()?.limits().clone()
            })?;
            crate::proof_view::application(&f.base.cas, proof.logical_id(), &mut b)?;
        }
        if kind == 2 {
            assert_eq!(proofs[0], proofs[1]);
            assert!(workspace + payloads.iter().sum::<u64>() - payloads[1] <= effective);
        }
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        let mut b = Budget::new(Limits {
            staged_payload_bytes: allowance,
            ..default_budget()?.limits().clone()
        })?;
        let fault = f
            .base
            .cas
            .expand_series_source(&request, &proofs, &mut b)
            .expect_err("aggregate includes each owned slot");
        assert_eq!(
            fault,
            crate::Fault::exhausted(
                BudgetKind::StagedPayloadBytes,
                effective,
                needed,
                crate::Location::new("formal series arithmetic")
            )
        );
        let expected = [
            (640, 12_288, vec![4408], 17_335, 16_695, 16_696, 30_534),
            (640, 12_288, vec![592], 13_519, 12_879, 12_880, 13_220),
            (
                768,
                12_288,
                vec![980, 980, 1292, 700],
                17_007,
                16_239,
                16_240,
                17_996,
            ),
        ];
        assert_eq!(
            (
                enclosing,
                workspace,
                payloads.clone(),
                allowance,
                effective,
                needed,
                b.usage().work
            ),
            expected[kind]
        );
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
        for slots in [
            Vec::new(),
            vec![proofs[0].clone(); request.clause.arity() + 1],
        ] {
            let mut early = Budget::new(Limits {
                work: 256,
                ..default_budget()?.limits().clone()
            })?;
            assert_eq!(
                f.base
                    .cas
                    .expand_series_source(&request, &slots, &mut early)
                    .expect_err("typed arity precedes sizing")
                    .kind,
                FaultKind::Invalid(InvalidCode::Rule)
            );
            assert_eq!(early.usage().work, 256);
        }
        let positive =
            f.base
                .cas
                .expand_series_source(&request, &proofs, &mut default_budget()?)?;
        let bytes = f
            .base
            .cas
            .export_witness(&positive, &mut default_budget()?)?;
        f.base
            .cas
            .replay(&bytes, positive.judgment(), &mut default_budget()?)?;
        let mut certificate = portable_certificate(&mut f.base.cas, &positive)?;
        assert_eq!(&certificate.bytes[..4], &[0; 4]);
        assert_eq!(certificate.bytes[4], request.clause.mode());
        assert_eq!(
            index(&certificate.bytes, &mut 5)?,
            u32::try_from(proofs.len())?
        );
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        let aggregate = payloads.iter().sum::<u64>();
        let mut b = Budget::new(Limits {
            staged_payload_bytes: aggregate - 1,
            ..default_budget()?.limits().clone()
        })?;
        b.work(37, crate::Location::new("source portable reservation"))?;
        let fault = replay_certificate(&mut f.base.cas, &certificate, &mut b)
            .expect_err("all portable premise copies coexist");
        assert_eq!(
            fault,
            crate::Fault::exhausted(
                BudgetKind::StagedPayloadBytes,
                aggregate - 1,
                aggregate,
                crate::Location::new(if kind == 2 {
                    "formal series arithmetic"
                } else {
                    "scalar statement view"
                })
            )
        );
        assert_eq!(
            (aggregate, b.usage().work),
            [(4408, 14_144), (592, 646), (3952, 7577)][kind]
        );
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
        replay_certificate(&mut f.base.cas, &certificate, &mut default_budget()?)?;
        certificate.bytes[5..9].copy_from_slice(&u32::try_from(proofs.len() + 1)?.to_le_bytes());
        let mut early = Budget::new(Limits {
            work: 100,
            ..default_budget()?.limits().clone()
        })?;
        assert_eq!(
            replay_certificate(&mut f.base.cas, &certificate, &mut early)
                .expect_err("portable typed arity before sizing")
                .kind,
            FaultKind::Invalid(InvalidCode::Rule)
        );
        assert_eq!(early.usage().work, 9);
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
    }
    Ok(())
}

/// Seed x, sine origin or the repeated sine product on the large region D, every ordinary slot a
/// visible context hypothesis.
///
/// Ordinary case reconstruction admits syntax against the whole retained store and a constant
/// subject fills the exact constant cache, so supplied steps calibrate the source aggregate only
/// when no earlier step needs more staged payload than the source slot itself.
#[expect(
    clippy::too_many_lines,
    reason = "Every visible hypothesis, the origin and the repeated product share one closed context"
)]
fn source_assumed_fixture(
    kind: usize,
) -> TestResult<(Fixture, crate::series::SourceRequest, Vec<Handle>)> {
    use crate::domain_model::{ContextDraft, HypothesisRef};
    use crate::graph::AssumptionDraft;
    use crate::model::{Operator, PredicateKind};
    use crate::proof_model::{Certificate, Statement};
    use crate::series::{SourceClause, SourceRequest};
    let (mut f, _) = large_ordinary_constant()?;
    let x = f.base.x.clone();
    let source = if kind == 0 {
        x.clone()
    } else {
        term(
            &mut f.base.cas,
            &f.base.header,
            Operator::Sine,
            std::slice::from_ref(&x),
        )?
    };
    let mut subjects = vec![source.clone()];
    if kind == 2 {
        subjects.push(term(
            &mut f.base.cas,
            &f.base.header,
            Operator::Mul,
            &[source.clone(), source.clone()],
        )?);
    }
    let mut predicates = Vec::new();
    let mut assumptions = Vec::new();
    for (slot, subject) in subjects.iter().enumerate() {
        let predicate = admit(
            &mut f.base.cas,
            Draft::Condition {
                header: Reference::Existing(f.base.header.clone()),
                scope: Vec::new(),
                kind: PredicateKind::Defined,
                arguments: vec![Reference::Existing(subject.clone())],
            },
        )?;
        assumptions.push(AssumptionDraft {
            key: format!("original-defined-{slot}"),
            condition: Reference::Existing(predicate.clone()),
        });
        predicates.push(predicate);
    }
    f.base.context = admit(
        &mut f.base.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(f.base.header.clone()),
            admissible: Reference::Existing(f.base.domain.clone()),
            assumptions,
        }),
    )?;
    let mut hypotheses = Vec::new();
    for (slot, predicate) in predicates.into_iter().enumerate() {
        let draft = ApplicationDraft {
            rule: crate::rule::Primitive::ContextHypothesis,
            context: f.base.context.clone(),
            conclusion: Statement::PredicateFact {
                region: f.base.domain.clone(),
                predicate,
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::Hypothesis(HypothesisRef {
                context: f.base.context.clone(),
                slot: slot.try_into()?,
            }),
        };
        hypotheses.push(
            f.base
                .cas
                .check_proof(&f.base.context, &[draft], &mut default_budget()?)?
                .proof()
                .clone(),
        );
    }
    let mut request = SourceRequest {
        clause: if kind == 0 {
            SourceClause::PolynomialSeed
        } else {
            SourceClause::SineOrigin
        },
        source,
        domain: f.base.domain.clone(),
        field: f.field.clone(),
        centre: f.base.zero.clone(),
        variable: x,
        scope: Vec::new(),
        context: f.base.context.clone(),
        order: 2,
    };
    let mut proofs = vec![hypotheses[0].clone()];
    if kind == 2 {
        let sine = f
            .base
            .cas
            .expand_series_source(&request, &proofs, &mut default_budget()?)?;
        let application = crate::proof_view::application(
            &f.base.cas,
            sine.proof().logical_id(),
            &mut default_budget()?,
        )?;
        let input = source_parts(&application)?.0.output.clone();
        let product = f.base.cas.compute_series(
            &O::Mul {
                left: input.clone(),
                right: input,
                order: Some(2),
            },
            &[],
            &mut default_budget()?,
        )?;
        request.source = subjects[1].clone();
        request.clause = SourceClause::Mul;
        proofs = vec![
            sine.proof().clone(),
            sine.proof().clone(),
            product.proof().clone(),
            hypotheses[1].clone(),
        ];
    }
    Ok((f, request, proofs))
}

/// Each ordered slot's dynamic application payload, counted per occurrence.
fn slot_payloads(f: &Fixture, proofs: &[Handle]) -> TestResult<Vec<u64>> {
    let mut payloads = Vec::new();
    for proof in proofs {
        payloads.push(crate::proof_view::application_bytes(
            &f.base.cas,
            proof.logical_id(),
            &mut default_budget()?,
        )?);
    }
    Ok(payloads)
}

/// The proof transaction's own staged reservation, measured rather than assumed.
fn enclosing_reservation(f: &mut Fixture) -> TestResult<u64> {
    let mut probe = default_budget()?;
    let outer = probe.limits().staged_payload_bytes;
    let effective = f
        .base
        .cas
        .transaction(&mut probe, |_, b| Ok(b.limits().staged_payload_bytes))?;
    Ok(outer - effective)
}

/// Every individual owned copy fits the calibrated effective limit on its own.
fn single_copies_fit(
    f: &Fixture,
    proofs: &[Handle],
    payloads: &[u64],
    effective: u64,
) -> TestResult {
    for (proof, payload) in proofs.iter().zip(payloads) {
        assert!(*payload <= effective);
        let mut b = Budget::new(Limits {
            staged_payload_bytes: effective,
            ..default_budget()?.limits().clone()
        })?;
        crate::proof_view::application(&f.base.cas, proof.logical_id(), &mut b)?;
    }
    Ok(())
}

/// The produced source statement and rows, borrowed from their checked application.
fn source_parts(
    application: &crate::proof_model::CheckedApplication,
) -> TestResult<(
    &crate::series::SourceStatement,
    &crate::series::SourceCertificate,
)> {
    use crate::proof_model::{Certificate, Statement};
    use crate::series::{SeriesCertificate, SeriesStatement};
    let Statement::Series(statement) = &application.judgment.statement else {
        return Err("source statement".into());
    };
    let SeriesStatement::Source(statement) = statement.as_ref() else {
        return Err("source result".into());
    };
    let Certificate::Series(certificate) = &application.certificate else {
        return Err("source certificate".into());
    };
    let SeriesCertificate::Source(certificate) = certificate.as_ref() else {
        return Err("source rows".into());
    };
    Ok((statement, certificate))
}

/// Re-express a checked proof closure as supplied steps, each distinct proof once, root last.
fn proof_steps(cas: &crate::Cas, root: &Handle) -> TestResult<Vec<ApplicationDraft>> {
    let mut steps = Vec::new();
    let mut indices: Vec<(crate::graph::LogicalId, u32)> = Vec::new();
    let mut pending = vec![(root.logical_id(), false)];
    while let Some((id, expanded)) = pending.pop() {
        if indices.iter().any(|(seen, _)| *seen == id) {
            continue;
        }
        let application = crate::proof_view::application(cas, id, &mut default_budget()?)?;
        if !expanded {
            pending.push((id, true));
            pending.extend(application.premises.iter().rev().map(|p| (*p, false)));
            continue;
        }
        let mut premises = Vec::new();
        for premise in &application.premises {
            let (_, index) = indices
                .iter()
                .find(|(seen, _)| seen == premise)
                .ok_or("premise precedes its consumer")?;
            premises.push(*index);
        }
        indices.push((id, u32::try_from(steps.len())?));
        steps.push(ApplicationDraft {
            rule: application.rule,
            context: application.judgment.context.clone(),
            conclusion: application.judgment.statement.clone(),
            premises,
            substitutions: application.substitutions.clone(),
            certificate: application.certificate.clone(),
        });
    }
    Ok(steps)
}

/// An isolated record-41 invocation over the same checked claim and ordered premise records.
fn portable_conclusion(
    cas: &mut crate::Cas,
    witness: &VerifiedWitness,
) -> TestResult<PortableCertificate> {
    use crate::series::portable::describe;
    use crate::wire_build::Entity;
    use crate::wire_decode::Decoded;
    let id = witness.proof().logical_id();
    let PortableCertificate {
        mut records,
        manifest,
        ..
    } = portable_certificate(cas, witness)?;
    records.truncate(1);
    records.push(Decoded::Application(witness.proof().clone()));
    let mut conclusion = describe(cas, &Entity::Object(id), &mut default_budget()?)?;
    for (offset, entity) in conclusion.references {
        let index = match entity {
            Entity::SeriesStatement(proof) if proof == id && offset == 0 => 0,
            Entity::SeriesCertificate(proof) if proof == id && offset == 4 => 1,
            Entity::Object(proof) => {
                let index = u32::try_from(records.len())?;
                records.push(Decoded::Application(cas.handle(proof)));
                index
            }
            _ => return Err("conclusion dependency".into()),
        };
        assert_eq!(
            &conclusion.payload[offset..offset + 4],
            &0_u32.to_le_bytes()
        );
        conclusion.payload[offset..offset + 4].copy_from_slice(&index.to_le_bytes());
    }
    Ok(PortableCertificate {
        bytes: conclusion.payload,
        records,
        manifest,
    })
}

fn replay_conclusion(
    cas: &mut crate::Cas,
    conclusion: &PortableCertificate,
    budget: &mut Budget,
) -> Result<(), crate::Fault> {
    let mut reader = crate::wire_read::Reader::new(&conclusion.bytes, budget)?;
    crate::series::portable::decode(
        cas,
        41,
        crate::domain_model::Layer::Logic,
        None,
        &mut reader,
        &conclusion.records,
        &conclusion.manifest,
    )?;
    reader.finished()
}

/// Supplied source rows refuse impossible arity before sizing and own every slot through publication.
/// ´test:crate:series-source-supplied-premise-payload´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Measured reservations, arity refusals and rollback share each produced source candidate"
)]
fn series_source_supplied_premise_payload() -> TestResult {
    let mut observed = Vec::new();
    for kind in 0..3 {
        let (mut f, request, proofs) = source_aggregate_fixture(kind)?;
        let positive =
            f.base
                .cas
                .expand_series_source(&request, &proofs, &mut default_budget()?)?;
        let application = crate::proof_view::application(
            &f.base.cas,
            positive.proof().logical_id(),
            &mut default_budget()?,
        )?;
        let (statement, certificate) = source_parts(&application)?;
        let payloads = slot_payloads(&f, &proofs)?;
        let workspace = crate::series::codec::source_workspace(
            &application.certificate,
            &mut default_budget()?,
        )?;
        let enclosing = enclosing_reservation(&mut f)?;
        let needed = workspace + payloads.iter().sum::<u64>();
        let effective = needed - 1;
        let allowance = enclosing + effective;
        single_copies_fit(&f, &proofs, &payloads, effective)?;
        if kind == 2 {
            assert_eq!(proofs[0], proofs[1]);
            assert!(needed - payloads[1] <= effective);
        }
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        let mut b = Budget::new(Limits {
            staged_payload_bytes: allowance,
            ..default_budget()?.limits().clone()
        })?;
        assert_eq!(
            f.base
                .cas
                .check_series_source(statement, certificate, &proofs, &mut b)
                .expect_err("supplied rows own every slot"),
            crate::Fault::exhausted(
                BudgetKind::StagedPayloadBytes,
                effective,
                needed,
                crate::Location::new("formal series arithmetic")
            )
        );
        let mut arity = Vec::new();
        for slots in [
            Vec::new(),
            vec![proofs[0].clone(); request.clause.arity() + 1],
        ] {
            let mut early = Budget::new(Limits {
                work: 256,
                ..default_budget()?.limits().clone()
            })?;
            assert_eq!(
                f.base
                    .cas
                    .check_series_source(statement, certificate, &slots, &mut early)
                    .expect_err("typed arity precedes sizing")
                    .kind,
                FaultKind::Invalid(InvalidCode::Rule)
            );
            assert!(payloads.iter().all(|p| early.usage().work < *p));
            arity.push(early.usage().work);
        }
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
        f.base
            .cas
            .check_series_source(statement, certificate, &proofs, &mut default_budget()?)?;
        observed.push((
            enclosing,
            workspace,
            payloads,
            allowance,
            effective,
            needed,
            b.usage().work,
            arity,
        ));
    }
    assert_eq!(
        observed,
        [
            (
                768,
                352,
                vec![4408],
                5527,
                4759,
                4760,
                18_302,
                vec![256, 256]
            ),
            (768, 1604, vec![592], 2963, 2195, 2196, 1205, vec![256, 256]),
            (
                768,
                52,
                vec![980, 980, 1292, 700],
                4771,
                4003,
                4004,
                5721,
                vec![256, 256]
            ),
        ]
    );
    Ok(())
}

/// Supplied proof steps refuse impossible source arity in preflight; one-slot steps own all scratch.
///
/// The repeated product's earlier object multiplication step reserves retained judgments beside
/// its checking scratch and exceeds the product's outer four-slot aggregate boundary. The live
/// production and supplied-candidate controls isolate that outer boundary: E is the transaction
/// reservation, before the source arithmetic's retained-judgment reservation. Their limits are
/// not complete-operation minima; the full-operation and fresh/registered-owner controls cover
/// the later retained payload and arithmetic. No generic supplied-proof reservation is assumed.
/// ´test:crate:series-source-proof-step-premise-payload´
#[test]
fn series_source_proof_step_premise_payload() -> TestResult {
    let mut observed = Vec::new();
    for kind in 0..3 {
        let (mut f, request, proofs) = source_assumed_fixture(kind)?;
        let positive =
            f.base
                .cas
                .expand_series_source(&request, &proofs, &mut default_budget()?)?;
        let steps = proof_steps(&f.base.cas, positive.proof())?;
        let root = steps.last().ok_or("root")?;
        assert_eq!(root.premises.len(), proofs.len());
        let payloads = slot_payloads(&f, &proofs)?;
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        let aggregate = if kind == 2 {
            assert_eq!(root.premises[0], root.premises[1]);
            None
        } else {
            let workspace =
                crate::series::codec::source_workspace(&root.certificate, &mut default_budget()?)?;
            let enclosing = enclosing_reservation(&mut f)?;
            let needed = workspace + payloads.iter().sum::<u64>();
            let effective = needed - 1;
            let allowance = enclosing + effective;
            single_copies_fit(&f, &proofs, &payloads, effective)?;
            let mut b = Budget::new(Limits {
                staged_payload_bytes: allowance,
                ..default_budget()?.limits().clone()
            })?;
            assert_eq!(
                f.base
                    .cas
                    .check_proof(&f.base.context, &steps, &mut b)
                    .expect_err("supplied step owns workspace and slot together"),
                crate::Fault::exhausted(
                    BudgetKind::StagedPayloadBytes,
                    effective,
                    needed,
                    crate::Location::new("typed proof admission")
                )
            );
            Some((
                enclosing,
                workspace,
                allowance,
                effective,
                needed,
                b.usage().work,
            ))
        };
        let mut arity = Vec::new();
        for slots in [
            Vec::new(),
            vec![root.premises[0]; request.clause.arity() + 1],
        ] {
            let mut bad = steps.clone();
            bad.last_mut().ok_or("root")?.premises = slots;
            let mut early = Budget::new(Limits {
                work: u64::try_from(steps.len())?,
                ..default_budget()?.limits().clone()
            })?;
            assert_eq!(
                f.base
                    .cas
                    .check_proof(&f.base.context, &bad, &mut early)
                    .expect_err("preflight arity precedes sizing")
                    .kind,
                FaultKind::Invalid(InvalidCode::Rule)
            );
            assert!(payloads.iter().all(|p| early.usage().work < *p));
            arity.push(early.usage().work);
        }
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
        let checked = f
            .base
            .cas
            .check_proof(&f.base.context, &steps, &mut default_budget()?)?;
        assert_eq!(checked.judgment(), positive.judgment());
        observed.push((steps.len(), payloads, aggregate, arity));
    }
    assert_eq!(
        observed,
        [
            (
                2,
                vec![700],
                Some((768, 648, 2115, 1347, 1348, 4029)),
                vec![2, 2]
            ),
            (
                2,
                vec![700],
                Some((768, 1604, 3071, 2303, 2304, 5510)),
                vec![2, 2]
            ),
            (5, vec![1048, 1048, 1292, 700], None, vec![5, 5]),
        ]
    );
    Ok(())
}

/// Isolated record 41 refuses impossible source arity before sizing its conclusion copy.
/// ´test:crate:series-source-conclusion-arity´
#[test]
fn series_source_conclusion_arity() -> TestResult {
    let mut observed = Vec::new();
    for kind in 0..3 {
        let (mut f, request, proofs) = source_aggregate_fixture(kind)?;
        let positive =
            f.base
                .cas
                .expand_series_source(&request, &proofs, &mut default_budget()?)?;
        let mut conclusion = portable_conclusion(&mut f.base.cas, &positive)?;
        assert_eq!(
            index(&conclusion.bytes, &mut 8)?,
            u32::try_from(proofs.len())?
        );
        replay_conclusion(&mut f.base.cas, &conclusion, &mut default_budget()?)?;
        let mut payloads = slot_payloads(&f, &proofs)?;
        payloads.push(crate::proof_view::application_bytes(
            &f.base.cas,
            positive.proof().logical_id(),
            &mut default_budget()?,
        )?);
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        let mut arity = Vec::new();
        for count in [0, proofs.len() + 1] {
            conclusion.bytes[8..12].copy_from_slice(&u32::try_from(count)?.to_le_bytes());
            let mut early = Budget::new(Limits {
                work: 100,
                ..default_budget()?.limits().clone()
            })?;
            assert_eq!(
                replay_conclusion(&mut f.base.cas, &conclusion, &mut early)
                    .expect_err("portable conclusion arity precedes sizing")
                    .kind,
                FaultKind::Invalid(InvalidCode::Rule)
            );
            arity.push(early.usage().work);
        }
        assert!(payloads.iter().all(|p| 100 < *p));
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
        observed.push((payloads, arity));
    }
    assert_eq!(
        observed,
        [
            (vec![4408, 760], vec![12, 12]),
            (vec![592, 980], vec![12, 12]),
            (vec![980, 980, 1292, 700, 872], vec![12, 12]),
        ]
    );
    Ok(())
}

/// A series lemma's new-publication test follows occupancy even when its serial has gaps.
/// ´test:crate:logical-identity-series-publication´
#[test]
fn logical_identity_series_publication() -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    let mut f = fixture(false)?;
    let input = object(&mut f.base, &f.field, &["1", "2", "3"])?;
    let operation = O::Truncate {
        input: input.clone(),
        order: 2,
    };
    let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
    let counts = f.base.cas.counts();
    let proof_count = f.base.cas.state.proof_count(&mut budget)?;
    let fault = crate::Fault::invalid(
        InvalidCode::Rule,
        crate::Location::new("series identity control"),
    );
    let mut stale = None;
    let mut abandoned = Vec::new();
    let result: Result<(), crate::Fault> =
        f.base
            .cas
            .transaction(&mut default_budget()?, |cas, budget| {
                stale = Some(cas.compute_series(&operation, &[], budget)?);
                abandoned.extend(
                    cas.state
                        .indices
                        .keys()
                        .filter(|id| id.kind == crate::graph::VertexKind::AnonymousLemma)
                        .map(|id| cas.handle(*id)),
                );
                Err(fault.clone())
            });
    assert_eq!(result.as_ref(), Err(&fault));
    assert_eq!(f.base.cas.counts(), counts);
    assert_eq!(f.base.cas.state.proof_count(&mut budget)?, proof_count);
    assert_eq!(f.base.cas.retained_judgment_count(), 0);
    assert_eq!(
        f.base.cas.canonical_snapshot(&mut default_budget()?)?,
        before
    );
    let stale = stale.ok_or("discarded series witness")?;
    assert_eq!(
        f.base
            .cas
            .checked(stale.proof(), &mut budget)
            .err()
            .ok_or("discarded series proof")?
            .kind,
        FaultKind::Invalid(InvalidCode::Reference)
    );
    let mut budget = Budget::new(Limits {
        proof_nodes: proof_count + 1,
        ..Limits::default()
    })?;
    let kept = f.base.cas.compute_series(&operation, &[], &mut budget)?;
    assert_eq!(kept.bytes, stale.bytes);
    assert_ne!(kept.proof(), stale.proof());
    assert!(kept.proof().logical_id().serial > stale.proof().logical_id().serial);
    assert_eq!(f.base.cas.state.proof_count(&mut budget)?, proof_count + 1);
    assert_eq!(f.base.cas.retained_judgment_count(), 1);
    assert_eq!(abandoned.len(), 1);
    for handle in &abandoned {
        assert_eq!(
            f.base
                .cas
                .checked(handle, &mut budget)
                .err()
                .ok_or("series lemma reincarnated")?
                .kind,
            FaultKind::Invalid(InvalidCode::Reference)
        );
    }
    let lemma = f
        .base
        .cas
        .state
        .indices
        .keys()
        .find(|id| id.kind == crate::graph::VertexKind::AnonymousLemma)
        .copied()
        .ok_or("retained series lemma")?;
    f.base.cas.series_application(lemma, &mut budget)?;
    assert!(lemma.serial > abandoned[0].logical_id().serial);
    let counts = f.base.cas.counts();
    let again = f
        .base
        .cas
        .compute_series(&operation, &[], &mut default_budget()?)?;
    assert_eq!(again.proof(), kept.proof());
    assert_eq!(f.base.cas.counts(), counts);
    f.base.cas.checked(&input, &mut budget)?;
    let mut fresh = crate::Cas::new(Limits::default())?;
    let imported = fresh.inspect(&kept.bytes, &mut default_budget()?)?;
    assert_eq!(
        fresh.export_witness(&imported, &mut default_budget()?)?,
        kept.bytes
    );
    Ok(())
}
