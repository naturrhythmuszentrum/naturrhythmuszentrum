// SPDX-License-Identifier: AGPL-3.0-only
//! Independent kernel rejection of mutated function, orientation and ownership evidence.

use crate::proof_model::{Ownership, Statement};
use crate::testing::boundary_native::obstruction;
use crate::testing::common::TestResult;
use crate::testing::native::budget;

/// Swapping an incident side or the exact function identity invalidates the primitive.
/// ´test:crate:boundary-function-and-side-tampering´
#[test]
fn boundary_function_and_side_tampering() -> TestResult {
    let mut evidence = obstruction()?;
    crate::scoped_check::check_application(
        &mut evidence.fixture.cas,
        &evidence.draft,
        &evidence.premises,
        &mut budget()?,
    )?;
    let mut wrong_side = evidence.premises.clone();
    let Statement::Jet { side, .. } =
        &mut wrong_side.get_mut(1).ok_or("left jet")?.judgment.statement
    else {
        return Err("jet".into());
    };
    *side = crate::domain_model::Side::Above;
    assert!(
        crate::scoped_check::check_application(
            &mut evidence.fixture.cas,
            &evidence.draft,
            &wrong_side,
            &mut budget()?
        )
        .is_err()
    );
    let mut wrong_function = evidence.draft.clone();
    let Statement::DerivativeDoesNotExist(value) = &mut wrong_function.conclusion else {
        return Err("nonexistence".into());
    };
    value.function = evidence.fixture.x.clone();
    assert!(
        crate::scoped_check::check_application(
            &mut evidence.fixture.cas,
            &wrong_function,
            &evidence.premises,
            &mut budget()?
        )
        .is_err()
    );
    Ok(())
}

/// Point ownership cannot be omitted, transferred between functions or replaced by a side trace.
/// ´test:crate:boundary-owned-point-tampering´
#[test]
fn boundary_owned_point_tampering() -> TestResult {
    let mut evidence = obstruction()?;
    for mode in 0..3 {
        let mut premises = evidence.premises.clone();
        let Statement::Ownership {
            function, value, ..
        } = &mut premises.first_mut().ok_or("ownership")?.judgment.statement
        else {
            return Err("ownership statement".into());
        };
        match mode {
            0 => *value = Ownership::Present(evidence.fixture.one.clone()),
            1 => *value = Ownership::Absent,
            _ => *function = evidence.fixture.zero.clone(),
        }
        assert!(
            crate::scoped_check::check_application(
                &mut evidence.fixture.cas,
                &evidence.draft,
                &premises,
                &mut budget()?
            )
            .is_err()
        );
    }
    let mut changed = evidence.draft.clone();
    let Statement::DerivativeDoesNotExist(value) = &mut changed.conclusion else {
        return Err("nonexistence".into());
    };
    value.order = 2;
    assert!(
        crate::scoped_check::check_application(
            &mut evidence.fixture.cas,
            &changed,
            &evidence.premises,
            &mut budget()?
        )
        .is_err()
    );
    Ok(())
}

/// Removing one selector, cut or exact cell cannot retain a completeness claim.
/// ´test:crate:finite-boundary-completeness-tampering´
#[test]
fn finite_boundary_completeness_tampering() -> TestResult {
    let mut evidence = obstruction()?;
    let Statement::DerivativeDoesNotExist(value) = &evidence.draft.conclusion else {
        return Err("nonexistence".into());
    };
    let (partition, certificate) = crate::boundary_partition::build(
        &mut evidence.fixture.cas,
        &value.function,
        &evidence.fixture.domain,
        "x",
        &mut budget()?,
    )?;
    let draft = crate::proof_model::ApplicationDraft {
        context: evidence.fixture.context.clone(),
        rule: crate::rule::Primitive::JetPolynomial,
        conclusion: Statement::BoundaryPartition(Box::new(partition)),
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: crate::proof_model::Certificate::Boundary(Box::new(certificate)),
    };
    crate::scoped_check::check_application(&mut evidence.fixture.cas, &draft, &[], &mut budget()?)?;
    let mut changed = draft.clone();
    let Statement::BoundaryPartition(value) = &mut changed.conclusion else {
        return Err("partition".into());
    };
    value.boundaries.clear();
    assert!(
        crate::scoped_check::check_application(
            &mut evidence.fixture.cas,
            &changed,
            &[],
            &mut budget()?
        )
        .is_err()
    );
    let mut changed = draft;
    let crate::proof_model::Certificate::Boundary(mode) = &mut changed.certificate else {
        return Err("certificate".into());
    };
    let crate::boundary_model::BoundaryCertificate::Partition { regions, .. } = mode.as_mut()
    else {
        return Err("mode".into());
    };
    regions.clear();
    assert!(
        crate::scoped_check::check_application(
            &mut evidence.fixture.cas,
            &changed,
            &[],
            &mut budget()?
        )
        .is_err()
    );
    Ok(())
}

/// A changed derivative value and a changed polynomial coefficient are both rejected.
/// ´test:crate:boundary-value-and-jet-coefficient-tampering´
#[test]
fn boundary_value_and_jet_coefficient_tampering() -> TestResult {
    let mut evidence = crate::testing::boundary_native::smooth()?;
    let Statement::DerivativeAt(value) = &mut evidence.draft.conclusion else {
        return Err("smooth point derivative".into());
    };
    assert_eq!(value.value, evidence.fixture.one);
    value.value = evidence.fixture.zero.clone();
    assert!(
        crate::scoped_check::check_application(
            &mut evidence.fixture.cas,
            &evidence.draft,
            &evidence.premises,
            &mut budget()?
        )
        .is_err()
    );
    let jet = evidence.premises.get(1).ok_or("jet")?.clone();
    let mut earlier = Vec::new();
    for id in &jet.premises {
        earlier.push(crate::proof_view::application(
            &evidence.fixture.cas,
            *id,
            &mut budget()?,
        )?);
    }
    let mut draft = crate::proof_model::ApplicationDraft {
        context: jet.judgment.context,
        rule: jet.rule,
        conclusion: jet.judgment.statement,
        premises: vec![0, 1],
        substitutions: Vec::new(),
        certificate: jet.certificate,
    };
    let crate::proof_model::Certificate::Boundary(mode) = &mut draft.certificate else {
        return Err("jet certificate".into());
    };
    let crate::boundary_model::BoundaryCertificate::Polynomial { coefficients, .. } = mode.as_mut()
    else {
        return Err("polynomial certificate".into());
    };
    *coefficients.first_mut().ok_or("coefficient")? = crate::number::Rational::one();
    assert!(
        crate::scoped_check::check_application(
            &mut evidence.fixture.cas,
            &draft,
            &earlier,
            &mut budget()?
        )
        .is_err()
    );
    Ok(())
}

/// The name wrapper cannot authenticate an arbitrary retained case operand.
/// ´test:crate:named-case-definition-tampering´
#[test]
fn named_case_definition_tampering() -> TestResult {
    let mut fixture = crate::testing::native::Fixture::new()?;
    let forged = crate::testing::native::term(
        &mut fixture.cas,
        &fixture.header,
        crate::model::Operator::NamedCase(Box::new(crate::boundary_model::NamedCase {
            name: crate::boundary_model::CaseName::Abs,
            coordinate: "x".into(),
            parameter: None,
        })),
        std::slice::from_ref(&fixture.cases),
    )?;
    assert!(
        crate::boundary_partition::build(
            &mut fixture.cas,
            &forged,
            &fixture.domain,
            "x",
            &mut budget()?
        )
        .is_err()
    );
    Ok(())
}
