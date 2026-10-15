// SPDX-License-Identifier: AGPL-3.0-only
//! Public Gaussian syntax, canonical components and numeric capability boundaries.

use crate::testing::common::TestResult;
use crate::testing::solve_numeric::{parts, stored};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::solving::{
    Availability, CoefficientValueData, CoefficientValueObject, SolveDraft, SolveView,
};
use nrz_cas::syntax::Draft;
use nrz_cas::{FaultKind, Reference};

/// Exact arithmetic availability is independent of the separately tested theorem bitmap.
/// ´test:integration:numeric-capability-is-available´
#[test]
fn numeric_capability_is_available() {
    assert_eq!(
        nrz_cas::solving::capabilities().first(),
        Some(&Availability::Available)
    );
}

/// Gaussian drafts retain reduced rational components and canonical positive zero.
/// ´test:integration:gaussian-draft-components-are-canonical´
#[test]
fn gaussian_draft_components_are_canonical() -> TestResult {
    let (real, imaginary) = parts("-0/7", "6/9")?;
    let (mut cas, domain, object) = stored(&real, &imaginary)?;
    let SolveView::CoefficientValue(data) = cas.solve_view(&object)? else {
        return Err("Gaussian payload missing".into());
    };
    let CoefficientValueData::Gaussian { real, imaginary } = &data.value else {
        return Err("Gaussian components missing".into());
    };
    let mut budget = Budget::new(Limits::default())?;
    assert_eq!(real.small_ratio(&mut budget)?, Some((0, 1)));
    assert!(!real.is_negative());
    assert_eq!(imaginary.small_ratio(&mut budget)?, Some((2, 3)));
    let (real, imaginary) = parts("0", "2/3")?;
    let duplicate = Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
        domain: Reference::Existing(domain),
        value: CoefficientValueData::Gaussian { real, imaginary },
    }));
    let before = cas.counts();
    assert_eq!(
        cas.admit(std::slice::from_ref(&duplicate), &mut budget)?,
        [object]
    );
    assert_eq!(cas.counts(), before);
    Ok(())
}

/// Both Gaussian components obey the caller's bit ceiling before transactional publication.
/// ´test:integration:gaussian-draft-component-boundary´
#[test]
fn gaussian_draft_component_boundary() -> TestResult {
    let (zero, one) = parts("0", "1")?;
    let (mut cas, domain, _) = stored(&zero, &one)?;
    let (real, imaginary) = parts("3/7", "5/7")?;
    let draft = Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
        domain: Reference::Existing(domain),
        value: CoefficientValueData::Gaussian { real, imaginary },
    }));
    let before = cas.counts();
    let mut short = Budget::new(Limits {
        component_bits: 2,
        ..Limits::default()
    })?;
    let fault = cas
        .admit(std::slice::from_ref(&draft), &mut short)
        .err()
        .ok_or("oversized Gaussian admitted")?;
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::ComponentBits,
            limit: 2,
            needed: 3,
            ..
        }
    ));
    assert_eq!(cas.counts(), before);
    let mut exact = Budget::new(Limits {
        component_bits: 3,
        ..Limits::default()
    })?;
    assert_eq!(
        cas.admit(std::slice::from_ref(&draft), &mut exact)?.len(),
        1
    );
    Ok(())
}
