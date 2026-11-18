// SPDX-License-Identifier: AGPL-3.0-only
//! Private sealing controls exercise failures unavailable through the public constructor.
use super::*;

/// Foreign owner authentication precedes the complete same-store expected-request comparison.
pub(in crate::lc_omega) fn identity(
    declaration: &LcOmegaDeclarationToken,
    budget: &mut Budget,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let (cas, expected, _) = prepare(declaration, None, budget)?;
    let (_, _, foreign) = prepare(declaration, None, budget)?;
    let result = seal(cas, foreign, declaration, &expected, budget);
    assert!(matches!(
        result,
        Err(AdmissionError::Algebra(nrz_cas::Fault {
            kind: nrz_cas::FaultKind::Invalid(nrz_cas::InvalidCode::ForeignHandle),
            ..
        }))
    ));
    let (cas, mut expected, receipt) = prepare(declaration, None, budget)?;
    expected.constraints.swap(0, 3);
    assert!(matches!(
        seal(cas, receipt, declaration, &expected, budget),
        Err(AdmissionError::ValueMismatch)
    ));
    Ok(())
}

/// Inspect the actual receiving store policy without exporting any production store capability.
pub(in crate::lc_omega) fn policy(
    declaration: &LcOmegaDeclarationToken,
    expected: u64,
    budget: &mut Budget,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let (cas, _) = translate(declaration, budget.limits().clone(), budget)?;
    assert_eq!(cas.limits().name_bytes, expected);
    Ok(())
}

/// Confirm publication before deliberately exhausting the typed seal's final prepaid edge.
pub(in crate::lc_omega) fn published_failure(
    declaration: &LcOmegaDeclarationToken,
    budget: &mut Budget,
) -> Result<LcOmegaCorrespondence> {
    let (cas, request, receipt) =
        budget.with_retained_payload(declaration.retained_bytes()?, AT, |budget| {
            prepare(declaration, None, budget)
        })?;
    assert!(receipt.request(&cas).is_ok());
    assert_eq!(cas.counts().payload_bytes, 20_514 + 10_119 + 1170);
    seal(cas, receipt, declaration, &request, budget)
}

/// Compare separately derived phase tariffs; observations are assertions, never oracle inputs.
pub(in crate::lc_omega) fn source_tariffs(
    declaration: &LcOmegaDeclarationToken,
    budget: &mut Budget,
) -> Result<()> {
    let original = declaration.originals();
    let before = budget.usage().work;
    let policy = policy::receiving(original, budget.limits().clone(), budget)?;
    assert_eq!(
        budget.usage().work - before,
        2 + 30 + 33 + 187 + 2,
        "original policy inventory"
    );
    let mut cas = Cas::new(policy)?;
    let before = budget.usage().work;
    let translated = translation::build(original.question.relation().inputs(), budget)?;
    assert_eq!(
        budget.usage().work - before,
        398 + 1383 + 4160,
        "rational pool and draft assembly"
    );
    let before = budget.usage().work;
    let expected = translated.admit(&mut cas, original.positions, budget)?;
    assert_eq!(
        budget.usage().work - before,
        57_567 + 128,
        "batch admission and expected request"
    );
    let before = budget.usage().work;
    let receipt = cas.construct_lc_root(&expected, budget)?;
    assert_eq!(
        budget.usage().work - before,
        259_400,
        "registered construction"
    );
    let before = budget.usage().work;
    let _sealed = seal(cas, receipt, declaration, &expected, budget)?;
    assert_eq!(budget.usage().work - before, 32, "typed seal and freeze");
    Ok(())
}
