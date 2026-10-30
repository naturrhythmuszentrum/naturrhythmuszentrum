// SPDX-License-Identifier: AGPL-3.0-only
//! Complete Machin byte keys, unchanged cold checking and bounded warm citations.

use super::*;
use crate::budget::Limits;
use crate::decision_kernel::cache::{atomic, retained, synchronized};
use crate::encoding::Encoder;
use crate::testing::common::TestResult;
use crate::{FaultKind, InvalidCode, Rational};

/// Ordinary finite ledger shared by setup and the measured checker.
fn ledger() -> Result<Budget, Fault> {
    Budget::new(Limits::default())
}

/// The existing two/one-term LC Machin theorem instance.
fn certificate() -> TestResult<PiBoundCertificate> {
    let mut budget = ledger()?;
    Ok(PiBoundCertificate {
        version: 1,
        terms: [2, 1],
        sums: [
            Rational::from_ratio(74, 375, &mut budget)?,
            Rational::from_ratio(1, 239, &mut budget)?,
        ],
        lower: Rational::from_ratio(128, 41, &mut budget)?,
        upper: Rational::from_ratio(640, 203, &mut budget)?,
    })
}

/// Match the same guarded, atomic reservation used by both public enclosure origins.
fn check(cas: &Cas, certificate: &PiBoundCertificate, budget: &mut Budget) -> Result<u64, Fault> {
    synchronized(cas, budget, |budget| {
        atomic(cas, budget, |budget| {
            retained(cas, budget, |budget| validate(cas, certificate, budget))
        })
    })
}

/// Borrowed key bytes equal the existing canonical encoder for signed, zero and multi-limb values.
/// ´test:unit:machin-cache-canonical-byte-view´
/// ´claim:cas:machin-cache-canonical-byte-view´
#[test]
fn machin_cache_canonical_byte_view() -> TestResult {
    for value in ["74/375", "0", "-1/3", "4294967297/1099511627791"] {
        let mut certificate = certificate()?;
        let mut budget = ledger()?;
        certificate.sums[0] = Rational::parse(value, &mut budget)?;
        let mut out = Encoder::new(&mut budget, AT);
        out.unsigned(u64::from(certificate.version))?;
        for terms in certificate.terms {
            out.unsigned(u64::from(terms))?;
        }
        for number in [
            &certificate.sums[0],
            &certificate.sums[1],
            &certificate.lower,
            &certificate.upper,
        ] {
            out.number(number)?;
        }
        let expected = out.finish();
        assert_eq!(certificate.canonical_key()?.collect::<Vec<_>>(), expected);
        assert_eq!(certificate.key_bytes(&budget)?, size(expected.len(), AT)?);
    }
    Ok(())
}

/// One store checks the full Machin certificate once and later cites its identical byte key.
/// ´test:unit:machin-cache-reuses-certificate´
/// ´claim:cas:machin-cache-reuses-certificate´
#[test]
fn machin_cache_reuses_certificate() -> TestResult {
    let cas = Cas::new(Limits::default())?;
    let certificate = certificate()?;
    let mut cold = ledger()?;
    assert_eq!(check(&cas, &certificate, &mut cold)?, 680);
    let mut warm = ledger()?;
    assert_eq!(check(&cas, &certificate, &mut warm)?, 680);
    assert_eq!(cold.usage().work, 17_499);
    assert_eq!(warm.usage().work, 70);
    assert_eq!(records(&cas)?.pi.len(), 1);
    assert_eq!(records(&cas)?.bytes, 936);
    assert_eq!(warm.checked_records.bytes, 1_000);
    let mut independent = ledger()?;
    certificate.check(&mut independent)?;
    assert_eq!(independent.usage().work, 18_179);
    Ok(())
}

/// Change one complete certificate field while preserving the other claims.
fn change(certificate: &mut PiBoundCertificate, kind: u8) -> TestResult<InvalidCode> {
    let mut budget = ledger()?;
    match kind {
        0 => {
            certificate.version = 2;
            Ok(InvalidCode::UnsupportedVersion)
        }
        1 => {
            certificate.terms[0] = 3;
            Ok(InvalidCode::EndpointMismatch)
        }
        2 => {
            let value = Rational::from_ratio(73, 375, &mut budget)?;
            assert_eq!(value.component_bits(), certificate.sums[0].component_bits());
            assert_eq!(value.is_negative(), certificate.sums[0].is_negative());
            certificate.sums[0] = value;
            Ok(InvalidCode::EndpointMismatch)
        }
        3 => {
            certificate.sums[1] = Rational::from_ratio(1, 237, &mut budget)?;
            Ok(InvalidCode::EndpointMismatch)
        }
        4 => {
            certificate.lower = Rational::from_ratio(129, 41, &mut budget)?;
            Ok(InvalidCode::DecisionBox)
        }
        _ => {
            certificate.upper = Rational::from_ratio(636, 203, &mut budget)?;
            Ok(InvalidCode::DecisionBox)
        }
    }
}

/// Every mutated field misses the checked byte key and refuses at the finite Machin checker.
/// ´test:unit:machin-cache-forged-fields´
/// ´claim:cas:machin-cache-forged-fields´
#[test]
fn machin_cache_forged_fields() -> TestResult {
    let cas = Cas::new(Limits::default())?;
    let original = certificate()?;
    check(&cas, &original, &mut ledger()?)?;
    for kind in 0..6 {
        let mut changed = original.clone();
        let expected = change(&mut changed, kind)?;
        assert!(!original.canonical_key()?.eq(changed.canonical_key()?));
        let Err(fault) = check(&cas, &changed, &mut ledger()?) else {
            panic!("mutated Machin field must refuse at the finite checker");
        };
        assert_eq!(fault.kind, FaultKind::Invalid(expected));
        assert_eq!(fault.location, Location::new("Machin pi bounds"));
        assert_eq!(records(&cas)?.pi.len(), 1);
        assert_eq!(records(&cas)?.bytes, 936);
    }
    Ok(())
}

/// Fresh stores, rollback and a saturated generation never reuse a previous Machin judgment.
/// ´test:unit:machin-cache-owner-and-rollback´
/// ´claim:cas:machin-cache-owner-and-rollback´
#[test]
fn machin_cache_owner_and_rollback() -> TestResult {
    let mut cas = Cas::new(Limits::default())?;
    let certificate = certificate()?;
    check(&cas, &certificate, &mut ledger()?)?;
    let fresh = Cas::new(Limits::default())?;
    let mut independent = ledger()?;
    check(&fresh, &certificate, &mut independent)?;
    assert_eq!(independent.usage().work, 17_499);
    let Err(fault) = cas.transaction(&mut ledger()?, |cas, budget| {
        check(cas, &certificate, budget)?;
        Err::<(), _>(Fault::invalid(InvalidCode::Reference, AT))
    }) else {
        panic!("deliberate Machin transaction refusal");
    };
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Reference));
    assert_eq!(records(&cas)?.pi.len(), 0);
    let mut after = ledger()?;
    check(&cas, &certificate, &mut after)?;
    assert_eq!(after.usage().work, 17_499);
    cas.memo_epoch = u64::MAX;
    for _ in 0..2 {
        let mut saturated = ledger()?;
        check(&cas, &certificate, &mut saturated)?;
        assert_eq!(saturated.usage().work, 17_499);
        assert_eq!(records(&cas)?.pi.len(), 0);
    }
    Ok(())
}

/// Cold and warm literal Work boundaries preserve the monotone ledger and atomic retention.
/// ´test:unit:machin-cache-work-boundaries´
/// ´claim:cas:machin-cache-work-boundaries´
#[test]
fn machin_cache_work_boundaries() -> TestResult {
    let certificate = certificate()?;
    for (warm, needed) in [(false, 17_516), (true, 87)] {
        for work in [needed, needed - 1] {
            let cas = Cas::new(Limits::default())?;
            if warm {
                check(&cas, &certificate, &mut ledger()?)?;
            }
            let mut budget = Budget::new(Limits {
                work,
                ..Limits::default()
            })?;
            budget.work(17, AT)?;
            let result = check(&cas, &certificate, &mut budget);
            if work == needed {
                assert_eq!(result?, 680);
                assert_eq!(budget.usage().work, needed);
                assert_eq!(records(&cas)?.pi.len(), 1);
            } else {
                let Err(fault) = result else {
                    panic!("complete Machin validation or citation must refuse below N");
                };
                assert!(matches!(fault.kind, FaultKind::Exhausted {
                    budget: BudgetKind::Work, limit, needed: actual, ..
                } if limit == work && actual == needed));
                assert!(budget.usage().work >= 17);
                assert_eq!(records(&cas)?.pi.len(), usize::from(warm));
                assert_eq!(budget.checked_records.bytes, 0);
            }
        }
    }
    Ok(())
}

/// The live LC certificate's distinct dyadic bounds have their own complete 73-byte citation.
/// ´test:unit:machin-cache-dyadic-citation´
/// ´claim:cas:machin-cache-dyadic-citation´
#[test]
fn machin_cache_dyadic_citation() -> TestResult {
    let cas = Cas::new(Limits::default())?;
    let mut certificate = certificate()?;
    certificate.lower = Rational::from_ratio(3215, 1024, &mut ledger()?)?;
    certificate.upper = Rational::from_ratio(1609, 512, &mut ledger()?)?;
    let mut cold = ledger()?;
    assert_eq!(check(&cas, &certificate, &mut cold)?, 692);
    assert_eq!(cold.usage().work, 18_019);
    for work in [90, 89] {
        let mut budget = Budget::new(Limits {
            work,
            ..Limits::default()
        })?;
        budget.work(17, AT)?;
        let result = check(&cas, &certificate, &mut budget);
        if work == 90 {
            assert_eq!(result?, 692);
            assert_eq!(budget.usage().work, 90);
        } else {
            let Err(fault) = result else {
                panic!("dyadic Machin citation must refuse one below its complete key cost");
            };
            assert!(matches!(
                fault.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::Work,
                    limit: 89,
                    needed: 90,
                    ..
                }
            ));
            assert_eq!(budget.usage().work, 17);
        }
    }
    Ok(())
}

/// A later enclosing operation cannot publish a new Pi judgment after its own refusal.
/// ´test:unit:machin-cache-late-refusal´
/// ´claim:cas:machin-cache-late-refusal´
#[test]
fn machin_cache_late_refusal() -> TestResult {
    let cas = Cas::new(Limits::default())?;
    let certificate = certificate()?;
    let mut budget = Budget::new(Limits {
        work: 17_499,
        ..Limits::default()
    })?;
    let Err(fault) = synchronized(&cas, &mut budget, |budget| {
        atomic(&cas, budget, |budget| {
            retained(&cas, budget, |budget| {
                validate(&cas, &certificate, budget)?;
                budget.work(1, AT)
            })
        })
    }) else {
        panic!("late enclosing operation must exhaust after complete Pi checking");
    };
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::Work,
            limit: 17_499,
            needed: 17_500,
            ..
        }
    ));
    assert_eq!(budget.usage().work, 17_499);
    assert_eq!(budget.checked_records.bytes, 0);
    assert_eq!(records(&cas)?.pi.len(), 0);
    assert_eq!(records(&cas)?.bytes, 0);
    assert_eq!(records(&cas)?.pi.capacity(), 0);
    Ok(())
}

/// A refused multi-certificate attempt releases speculative capacity while preserving prior evidence.
/// ´test:unit:machin-cache-refusal-releases-growth´
/// ´claim:cas:machin-cache-refusal-releases-growth´
#[test]
fn machin_cache_refusal_releases_growth() -> TestResult {
    let cas = Cas::new(Limits::default())?;
    let original = certificate()?;
    check(&cas, &original, &mut ledger()?)?;
    let mut budget = ledger()?;
    let Err(fault) = synchronized(&cas, &mut budget, |budget| {
        atomic(&cas, budget, |budget| {
            retained(&cas, budget, |budget| {
                for lower in -8..0 {
                    let mut changed = original.clone();
                    changed.lower = Rational::from_ratio(lower, 1, budget)?;
                    validate(&cas, &changed, budget)?;
                }
                Err::<(), _>(Fault::invalid(InvalidCode::Reference, AT))
            })
        })
    }) else {
        panic!("deliberate enclosing refusal must discard all new Machin judgments");
    };
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Reference));
    assert_eq!(records(&cas)?.pi.len(), 1);
    assert_eq!(records(&cas)?.pi.capacity(), 1);
    assert_eq!(records(&cas)?.bytes, 936);
    assert_eq!(budget.checked_records.bytes, 0);
    assert!(budget.usage().work > 17_499);
    let mut citation = ledger()?;
    check(&cas, &original, &mut citation)?;
    assert_eq!(citation.usage().work, 70);
    Ok(())
}

/// Complete retained certificate storage remains reserved beside one extra caller byte.
/// ´test:unit:machin-cache-storage-boundaries´
/// ´claim:cas:machin-cache-storage-boundaries´
#[test]
fn machin_cache_storage_boundaries() -> TestResult {
    let cas = Cas::new(Limits::default())?;
    check(&cas, &certificate()?, &mut ledger()?)?;
    for bytes in [1_001, 1_000] {
        let limits = Limits {
            staged_payload_bytes: bytes,
            ..Limits::default()
        };
        let mut budget = Budget::new(limits.clone())?;
        let result = retained(&cas, &mut budget, |budget| {
            budget.with_retained_payload(1, AT, |budget| {
                budget.check(BudgetKind::StagedPayloadBytes, 0, AT)
            })
        });
        if bytes == 1_001 {
            result?;
        } else {
            let Err(fault) = result else {
                panic!("checked Pi storage must reserve the caller's extra byte");
            };
            assert!(matches!(
                fault.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::StagedPayloadBytes,
                    limit: 1_000,
                    needed: 1_001,
                    ..
                }
            ));
        }
        assert_eq!(budget.limits(), &limits);
        assert_eq!(budget.usage().work, 0);
        assert_eq!(records(&cas)?.bytes, 936);
    }
    Ok(())
}
