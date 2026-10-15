// SPDX-License-Identifier: AGPL-3.0-only
//! Untrusted germ witnesses cannot change approaches, coefficients or remainder relations.

use crate::domain_model::Side;
use crate::germ_model::{GermAt, SeriesAt};
use crate::number::Rational;
use crate::proof_model::{ApplicationDraft, Certificate, CheckedApplication, Statement};
use crate::request_model::Request;
use crate::search::SearchPolicy;
use crate::task::Task;
use crate::testing::common::TestResult;
use crate::testing::native::{Fixture, budget};

/// Altering a side or coefficient fails even with the original checked branch premises.
/// ´test:crate:germ-restriction-witness-tampering´
#[test]
fn germ_restriction_witness_tampering() -> TestResult {
    for series in [false, true] {
        let (mut fixture, original, premises) = evidence(series)?;
        for change in [false, true] {
            let mut draft = original.clone();
            let Statement::Germ(value) = &mut draft.conclusion else {
                return Err("germ claim".into());
            };
            if change {
                value.at.side = Some(Side::Below);
            } else {
                *value.coefficients.first_mut().ok_or("coefficient")? = Rational::one();
            }
            assert!(
                crate::scoped_check::check_application(
                    &mut fixture.cas,
                    &draft,
                    &premises,
                    &mut budget()?
                )
                .is_err()
            );
        }
    }
    Ok(())
}

/// A false remainder or changed formal/analytic mode cannot reuse a genuine side jet.
/// ´test:crate:germ-series-remainder-tampering´
#[test]
fn germ_series_remainder_tampering() -> TestResult {
    let (mut fixture, original, premises) = evidence(true)?;
    let mut draft = original.clone();
    let Certificate::Germ(value) = &mut draft.certificate else {
        return Err("remainder certificate".into());
    };
    value.remainders = vec![vec![Rational::one()]];
    assert!(
        crate::scoped_check::check_application(&mut fixture.cas, &draft, &premises, &mut budget()?)
            .is_err()
    );
    draft = original;
    let Statement::Germ(value) = &mut draft.conclusion else {
        return Err("germ claim".into());
    };
    value.mode = crate::germ_model::GermMode::Series {
        order: 2,
        analytic: true,
    };
    assert!(
        crate::scoped_check::check_application(&mut fixture.cas, &draft, &premises, &mut budget()?)
            .is_err()
    );
    Ok(())
}

/// Generate real premises, then expose their exact checked root solely for adversarial mutations.
fn evidence(series: bool) -> TestResult<(Fixture, ApplicationDraft, Vec<CheckedApplication>)> {
    let mut fixture = Fixture::new()?;
    let point = crate::boundary_regions::boundary(
        &mut fixture.cas,
        &fixture.domain,
        &Rational::zero(),
        &mut budget()?,
    )?;
    let at = GermAt {
        coordinate: "x".into(),
        point,
        side: Some(Side::Above),
    };
    let task = if series {
        Task::Series(Box::new(SeriesAt {
            at,
            order: 2,
            analytic: false,
        }))
    } else {
        Task::Limit(Box::new(at))
    };
    let request = Request {
        task,
        context: fixture.context.clone(),
        inputs: Vec::new(),
        premises: Vec::new(),
        limits: crate::budget::LimitsOverride::default(),
    };
    let result = crate::germ_producer::execute(
        &mut fixture.cas,
        &request,
        &[fixture.x.logical_id()],
        &SearchPolicy::default(),
        &mut budget()?,
        &mut budget()?,
    )?;
    let application = crate::proof_view::application(
        &fixture.cas,
        result.witness.proof().logical_id(),
        &mut budget()?,
    )?;
    let mut premises = Vec::new();
    for id in &application.premises {
        premises.push(crate::proof_view::application(
            &fixture.cas,
            *id,
            &mut budget()?,
        )?);
    }
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
    Ok((fixture, draft, premises))
}
