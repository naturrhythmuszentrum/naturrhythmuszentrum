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
