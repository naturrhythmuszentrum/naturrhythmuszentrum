// SPDX-License-Identifier: AGPL-3.0-only
//! Exact source-derived boundaries with a caller-owned original baseline and nonzero prefixes.
use super::*;
mod authentication;
mod inspection;

/// A caller-supplied original-union allowance, not a bindings measurement of mixed owners.
const ORIGINAL: u64 = 1_000_424;
/// Independent unrelated caller residence remains reserved across every attempt.
const CALLER: u64 = 37;
/// Earlier consumed work is never replaced by a fresh nested ledger.
const PRIOR: u64 = 7;

/// The caller hands off only the captured owner increment, keeping its own scope intact.
fn run(
    token: &LcOmegaDeclarationToken,
    certificate: Option<&nrz_cas::enclosure::lc_root::Certificate>,
    budget: &mut Budget,
) -> Result<LcOmegaCorrespondence, AdmissionError> {
    budget.with_retained_payload(CALLER, AT, |budget| {
        budget.with_retained_payload(ORIGINAL - 424, AT, |budget| {
            if let Some(certificate) = certificate {
                LcOmegaCorrespondence::replay(token, certificate, budget)
            } else {
                LcOmegaCorrespondence::construct(token, budget)
            }
        })
    })
}

/// Source-derived tariffs and byte unions admit exactly and refuse one below on both routes.
/// ´test:unit:lc-omega-correspondence-resource-boundaries´
#[test]
fn lc_omega_correspondence_resource_boundaries() -> TestResult {
    assert_eq!(
        (
            size_of::<nrz_cas::Cas>(),
            size_of::<nrz_cas::syntax::Draft>(),
            size_of::<nrz_cas::Reference>(),
            size_of::<nrz_cas::syntax::Declaration>(),
            size_of::<nrz_cas::Rational>()
        ),
        (984, 328, 24, 144, 72)
    );
    let mut preparation = Budget::new(Limits::default())?;
    let f = fixture(&mut preparation)?;
    let token = owning::admit(f.request(), &mut preparation)?;
    crate::lc_omega::correspondence::controls::source_tariffs(
        &token,
        &mut Budget::new(Limits::default())?,
    )?;
    let certificate = run(&token, None, &mut preparation)?.certificate(&mut preparation)?;
    for proposed in [None, Some(&certificate)] {
        inspection::boundaries(&token, proposed)?;
        authentication::boundaries(&token, proposed)?;
        work(&token, proposed)?;
        bytes(&token, proposed, BudgetKind::OutputBytes)?;
        bytes(&token, proposed, BudgetKind::StagedPayloadBytes)?;
    }
    Ok(())
}

/// The last prepaid frozen-owner edge fails after the private CAS has published its law.
fn work(
    token: &LcOmegaDeclarationToken,
    certificate: Option<&nrz_cas::enclosure::lc_root::Certificate>,
) -> TestResult {
    let exact = PRIOR + oracle::work(certificate.is_some());
    for ceiling in [exact, exact - 1] {
        let mut budget = Budget::new(Limits {
            work: ceiling,
            ..Limits::default()
        })?;
        budget.work(PRIOR, AT)?;
        let staged = budget.limits().staged_payload_bytes;
        let result = run(token, certificate, &mut budget);
        if ceiling == exact {
            let value = result?;
            assert_eq!(
                value.retained_bytes(),
                oracle::retained(certificate.is_some())
            );
            assert_eq!(budget.usage().work, exact);
        } else {
            let Err(AdmissionError::Algebra(ref fault)) = result else {
                return Err("expected post-CAS exhaustion".into());
            };
            assert_eq!(fault.location.operation, "LC omega correspondence");
            exhausted(result, BudgetKind::Work, exact - 1, exact)?;
            assert_eq!(budget.usage().work, exact - 2);
        }
        assert_eq!(budget.limits().staged_payload_bytes, staged);
    }
    Ok(())
}

/// Output counts only new retained ownership; staged includes caller P, original U and all overlap.
fn bytes(
    token: &LcOmegaDeclarationToken,
    certificate: Option<&nrz_cas::enclosure::lc_root::Certificate>,
    kind: BudgetKind,
) -> TestResult {
    let replay = certificate.is_some();
    let exact = if kind == BudgetKind::OutputBytes {
        oracle::retained(replay)
    } else {
        CALLER + ORIGINAL + oracle::staged(replay)
    };
    for ceiling in [exact, exact - 1] {
        let mut limits = Limits::default();
        if kind == BudgetKind::OutputBytes {
            limits.output_bytes = ceiling;
        } else {
            limits.staged_payload_bytes = ceiling;
        }
        let mut budget = Budget::new(limits)?;
        budget.work(PRIOR, AT)?;
        let staged = budget.limits().staged_payload_bytes;
        let result = run(token, certificate, &mut budget);
        if ceiling == exact {
            assert_eq!(result?.retained_bytes(), oracle::retained(replay));
        } else {
            let Err(AdmissionError::Algebra(fault)) = result else {
                return Err("expected byte exhaustion".into());
            };
            let nrz_cas::FaultKind::Exhausted {
                budget: actual,
                limit,
                needed,
                ..
            } = fault.kind
            else {
                return Err("expected exhausted kind".into());
            };
            assert_eq!(actual, kind);
            assert_eq!(needed, limit + 1);
        }
        assert_eq!(budget.limits().staged_payload_bytes, staged);
        budget.check(BudgetKind::StagedPayloadBytes, staged, AT)?;
    }
    Ok(())
}

/// Early and post-publication refusal preserves the original owner and allows a complete retry.
/// ´test:unit:lc-omega-correspondence-failure-ownership´
#[test]
fn lc_omega_correspondence_failure_ownership() -> TestResult {
    let mut preparation = Budget::new(Limits::default())?;
    let f = fixture(&mut preparation)?;
    let token = owning::admit(f.request(), &mut preparation)?;
    let shared = token.share(&mut preparation)?;
    let question = f.question.clone();
    let output = f.output.clone();
    for ceiling in [PRIOR, PRIOR + oracle::work(false) - 1] {
        let mut budget = Budget::new(Limits {
            work: ceiling,
            ..Limits::default()
        })?;
        budget.work(PRIOR, AT)?;
        let staged = budget.limits().staged_payload_bytes;
        let result = run(&token, None, &mut budget);
        if ceiling == PRIOR {
            exhausted(result, BudgetKind::Work, PRIOR, PRIOR + 1)?;
            assert_eq!(budget.usage().work, PRIOR);
        } else {
            exhausted(result, BudgetKind::Work, ceiling, ceiling + 1)?;
            assert_eq!(budget.usage().work, ceiling - 1);
        }
        assert_eq!(budget.limits().staged_payload_bytes, staged);
        token.check_same(&shared, &mut preparation)?;
        assert!(f.question.shares_allocation(&question));
        assert_eq!(f.output.value(), output.value());
        assert_eq!(f.output.bound(), output.bound());
        assert_eq!(f.output.error(), output.error());
    }
    let mut late = Budget::new(Limits {
        work: PRIOR + oracle::work(false) - 1,
        ..Limits::default()
    })?;
    late.work(PRIOR, AT)?;
    let result = late.with_retained_payload(CALLER + ORIGINAL - 424, AT, |budget| {
        crate::lc_omega::correspondence::controls::published_failure(&token, budget)
    });
    exhausted(
        result,
        BudgetKind::Work,
        PRIOR + oracle::work(false) - 1,
        PRIOR + oracle::work(false),
    )?;
    let retried = run(&token, None, &mut preparation)?;
    retried.check_declaration(&token, &mut preparation)?;
    // Independent authentication cases do not consume the construction-failure setup ledger.
    authentication::failures(&token, &mut Budget::new(Limits::default())?)?;
    Ok(())
}
