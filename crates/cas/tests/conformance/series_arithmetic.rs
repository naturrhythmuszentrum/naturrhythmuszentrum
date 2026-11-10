// SPDX-License-Identifier: AGPL-3.0-only
//! Independent finite arithmetic and precise capability boundaries.

use crate::testing::common::{TestResult, default_budget};
use crate::testing::series_proof::{fixture, rational};
use nrz_cas::proof::{Certificate, Statement};
use nrz_cas::series::SeriesCertificate as C;
use nrz_cas::{FaultKind, InvalidCode};

/// Every coefficient is chosen by hand, including all six convolution pairs and zero terms.
/// ´test:integration:series-arithmetic-hand-fixtures´
#[test]
fn series_arithmetic_hand_fixtures() -> TestResult {
    for kind in 0..12 {
        let mut f = fixture(kind, false)?;
        assert_eq!(
            f.base
                .cas
                .series_object(match &f.expected.statement {
                    Statement::Series(s) => match s.as_ref() {
                        nrz_cas::series::SeriesStatement::Source(_)
                        | nrz_cas::series::SeriesStatement::EvaluatedResidual { .. } =>
                            return Err("object fixture".into()),
                        nrz_cas::series::SeriesStatement::Reciprocal { input, .. }
                        | nrz_cas::series::SeriesStatement::Truncate { input, .. }
                        | nrz_cas::series::SeriesStatement::Scale { input, .. }
                        | nrz_cas::series::SeriesStatement::Derive { input, .. }
                        | nrz_cas::series::SeriesStatement::Integrate { input, .. }
                        | nrz_cas::series::SeriesStatement::Coefficient { input, .. } => input,
                        nrz_cas::series::SeriesStatement::Add { left, .. }
                        | nrz_cas::series::SeriesStatement::Compose { left, .. }
                        | nrz_cas::series::SeriesStatement::Mul { left, .. } => left,
                    },
                    _ => return Err("series".into()),
                })?
                .field,
            f.field
        );
        let witness = f
            .base
            .cas
            .check_proof(&f.base.context, &f.steps, &mut default_budget()?)?;
        assert_eq!(witness.judgment(), &f.expected);
        assert!(witness.is_discharged());
        assert_eq!(witness.hypotheses(), []);
        assert_eq!(
            f.base.cas.retained_judgment_count(),
            if kind == 4 { 2 } else { 1 }
        );
        assert!(f.base.cas.retained_judgment_bytes() > 0);
        let citations = f
            .base
            .cas
            .proof_citations(witness.proof(), &mut default_budget()?)?;
        assert_eq!(citations.len(), f.steps.len());
    }
    Ok(())
}

/// A false result, missing convolution pair or altered index cannot supply finite authority.
/// ´test:integration:series-arithmetic-row-mutations´
#[test]
fn series_arithmetic_row_mutations() -> TestResult {
    for kind in 0..12 {
        let mut f = fixture(kind, false)?;
        let root = f.steps.last_mut().ok_or("root")?;
        let Certificate::Series(value) = &mut root.certificate else {
            return Err("series certificate".into());
        };
        match value.as_mut() {
            C::Source(_) | C::EvaluatedResidual { .. } => return Err("object fixture".into()),
            C::Truncate { coefficients, .. } => coefficients[0] = rational("9")?,
            C::Add { rows, .. } | C::Scale { rows, .. } => rows[0].result = rational("9")?,
            C::Compose { sums, .. } => sums[0].product = rational("9")?,
            C::Reciprocal { rows, .. } | C::Mul { rows, .. } => {
                rows.pop();
            }
            C::Derive { rows, .. } => rows[0].result = rational("9")?,
            C::Integrate { rows, .. } => rows[0].result = rational("9")?,
            C::Coefficient { value, .. } => *value = rational("9")?,
        }
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        assert!(matches!(
            f.base
                .cas
                .check_proof(&f.base.context, &f.steps, &mut default_budget()?)
                .map_err(|e| e.kind),
            Err(FaultKind::Invalid(_))
        ));
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
    }
    Ok(())
}

/// Complete admitted source forms check; object bytes alone and unadmitted forms refuse.
/// ´test:integration:series-arithmetic-deferred-forms´
#[test]
fn series_arithmetic_deferred_forms() -> TestResult {
    for kind in 0..8 {
        let mut f = crate::testing::series_source::lifted(kind, false)?;
        let expected = f.expected.as_ref().ok_or("expected")?;
        let value = crate::testing::series_source::statement(expected)?;
        f.base
            .cas
            .check_series_source(value, &f.certificate, &f.proofs, &mut default_budget()?)?;
    }
    let mut seed = crate::testing::series_source::setup(3, 4, false, true)?;
    let expected = seed.expected.as_ref().ok_or("expected")?;
    seed.base.cas.check_series_source(
        crate::testing::series_source::statement(expected)?,
        &seed.certificate,
        &seed.proofs,
        &mut default_budget()?,
    )?;
    for kind in 0..16 {
        for form in [1, 2] {
            let mut f = fixture(kind, false)?;
            let root = f.steps.last_mut().ok_or("root")?;
            let Certificate::Series(value) = &mut root.certificate else {
                return Err("series certificate".into());
            };
            match value.as_mut() {
                C::Source(_) | C::EvaluatedResidual { .. } => return Err("object fixture".into()),
                C::Reciprocal { form: slot, .. }
                | C::Truncate { form: slot, .. }
                | C::Add { form: slot, .. }
                | C::Scale { form: slot, .. }
                | C::Derive { form: slot, .. }
                | C::Integrate { form: slot, .. }
                | C::Compose { form: slot, .. }
                | C::Mul { form: slot, .. }
                | C::Coefficient { form: slot, .. } => *slot = form,
            }
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            let fault = f
                .base
                .cas
                .check_proof(&f.base.context, &f.steps, &mut default_budget()?)
                .expect_err("deferred form");
            assert_eq!(
                fault.kind,
                FaultKind::Invalid(InvalidCode::SeriesUnsupported)
            );
            assert_eq!(
                f.base.cas.canonical_snapshot(&mut default_budget()?)?,
                before
            );
        }
    }
    assert!(
        nrz_cas::proof::PRIMITIVES
            .iter()
            .any(|rule| rule.specification().key == "series.compose"
                && rule.specification().version == 1)
    );
    assert!(
        nrz_cas::proof::PRIMITIVES
            .iter()
            .any(|rule| rule.specification().key == "series.residual"
                && rule.specification().available)
    );
    assert!(
        nrz_cas::proof::Primitive::SeriesReciprocal
            .specification()
            .available
    );
    for key in ["series.divide", "series.invert"] {
        assert!(
            !nrz_cas::proof::PRIMITIVES
                .iter()
                .any(|rule| rule.specification().key == key)
        );
    }
    let f = fixture(0, false)?;
    assert!(matches!(f.expected.statement, Statement::Series(_)));
    Ok(())
}
