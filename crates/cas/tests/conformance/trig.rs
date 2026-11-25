// SPDX-License-Identifier: AGPL-3.0-only
//! Closed Real syntax admits no trigonometric rule authority.

use crate::testing::common::{TestResult, default_budget, real};
use crate::testing::trig::{Fixture, operators};
use nrz_cas::syntax::{AtomicSort, Binding, Draft, Field, Operator, Sort};
use nrz_cas::{FaultKind, InvalidCode, Rational};

/// Checked projections and integer embeddings preserve their original typed operands.
/// ´test:integration:trig-real-admission´
#[test]
fn trig_real_admission() -> TestResult {
    let mut f = Fixture::new()?;
    let mut inputs = vec![
        f.x.clone(),
        f.term(Operator::Rational(Rational::zero()), &[])?,
    ];
    for projection in [Operator::RealPart, Operator::ImagPart] {
        let projected = f.term(projection, &[f.z.clone()])?;
        assert_eq!(
            f.cas.operands(&projected, &mut default_budget()?)?,
            [f.z.clone()]
        );
        inputs.push(projected);
    }
    let mut integer = f.draft(Operator::Rational(Rational::one()), &[]);
    integer.sort = Sort::Value(AtomicSort::Integer);
    let integer = f
        .cas
        .admit(&[Draft::Expression(integer)], &mut default_budget()?)?
        .pop()
        .ok_or("integer")?;
    let embedded = f.term(Operator::IntegerEmbed, std::slice::from_ref(&integer))?;
    assert_eq!(
        f.cas.operands(&embedded, &mut default_budget()?)?,
        [integer]
    );
    inputs.push(embedded);
    let ambient = f
        .cas
        .scalar_ambient(&f.x, &mut crate::testing::budgets::graph_budget()?)?;
    let scope = vec![vec![AtomicSort::Scalar(Field::Real)]];
    let bound = f.scoped(Operator::Bound { depth: 0, slot: 0 }, scope.clone(), &[])?;
    assert!(ambient.is_some());
    assert_ne!(
        ambient,
        f.cas
            .scalar_ambient(&f.z, &mut crate::testing::budgets::graph_budget()?)?
    );
    for (operator, name) in operators() {
        f.scoped(
            operator.clone(),
            scope.clone(),
            std::slice::from_ref(&bound),
        )?;
        for input in &inputs {
            let node = f.term(operator.clone(), std::slice::from_ref(input))?;
            let again = f.term(operator.clone(), std::slice::from_ref(input))?;
            let opaque = f.term(Operator::Call(name.into()), std::slice::from_ref(input))?;
            assert_eq!(node, again);
            assert_ne!(node, opaque);
            assert_eq!(
                f.cas
                    .expression(&node, &mut crate::testing::budgets::graph_budget()?)?,
                (&operator, &real())
            );
            assert_eq!(
                f.cas
                    .scalar_ambient(&node, &mut crate::testing::budgets::graph_budget()?)?,
                ambient
            );
            assert_eq!(
                f.cas.operands(&node, &mut default_budget()?)?,
                std::slice::from_ref(input)
            );
        }
    }
    Ok(())
}

/// Wrong arity, scope, and Complex inputs fail atomically at structural admission.
/// ´test:integration:trig-invalid-admission´
#[test]
fn trig_invalid_admission() -> TestResult {
    let mut f = Fixture::new()?;
    let mut zero = f.draft(Operator::Rational(Rational::zero()), &[]);
    zero.sort = Sort::Value(AtomicSort::Scalar(Field::Complex));
    let zero = f
        .cas
        .admit(&[Draft::Expression(zero)], &mut default_budget()?)?
        .pop()
        .ok_or("complex zero")?;
    let mut non_real = vec![f.z.clone(), zero];
    for sort in [AtomicSort::Integer, AtomicSort::Scalar(Field::Rational)] {
        let mut literal = f.draft(Operator::Rational(Rational::one()), &[]);
        literal.sort = Sort::Value(sort);
        non_real.push(
            f.cas
                .admit(&[Draft::Expression(literal)], &mut default_budget()?)?
                .pop()
                .ok_or("non-Real literal")?,
        );
    }
    for (operator, _) in operators() {
        let valid = f.draft(operator.clone(), &[f.x.clone()]);
        let mut cases = Vec::new();
        for operand in &non_real {
            cases.push((
                f.draft(operator.clone(), std::slice::from_ref(operand)),
                InvalidCode::Sort,
            ));
            let mut complex_result = f.draft(operator.clone(), std::slice::from_ref(operand));
            complex_result.sort = Sort::Value(AtomicSort::Scalar(Field::Complex));
            cases.push((complex_result, InvalidCode::Sort));
        }
        cases.push((f.draft(operator.clone(), &[]), InvalidCode::Sort));
        cases.push((
            f.draft(operator, &[f.x.clone(), f.x.clone()]),
            InvalidCode::Sort,
        ));
        let mut scope = valid.clone();
        scope.scope = vec![vec![AtomicSort::Scalar(Field::Real)]];
        cases.push((scope, InvalidCode::Scope));
        let mut result = valid.clone();
        result.sort = Sort::Value(AtomicSort::Scalar(Field::Complex));
        cases.push((result, InvalidCode::Sort));
        let mut binding = valid.clone();
        binding.binding = Some(Binding {
            sorts: vec![AtomicSort::Scalar(Field::Real)],
            body_operands: vec![0],
        });
        cases.push((binding, InvalidCode::Scope));
        for (invalid, code) in cases {
            let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
            let counts = f.cas.counts();
            let fault = f
                .cas
                .admit(
                    &[Draft::Expression(valid.clone()), Draft::Expression(invalid)],
                    &mut default_budget()?,
                )
                .err()
                .ok_or("invalid trig node admitted")?;
            assert_eq!(fault.kind, FaultKind::Invalid(code));
            assert_eq!(f.cas.counts(), counts);
            assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
        }
    }
    Ok(())
}

/// Closed trigonometric syntax has no chain rule and retains its exact pending derivative blocker.
/// ´test:integration:trig-derivative-remains-pending´
#[test]
fn trig_derivative_remains_pending() -> TestResult {
    use nrz_cas::request::{Outcome, Task};

    let mut f = crate::testing::requests::Fixture::new(&["x"])?;
    let x = f.variable("x")?;
    for (operator, _) in operators() {
        let sine = f.term(operator, std::slice::from_ref(&x))?;
        let request = f.request(
            Task::Differentiate {
                coordinate: "x".into(),
                order: 1,
                layer: nrz_cas::domain::Layer::Pointwise,
                domain: f.domain.clone(),
            },
            std::slice::from_ref(&sine),
        )?;
        let outcome = f.cas.run(&request);
        let Outcome::Unresolved {
            blocker: Some(fault),
            ..
        } = outcome
        else {
            return Err(format!("expected pending trig derivative: {outcome:?}").into());
        };
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::UnsupportedFragment)
        );
        assert_eq!(fault.location.logical_id, Some(sine.logical_id().serial));
    }
    Ok(())
}

/// Closed trig syntax refuses enclosure before evaluation Work or evidence publication.
/// ´test:integration:trig-enclosure-preflight-refusal´
#[test]
fn trig_enclosure_preflight_refusal() -> TestResult {
    use nrz_cas::enclosure::{
        CoordinateBound, EnclosureGoal, EnclosureOutcome, EnclosureRequest, Interval,
        RefinementPolicy,
    };

    let mut f = crate::testing::requests::Fixture::new(&["x"])?;
    let x = f.variable("x")?;
    for (operator, _) in operators() {
        let target = f.term(operator, std::slice::from_ref(&x))?;
        let request = EnclosureRequest {
            target,
            context: f.context.clone(),
            coordinates: vec![CoordinateBound {
                coordinate: x.clone(),
                interval: Interval::new(Rational::zero(), Rational::one(), &mut default_budget()?)?,
            }],
            goal: EnclosureGoal::Width,
        };
        let policy = RefinementPolicy {
            version: 1,
            precision: 4,
            grid_ceiling: 64,
            rounds: 2,
            reduction_ceiling: 16,
            term_ceiling: 64,
            checking_work: 50_000,
        };
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        let mut budget = default_budget()?;
        let report = f
            .cas
            .enclose(&request, policy, &mut budget, &mut |_| false)?;
        assert_eq!(report.outcome, EnclosureOutcome::Unsupported);
        assert_eq!(report.work, 0);
        assert_eq!(budget.usage().work, 0);
        assert_eq!(report.completed_rounds, 0);
        assert!(report.checked.is_none());
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
    }
    Ok(())
}
