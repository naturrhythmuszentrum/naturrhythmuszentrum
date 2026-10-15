// SPDX-License-Identifier: AGPL-3.0-only
//! Public Gaussian construction helpers, compiled in both test binaries.

use crate::test_api::limits::{Budget, Limits};
use crate::test_api::solving::{
    CoefficientDomainData, CoefficientValueData, CoefficientValueObject, SolveDraft,
};
use crate::test_api::syntax::Draft;
use crate::test_api::{Cas, Handle, Rational, Reference};

use super::common::TestResult;

/// Parse two independent exact components without Gaussian arithmetic.
pub(crate) fn parts(real: &str, imaginary: &str) -> TestResult<(Rational, Rational)> {
    let mut budget = Budget::new(Limits::default())?;
    Ok((
        Rational::parse(real, &mut budget)?,
        Rational::parse(imaginary, &mut budget)?,
    ))
}

/// A graph-owned Gaussian domain and value, admitted atomically in one batch.
pub(crate) fn stored(real: &Rational, imaginary: &Rational) -> TestResult<(Cas, Handle, Handle)> {
    let mut cas = Cas::new(Limits::default())?;
    let handles = cas.admit(
        &[
            Draft::Solve(SolveDraft::CoefficientDomain(
                CoefficientDomainData::Gaussian,
            )),
            Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
                domain: Reference::Local(0),
                value: CoefficientValueData::Gaussian {
                    real: real.clone(),
                    imaginary: imaginary.clone(),
                },
            })),
        ],
        &mut Budget::new(Limits::default())?,
    )?;
    let domain = handles.first().ok_or("Gaussian domain missing")?.clone();
    let object = handles.get(1).ok_or("Gaussian value missing")?.clone();
    Ok((cas, domain, object))
}
