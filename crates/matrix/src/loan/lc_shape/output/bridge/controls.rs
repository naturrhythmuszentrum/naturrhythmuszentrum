// SPDX-License-Identifier: AGPL-3.0-only
//! Crate-only corruption witnesses remain distinct from reachable public refusals.
use super::{LcOmegaCandidate, LcOmegaOutput, LcShapeError, prepare_lc_omega_correspondence};
use crate::{ExecutionAccount, Heap, LcShapeTile};
use nrz_bindings::{AdmissionError, Registry};
use nrz_scalar::Scalar;

/// Independent declaration owners cannot be substituted even when their originals match.
pub(in crate::loan::lc_shape) fn defensive<P: Scalar + 'static, const N: usize>(
    output: &LcOmegaOutput<'_, P, N>,
    changed_output: &LcOmegaOutput<'_, P, N>,
    heap: &Heap,
    tiles: &[&LcShapeTile<'_, P>],
    registry: &Registry,
    account: &mut ExecutionAccount,
) -> Result<(), Box<dyn std::error::Error>> {
    let original = prepare_lc_omega_correspondence(output, registry, None, account)?;
    let separate = prepare_lc_omega_correspondence(output, registry, None, account)?;
    let mismatched = LcOmegaCandidate {
        token: separate.token,
        correspondence: original.correspondence,
    };
    let result = mismatched.authenticate(output, registry, &mut account.ledger);
    assert!(matches!(
        result,
        Err(LcShapeError::Declaration(AdmissionError::ValueMismatch))
    ));
    drop(mismatched);
    let original = prepare_lc_omega_correspondence(output, registry, None, account)?;
    let result = original.authenticate(changed_output, registry, &mut account.ledger);
    assert!(matches!(
        result,
        Err(LcShapeError::Declaration(AdmissionError::ResultMismatch))
    ));
    super::borrowed::defensive_pin_reader(heap, tiles, account)?;
    Ok(())
}

/// Caller reservations compose with the actual indexed helper and sibling owner handoffs.
pub(in crate::loan::lc_shape) fn scopes<P: Scalar, const N: usize>(
    output: &LcOmegaOutput<'_, P, N>,
    registry: &Registry,
    account: &mut ExecutionAccount,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::LoanError;
    let original_limit = account.limits().staged_payload_bytes;
    token_scope(output, registry, None, account)?;
    let candidate = prepare_lc_omega_correspondence(output, registry, None, account)?;
    let certificate = candidate.certificate(&mut account.ledger)?;
    token_scope(output, registry, Some(&certificate), account)?;
    for short in [false, true] {
        // Authentication takes over C internally, then check_originals takes token424
        // while its sibling correspondence and indexed request remain externally reserved.
        let required = candidate.retained_bytes()? + 88;
        let caller = original_limit - (required - u64::from(short));
        let before = account.usage().work;
        let result = account
            .ledger
            .with_retained_payload(caller, crate::loan::AT, |budget| {
                Ok::<_, LoanError>(candidate.authenticate(output, registry, budget))
            })?;
        if short {
            assert!(matches!(
                result,
                Err(LcShapeError::Declaration(AdmissionError::Algebra(_)))
            ));
            assert_eq!(account.usage().work - before, 3);
        } else {
            result?;
            assert_eq!(account.usage().work - before, 43);
        }
        assert_eq!(account.limits().staged_payload_bytes, original_limit);
    }
    Ok(())
}

/// Token admission includes the portable input from its first byte reservation.
fn token_scope<P: Scalar, const N: usize>(
    output: &LcOmegaOutput<'_, P, N>,
    registry: &Registry,
    certificate: Option<&nrz_bindings::readings::algebra::enclosure::lc_root::Certificate>,
    account: &mut ExecutionAccount,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::LoanError;
    let original_limit = account.limits().staged_payload_bytes;
    for short in [false, true] {
        // Captured owner424 + borrowed checked-view80 + actual indexed request88.
        let required = 424 + 80 + 88 + if certificate.is_some() { 10_366 } else { 0 };
        let caller = original_limit - (required - u64::from(short));
        let before = account.usage().work;
        let result = account
            .ledger
            .with_retained_payload(caller, crate::loan::AT, |budget| {
                Ok::<_, LoanError>(super::admit_token(output, registry, certificate, budget))
            })?;
        if short {
            assert!(matches!(
                result,
                Err(LcShapeError::Declaration(AdmissionError::Algebra(_)))
            ));
            assert_eq!(account.usage().work - before, 1);
        } else {
            assert_eq!(result?.retained_bytes()?, 424);
            assert_eq!(account.usage().work - before, 1_017);
        }
        assert_eq!(account.limits().staged_payload_bytes, original_limit);
    }
    Ok(())
}

/// Known partitions use externally supplied design inventories, never this report as oracle.
pub(in crate::loan::lc_shape) fn retained_overlap<P: Scalar, const N: usize>(
    output: &LcOmegaOutput<'_, P, N>,
    registry: &Registry,
    expected: [u64; 2],
    account: &mut ExecutionAccount,
) -> Result<(), Box<dyn std::error::Error>> {
    use super::retained_overlap::{Candidate, EXCLUSIONS, Exclusion, Observation, Scope};
    use nrz_bindings::{Budget, LcOmegaCorrespondence, Limits};
    assert_eq!(
        EXCLUSIONS,
        [
            Exclusion::OriginalBaseline,
            Exclusion::NativeClosureFacadeAndTables,
            Exclusion::CasBackendCapacity,
            Exclusion::PrivateCheckedRecordRegistrations,
            Exclusion::ExternalCanonicalKeyAndProofCopies,
        ]
    );
    let root = prepare_lc_omega_correspondence(output, registry, None, account)?;
    let before = account.usage();
    let report = Candidate::new(&root)?;
    assert_eq!(account.usage(), before);
    assert_eq!(report.captured.scope, Scope::CapturedToken);
    assert_eq!(report.correspondence.scope, Scope::Correspondence);
    assert_eq!(report.captured.logical_allowance, Some(416));
    assert_eq!(report.correspondence.logical_allowance, Some(expected[0]));
    assert_eq!(report.initial_slot, 8);
    assert_eq!(report.logical_allowance, 424 + expected[0]);
    assert_eq!(report.captured.observed_residence, None);
    assert_eq!(report.correspondence.observed_residence, None);
    // Replace the aggregate with these children; adding both would double-count.
    assert_eq!(
        report.logical_allowance,
        report
            .captured
            .logical_allowance
            .expect("captured allowance")
            + report.initial_slot
            + report
                .correspondence
                .logical_allowance
                .expect("store allowance")
    );
    let token = root.token.share(&mut account.ledger)?;
    let before = account.usage().work;
    let shared = report
        .captured
        .same_allocation(&Observation::token(&token)?, &mut account.ledger)?;
    assert!(shared);
    assert_eq!(account.usage().work - before, 1);
    // Same admitted token, independently constructed store: equality of originals is insufficient.
    let second = LcOmegaCorrespondence::construct(&token, &mut account.ledger)?;
    second.check_declaration(&token, &mut account.ledger)?;
    let distinct = !report
        .correspondence
        .same_allocation(&Observation::correspondence(&second), &mut account.ledger)?;
    assert!(distinct);
    assert_eq!(second.retained_bytes(), expected[0]);
    // Independently constructed stores contribute two disjoint correspondence allowances.
    assert_eq!(
        root.correspondence.retained_bytes() + second.retained_bytes(),
        2 * expected[0]
    );
    let certificate = root.certificate(&mut account.ledger)?;
    let duplicate = root.certificate(&mut account.ledger)?;
    assert_eq!(certificate, duplicate);
    let source = Observation::certificate(&certificate);
    let copy = Observation::certificate(&duplicate);
    assert_eq!(source.scope, Scope::Certificate);
    assert_eq!(source.logical_allowance, None);
    assert_eq!(source.observed_residence, Some(24 + 10_342));
    let separate_buffer = !source.same_allocation(&copy, &mut account.ledger)?;
    assert!(separate_buffer);
    let borrowed =
        source.same_allocation(&Observation::certificate(&certificate), &mut account.ledger)?;
    assert!(borrowed);
    let replay = prepare_lc_omega_correspondence(output, registry, Some(&certificate), account)?;
    let replay_report = Candidate::new(&replay)?;
    assert_eq!(replay_report.logical_allowance, 424 + expected[1]);
    let distinct_store = !report
        .correspondence
        .same_allocation(&replay_report.correspondence, &mut account.ledger)?;
    let distinct_token = !report
        .captured
        .same_allocation(&replay_report.captured, &mut account.ledger)?;
    assert!(distinct_store);
    assert!(distinct_token);
    // Simultaneous construction/replay union retains both original captures, both stores,
    // and the replay input once. This is a mixed known partition, not backend capacity.
    assert_eq!(
        root.retained_bytes()? + replay.retained_bytes()? + certificate.retained_bytes(),
        2 * 424 + expected[0] + expected[1] + 10_366
    );
    let exported = replay.certificate(&mut account.ledger)?;
    assert_eq!(exported, certificate);
    let distinct_export = !exported.shares_allocation(&certificate);
    assert!(distinct_export);
    // Charged token comparisons propagate refusal; exhaustion must not masquerade as inequality.
    let mut exhausted = Budget::new(Limits {
        work: 0,
        ..Limits::default()
    })?;
    let refusal = report
        .captured
        .same_allocation(&report.captured, &mut exhausted);
    let exhausted_refused = refusal.is_err();
    assert!(exhausted_refused);
    retained_handoffs([&root, &replay], account)
}

/// Exact helper reservations reject a duplicate correspondence debit at the tight ceiling.
fn retained_handoffs(
    values: [&LcOmegaCandidate; 2],
    account: &mut ExecutionAccount,
) -> Result<(), Box<dyn std::error::Error>> {
    for value in values {
        let ceiling = account.limits().staged_payload_bytes;
        let caller = ceiling - value.retained_bytes()?;
        let before = account.usage().work;
        account
            .ledger
            .with_retained_payload(caller, crate::loan::AT, |budget| {
                value.correspondence.check_declaration(&value.token, budget)
            })?;
        assert_eq!(account.usage().work - before, 1);
        assert_eq!(account.limits().staged_payload_bytes, ceiling);
    }
    Ok(())
}

/// Follow the actual consuming join, retaining only an explicitly paid caller token witness.
pub(in crate::loan::lc_shape) fn retained_transfer<'i, P: Scalar + 'static, const N: usize>(
    output: &LcOmegaOutput<'i, P, N>,
    heap: &mut Heap,
    tiles: &[&LcShapeTile<'i, P>],
    registry: &Registry,
    case: (bool, u64, Option<usize>),
    account: &mut ExecutionAccount,
) -> Result<(), Box<dyn std::error::Error>> {
    use super::{
        borrowed::prepare_borrowed,
        capture,
        retained_overlap::{Candidate, Observation},
    };
    let (replay, expected, fail_after) = case;
    let certificate = if replay {
        let source = prepare_lc_omega_correspondence(output, registry, None, account)?;
        Some(source.certificate(&mut account.ledger)?)
    } else {
        None
    };
    if let Some(fail_after) = fail_after {
        let (candidate, plan) =
            prepare_borrowed(output, heap, tiles, registry, certificate.as_ref(), account)?;
        let before = account.usage().work;
        let debits = heap.loans.debits.iter().flatten().count();
        let injection = crate::loan::fault::Injection::after(fail_after);
        let external = certificate.as_ref().map_or(
            0,
            nrz_bindings::readings::algebra::enclosure::lc_root::Certificate::retained_bytes,
        );
        let refused = capture::join(heap, candidate, plan, external, account);
        drop(injection);
        let allocation_refused = matches!(
            refused,
            Err(LcShapeError::Loan(crate::LoanError::AllocationRefused))
        );
        assert!(allocation_refused);
        assert_eq!(heap.loans.debits.iter().flatten().count(), debits);
        let prefix_consumed = account.usage().work > before;
        assert!(prefix_consumed);
    }
    let (candidate, plan) =
        prepare_borrowed(output, heap, tiles, registry, certificate.as_ref(), account)?;
    let token = candidate.token.share(&mut account.ledger)?;
    let report = Candidate::new(&candidate)?;
    assert_eq!(report.logical_allowance, 424 + expected);
    let same_token = report
        .captured
        .same_allocation(&Observation::token(&token)?, &mut account.ledger)?;
    assert!(same_token);
    let external = 8 + certificate.as_ref().map_or(
        0,
        nrz_bindings::readings::algebra::enclosure::lc_root::Certificate::retained_bytes,
    );
    let (correspondence, receipt) = capture::join(heap, candidate, plan, external, account)?;
    // The initial slot is gone; the extra caller token keeps its own eight-byte slot.
    correspondence.check_declaration(&token, &mut account.ledger)?;
    assert_eq!(Observation::token(&token)?.logical_allowance, Some(416));
    assert_eq!(
        Observation::correspondence(&correspondence).logical_allowance,
        Some(expected)
    );
    let weak = std::rc::Rc::downgrade(&correspondence);
    // A test witness models two native users of the existing outer Rc, not another store.
    let shared = std::rc::Rc::clone(&correspondence);
    let before = account.usage();
    let shared_store = Observation::correspondence(&correspondence)
        .same_allocation(&Observation::correspondence(&shared), &mut account.ledger)?;
    assert!(shared_store);
    assert_eq!(account.usage(), before);
    drop(correspondence);
    assert_eq!(weak.strong_count(), 1);
    shared.check_declaration(&token, &mut account.ledger)?;
    drop(token);
    // No external slot now: captured416 remains owned through the correspondence.
    assert_eq!(shared.retained_bytes(), expected);
    drop(shared);
    assert_eq!(weak.strong_count(), 0);
    drop(receipt);
    Ok(())
}

/// One replay input survives an injected join prefix and the successful retry on one ledger.
pub(in crate::loan::lc_shape) fn retained_timeline_prepare<
    'i,
    P: Scalar + 'static,
    const N: usize,
>(
    output: LcOmegaOutput<'i, P, N>,
    heap: &mut Heap,
    tiles: &[&LcShapeTile<'i, P>],
    registry: &Registry,
    replay: (
        &nrz_bindings::readings::algebra::enclosure::lc_root::Certificate,
        u64,
    ),
    account: &mut ExecutionAccount,
) -> Result<crate::LcOmegaPrepared<'i, P, N>, Box<dyn std::error::Error>> {
    use super::{
        borrowed::prepare_borrowed,
        capture,
        retained_overlap::{Candidate, Observation},
    };
    let (certificate, expected) = replay;
    let (failed, plan) =
        prepare_borrowed(&output, heap, tiles, registry, Some(certificate), account)?;
    let report = Candidate::new(&failed)?;
    assert_eq!(report.captured.logical_allowance, Some(416));
    assert_eq!(report.correspondence.logical_allowance, Some(expected));
    assert_eq!(report.initial_slot, 8);
    assert_eq!(report.logical_allowance, 424 + expected);
    let failed_token = failed.token.share(&mut account.ledger)?;
    let same = report
        .captured
        .same_allocation(&Observation::token(&failed_token)?, &mut account.ledger)?;
    assert!(same);
    let before = account.usage().work;
    let debits = heap.loans.debits.iter().flatten().count();
    let injection = crate::loan::fault::Injection::after(1);
    let refused = capture::join(
        heap,
        failed,
        plan,
        certificate.retained_bytes() + 8,
        account,
    );
    drop(injection);
    assert!(matches!(
        refused,
        Err(LcShapeError::Loan(crate::LoanError::AllocationRefused))
    ));
    assert_eq!(heap.loans.debits.iter().flatten().count(), debits);
    assert!(account.usage().work > before);
    assert_eq!(
        Observation::token(&failed_token)?.logical_allowance,
        Some(416)
    );
    assert_eq!(
        Observation::certificate(certificate).observed_residence,
        Some(10_366)
    );
    let (candidate, plan) =
        prepare_borrowed(&output, heap, tiles, registry, Some(certificate), account)?;
    let report = Candidate::new(&candidate)?;
    assert_eq!(report.logical_allowance, 424 + expected);
    assert_eq!(report.correspondence.logical_allowance, Some(expected));
    let distinct = !report
        .captured
        .same_allocation(&Observation::token(&failed_token)?, &mut account.ledger)?;
    assert!(distinct);
    drop(failed_token);
    let token = candidate.token.share(&mut account.ledger)?;
    let same = report
        .captured
        .same_allocation(&Observation::token(&token)?, &mut account.ledger)?;
    assert!(same);
    let (correspondence, receipt) = capture::join(
        heap,
        candidate,
        plan,
        certificate.retained_bytes() + 8,
        account,
    )?;
    correspondence.check_declaration(&token, &mut account.ledger)?;
    assert_eq!(Observation::token(&token)?.logical_allowance, Some(416));
    assert_eq!(
        Observation::correspondence(&correspondence).logical_allowance,
        Some(expected)
    );
    drop(token);
    Ok(crate::LcOmegaPrepared {
        declaration: output,
        correspondence,
        receipt,
    })
}
