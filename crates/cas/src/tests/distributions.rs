// SPDX-License-Identifier: AGPL-3.0-only
//! Adversarial distribution evidence and native typed cost projections.

use crate::distribution_model::{DistributionCertificate, DistributionStatement};
use crate::domain_model::Layer;
use crate::model::Operator;
use crate::proof_model::{Certificate, Statement};
use crate::search::SearchPolicy;
use crate::task::Task;
use crate::testing::common::TestResult;
use crate::testing::distribution_native::{Fixture, evidence};
use crate::testing::native::budget;

/// Both explicitly selected producers prove the hand-specified second derivative of absolute value.
/// ´test:crate:distribution-both-derivative-routes´
#[test]
fn distribution_both_derivative_routes() -> TestResult {
    for named in [false, true] {
        let mut fixture = Fixture::new()?;
        let input = fixture.absolute()?;
        let task = Task::Differentiate {
            coordinate: "x".into(),
            domain: fixture.base.domain.clone(),
            layer: Layer::Distribution,
            order: 2,
        };
        let result = fixture.route(task, &[input], named)?;
        let delta = fixture.node(Operator::Delta { order: 0 }, &[fixture.base.zero.clone()])?;
        let two = crate::testing::native::term(
            &mut fixture.base.cas,
            &fixture.base.header,
            Operator::Rational(crate::number::Rational::from_ratio(2, 1, &mut budget()?)?),
            &[],
        )?;
        let expected = fixture.node(Operator::DistributionScale, &[two, delta])?;
        let Statement::Distribution(value) = &result.witness.judgment().statement else {
            return Err("distribution derivative".into());
        };
        assert!(
            matches!(value.as_ref(), DistributionStatement::Derivative { derivative, order: 2, .. }
            if *derivative == expected)
        );
    }
    Ok(())
}

/// An omitted region or changed coefficient cannot retain a finite regularity certificate.
/// ´test:crate:distribution-regularity-certificate-tampering´
#[test]
fn distribution_regularity_certificate_tampering() -> TestResult {
    let mut fixture = Fixture::new()?;
    let result = fixture.route(
        Task::EmbedRegular {
            space: fixture.space.clone(),
        },
        &[fixture.base.x.clone()],
        false,
    )?;
    let (_, premises) = evidence(&fixture, result.witness.proof())?;
    let regular = premises.first().ok_or("regularity premise")?;
    let mut draft = crate::proof_model::ApplicationDraft {
        context: regular.judgment.context.clone(),
        rule: regular.rule,
        conclusion: regular.judgment.statement.clone(),
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: regular.certificate.clone(),
    };
    crate::scoped_check::check_application(&mut fixture.base.cas, &draft, &[], &mut budget()?)?;
    let original = draft.certificate.clone();
    for mode in [false, true] {
        draft.certificate.clone_from(&original);
        let Certificate::Distribution(value) = &mut draft.certificate else {
            return Err("certificate".into());
        };
        let DistributionCertificate::Regular(value) = value.as_mut() else {
            return Err("regular".into());
        };
        if mode {
            value.regions.clear();
        } else {
            value.polynomials = vec![Some(vec![crate::number::Rational::one()])];
        }
        assert!(
            crate::scoped_check::check_application(
                &mut fixture.base.cas,
                &draft,
                &[],
                &mut budget()?
            )
            .is_err()
        );
    }
    Ok(())
}

/// The signed delta action independently rejects the opposite result and a changed premise layer.
/// ´test:crate:distribution-delta-action-tampering´
#[test]
fn distribution_delta_action_tampering() -> TestResult {
    let mut fixture = Fixture::new()?;
    let delta = fixture.node(Operator::Delta { order: 1 }, &[fixture.base.zero.clone()])?;
    let result = fixture.route(
        Task::Pair {
            space: fixture.space.clone(),
            action_mode: crate::task::ActionMode::NearSupport {
                neighborhood: fixture.base.domain.clone(),
            },
        },
        &[delta, fixture.base.x.clone()],
        false,
    )?;
    let (mut draft, mut premises) = evidence(&fixture, result.witness.proof())?;
    crate::scoped_check::check_application(
        &mut fixture.base.cas,
        &draft,
        &premises,
        &mut budget()?,
    )?;
    let original = draft.conclusion.clone();
    let Statement::Distribution(value) = &mut draft.conclusion else {
        return Err("pairing".into());
    };
    let DistributionStatement::Pairing { value, .. } = value.as_mut() else {
        return Err("action".into());
    };
    value.clone_from(&fixture.base.one);
    assert!(
        crate::scoped_check::check_application(
            &mut fixture.base.cas,
            &draft,
            &premises,
            &mut budget()?
        )
        .is_err()
    );
    draft.conclusion = original;
    premises.first_mut().ok_or("support")?.judgment.layer = Layer::Pointwise;
    assert!(
        crate::scoped_check::check_application(
            &mut fixture.base.cas,
            &draft,
            &premises,
            &mut budget()?
        )
        .is_err()
    );
    Ok(())
}

/// Native shortcuts keep exact spaces, strict older costs and replayable primitive expansions.
/// ´test:crate:distribution-native-shortcut-snapshot´
#[test]
fn distribution_native_shortcut_snapshot() -> TestResult {
    let mut fixture = Fixture::new()?;
    let input = fixture.absolute()?;
    let formal = fixture.node(Operator::DistributionDerivative { order: 2 }, &[input])?;
    let result = fixture.route(Task::Simplify, std::slice::from_ref(&formal), false)?;
    let expected = result.witness.judgment().clone();
    let policy = SearchPolicy::default();
    let before = fixture
        .base
        .cas
        .certify_frozen_cost(&expected, &policy.costs, &mut budget()?)?;
    assert_eq!(before.shortest_cost(), Some(2));
    fixture
        .base
        .cas
        .register_derived(&result.witness, &expected, 1, &mut budget()?)?;
    let after = fixture
        .base
        .cas
        .certify_frozen_cost(&expected, &policy.costs, &mut budget()?)?;
    assert_eq!(after.shortest_cost(), Some(1));
    assert_ne!(before.manifest(), after.manifest());
    assert!(
        fixture
            .base
            .cas
            .register_derived(&result.witness, &expected, 1, &mut budget()?)
            .is_err()
    );
    let request = fixture.request(Task::Simplify);
    let execution = crate::distribution_schedule::execute(
        &mut fixture.base.cas,
        &request,
        &[formal.logical_id()],
        &policy,
        &mut budget()?,
        &mut budget()?,
    )?;
    let reused = execution.result.ok_or("scheduled reuse")?;
    assert!(execution.fault.is_none());
    assert!(reused.cost < result.cost);
    assert!(reused.primitive_steps > reused.statistics.transitions);
    assert!(crate::proof_view::scalar_claim(&fixture.base.cas, reused.witness.judgment()).is_err());
    crate::case_producer::replay(&fixture.base.cas, &reused.witness, &mut budget()?)?;
    Ok(())
}

/// Reversing a reconstructed value jump cannot turn a valid first derivative into another theorem.
/// ´test:crate:distribution-jump-sign-tampering´
#[test]
fn distribution_jump_sign_tampering() -> TestResult {
    let mut fixture = Fixture::new()?;
    let function = fixture.base.cas.named_case(
        &fixture.base.domain,
        &crate::boundary_model::NamedCase {
            name: crate::boundary_model::CaseName::Step,
            coordinate: "x".into(),
            parameter: Some(crate::number::Rational::zero()),
        },
        &mut budget()?,
    )?;
    let embedded = fixture.node(Operator::RegularEmbed, &[function])?;
    let result = fixture.route(
        Task::Differentiate {
            coordinate: "x".into(),
            domain: fixture.base.domain.clone(),
            layer: Layer::Distribution,
            order: 1,
        },
        &[embedded],
        false,
    )?;
    let (mut draft, premises) = evidence(&fixture, result.witness.proof())?;
    let delta = fixture.node(Operator::Delta { order: 0 }, &[fixture.base.zero.clone()])?;
    let negative = crate::testing::native::term(
        &mut fixture.base.cas,
        &fixture.base.header,
        Operator::Rational(crate::number::Rational::from_ratio(-1, 1, &mut budget()?)?),
        &[],
    )?;
    let wrong = fixture.node(Operator::DistributionScale, &[negative, delta])?;
    let Statement::Distribution(value) = &mut draft.conclusion else {
        return Err("distribution".into());
    };
    let DistributionStatement::Derivative { derivative, .. } = value.as_mut() else {
        return Err("derivative".into());
    };
    *derivative = wrong;
    assert!(
        crate::scoped_check::check_application(
            &mut fixture.base.cas,
            &draft,
            &premises,
            &mut budget()?
        )
        .is_err()
    );
    Ok(())
}
