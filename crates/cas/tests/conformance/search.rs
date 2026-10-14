// SPDX-License-Identifier: AGPL-3.0-only
//! Public-contract checks for search.

use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::proof::{Judgment, Primitive, Statement};
use nrz_cas::request::{CostPolicy, Outcome, SearchPolicy, Task};
use nrz_cas::syntax::{AtomicSort, Declaration, Draft, Environment, Field, Operator, Sort};
use nrz_cas::{Fault, FaultKind, InvalidCode, Rational, Reference};

use crate::testing::budgets::derivation_budget as budget;
use crate::testing::builders::at;
use crate::testing::common::{TestResult, real};
use crate::testing::requests::Fixture;
use crate::testing::witness::{completed, equal, equality, replay};

/// All elementary formulas are tested against hand-derived rational expressions.
/// ´test:integration:generated-calculus-laws´
#[test]
fn generated_calculus_laws() -> TestResult {
    let mut fixture = Fixture::new(&["x", "y"])?;
    let variable = fixture.variable("x")?;
    let other = fixture.variable("y")?;
    let one = fixture.number("1")?;
    let two = fixture.number("2")?;
    let square = fixture.term(Operator::IntegerPower(2), std::slice::from_ref(&variable))?;
    let twice = fixture.term(Operator::Mul, &[two, variable.clone()])?;
    let denominator = fixture.term(Operator::IntegerPower(2), std::slice::from_ref(&other))?;
    let quotient = fixture.term(Operator::Div, &[variable.clone(), other.clone()])?;
    let reciprocal = fixture.term(Operator::Div, &[one.clone(), other.clone()])?;
    let negative = fixture.term(Operator::Neg, std::slice::from_ref(&variable))?;
    let minus_one = fixture.number("-1")?;
    let added = fixture.term(Operator::Add, &[square.clone(), variable.clone()])?;
    let added_derivative = fixture.term(Operator::Add, &[twice.clone(), one.clone()])?;
    let product = fixture.term(Operator::Mul, &[variable, other.clone()])?;
    for (input, expected) in [
        (square, twice),
        (quotient, reciprocal),
        (negative, minus_one),
        (added, added_derivative),
        (product, other),
    ] {
        let request = fixture.request(
            Task::Differentiate {
                coordinate: "x".into(),
                order: 1,
                layer: nrz_cas::domain::Layer::Pointwise,
                domain: fixture.domain.clone(),
            },
            &[input],
        )?;
        let result = completed(fixture.cas.run(&request))?;
        equal(
            &mut fixture,
            crate::testing::witness::right(result.witness.judgment())?,
            &expected,
        )?;
        replay(&fixture, &result)?;
    }
    let reciprocal_square = fixture.term(Operator::Div, &[one, denominator])?;
    let request = fixture.request(
        Task::Differentiate {
            coordinate: "x".into(),
            order: 1,
            layer: nrz_cas::domain::Layer::Pointwise,
            domain: fixture.domain.clone(),
        },
        &[reciprocal_square],
    )?;
    let result = completed(fixture.cas.run(&request))?;
    assert!(matches!(
            fixture.cas.expression(crate::testing::witness::right(result.witness.judgment())?)?.0,
        Operator::Rational(value) if value.is_zero()));
    Ok(())
}

/// Strict zero and negative powers retain nonzero while exponent one adds no guard.
/// ´test:integration:generated-power-domains´
#[test]
fn generated_power_domains() -> TestResult {
    let mut fixture = Fixture::new(&["x"])?;
    let variable = fixture.variable("x")?;
    for exponent in [-2, 0, 1] {
        let power = fixture.term(
            Operator::IntegerPower(exponent),
            std::slice::from_ref(&variable),
        )?;
        let request = fixture.request(
            Task::Differentiate {
                coordinate: "x".into(),
                order: 1,
                layer: nrz_cas::domain::Layer::Pointwise,
                domain: fixture.domain.clone(),
            },
            &[power],
        )?;
        let result = completed(fixture.cas.run(&request))?;
        assert_eq!(
            result.witness.judgment().residuals.is_empty(),
            exponent == 1
        );
        if exponent != 1 {
            assert_eq!(
                result.witness.judgment().residuals,
                vec![crate::testing::witness::guarded(
                    &mut fixture.cas,
                    &fixture.context,
                    nrz_cas::proof::GuardKind::NonZero,
                    &variable
                )?],
            );
        }
        replay(&fixture, &result)?;
    }
    Ok(())
}

/// Definedness is retained even when the formal derivative or simplified expression is zero.
/// ´test:integration:generated-zero-domain´
#[test]
fn generated_zero_domain() -> TestResult {
    let mut fixture = Fixture::new(&["x"])?;
    let variable = fixture.variable("x")?;
    let zero = fixture.number("0")?;
    let invalid = fixture.term(Operator::Div, &[variable, zero.clone()])?;
    let input = fixture.term(Operator::Mul, &[zero, invalid])?;
    for operation in [
        Task::Simplify,
        Task::Differentiate {
            coordinate: "x".into(),
            order: 1,
            layer: nrz_cas::domain::Layer::Pointwise,
            domain: fixture.domain.clone(),
        },
    ] {
        let request = fixture.request(operation, std::slice::from_ref(&input))?;
        assert!(matches!(
            fixture.cas.run(&request),
            Outcome::Refused {
                fault: Fault {
                    kind: FaultKind::Invalid(InvalidCode::RefutedDomain),
                    ..
                },
                ..
            }
        ));
    }
    Ok(())
}

/// Duplicate independent variables are refused before publishing a partial Jacobian.
/// ´test:integration:generated-jacobian-order´
#[test]
fn generated_jacobian_order() -> TestResult {
    let mut fixture = Fixture::new(&["x", "y"])?;
    let variable = fixture.variable("x")?;
    let other = fixture.variable("y")?;
    let request = fixture.request(
        Task::Jacobian(vec!["y".into(), "x".into()]),
        &[variable.clone(), other],
    )?;
    let result = completed(fixture.cas.run(&request))?;
    let cells = fixture.cas.operands(
        crate::testing::witness::right(result.witness.judgment())?,
        &mut budget()?,
    )?;
    for (cell, expected) in cells.iter().zip([0, 1, 1, 0]) {
        let (operator, _) = fixture.cas.expression(cell)?;
        assert_eq!(
            *operator,
            Operator::Rational(Rational::from_ratio(expected, 1, &mut budget()?)?),
        );
    }
    replay(&fixture, &result)?;
    let repeated = fixture.request(Task::Jacobian(vec!["x".into(), "x".into()]), &[variable])?;
    let before = fixture.cas.counts();
    assert!(matches!(
        fixture.cas.run(&repeated),
        Outcome::Invalid { .. }
    ));
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// Frozen certification ignores syntax edges and changes only when a checked cheaper route lands.
/// ´test:integration:frozen-cost-snapshots´
#[test]
fn frozen_cost_snapshots() -> TestResult {
    let mut fixture = Fixture::new(&["x"])?;
    let variable = fixture.variable("x")?;
    let zero = fixture.number("0")?;
    let sum = fixture.term(Operator::Add, &[variable.clone(), zero])?;
    let proof = fixture.cas.check_proof(
        &fixture.context,
        &[equality(
            &fixture.context,
            &fixture.domain,
            Primitive::Jets,
            &sum,
            &variable,
        )],
        &mut budget()?,
    )?;
    let first = fixture.cas.certify_frozen_cost(
        proof.judgment(),
        &CostPolicy::default(),
        &mut budget()?,
    )?;
    assert_eq!(first.shortest_cost(), Some(4));
    assert_eq!(first.settled(), 2);
    fixture.cas.register_derived_with_policy(
        &proof,
        proof.judgment(),
        &CostPolicy::default(),
        1,
        &mut budget()?,
    )?;
    let second = fixture.cas.certify_frozen_cost(
        proof.judgment(),
        &CostPolicy::default(),
        &mut budget()?,
    )?;
    assert_eq!(second.shortest_cost(), Some(1));
    assert_ne!(first.manifest(), second.manifest());
    assert_eq!(first.shortest_cost(), Some(4));
    assert!(
        fixture
            .cas
            .register_derived_with_policy(
                &proof,
                proof.judgment(),
                &CostPolicy::default(),
                1,
                &mut budget()?
            )
            .is_err()
    );
    let mut tiny = Budget::new(Limits {
        work: 0,
        ..Limits::default()
    })?;
    assert!(
        fixture
            .cas
            .certify_frozen_cost(proof.judgment(), &CostPolicy::default(), &mut tiny,)
            .is_err()
    );
    Ok(())
}

/// Policy changes discard old improvement records while the underlying theorem stays reusable.
/// ´test:integration:versioned-cost-policies´
#[test]
fn versioned_cost_policies() -> TestResult {
    let mut fixture = Fixture::new(&["x"])?;
    let variable = fixture.variable("x")?;
    let zero = fixture.number("0")?;
    let sum = fixture.term(Operator::Add, &[variable.clone(), zero])?;
    let proof = fixture.cas.check_proof(
        &fixture.context,
        &[equality(
            &fixture.context,
            &fixture.domain,
            Primitive::Jets,
            &sum,
            &variable,
        )],
        &mut budget()?,
    )?;
    fixture
        .cas
        .register_derived(&proof, proof.judgment(), 1, &mut budget()?)?;
    let mut policy = CostPolicy {
        key: "fixture".into(),
        version: 1,
        ..CostPolicy::default()
    };
    policy.weights.fill(7);
    let certificate = fixture
        .cas
        .certify_frozen_cost(proof.judgment(), &policy, &mut budget()?)?;
    assert_eq!(certificate.shortest_cost(), Some(7));
    fixture.cas.register_derived_with_policy(
        &proof,
        proof.judgment(),
        &policy,
        2,
        &mut budget()?,
    )?;
    assert_eq!(
        fixture
            .cas
            .certify_frozen_cost(proof.judgment(), &policy, &mut budget()?)?
            .shortest_cost(),
        Some(2),
    );
    let old = fixture.cas.counts();
    *policy.weights.first_mut().ok_or("missing policy weight")? = 8;
    assert!(
        fixture
            .cas
            .certify_frozen_cost(proof.judgment(), &policy, &mut budget()?)
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), old);
    policy.version = 2;
    assert_eq!(
        fixture
            .cas
            .certify_frozen_cost(proof.judgment(), &policy, &mut budget()?)?
            .shortest_cost(),
        Some(7),
    );
    Ok(())
}

/// Empty, nonpositive and incomplete cost tables refuse before request publication.
/// ´test:integration:cost-policy-refusals´
#[test]
fn cost_policy_refusals() -> TestResult {
    let mut fixture = Fixture::new(&["x"])?;
    let variable = fixture.variable("x")?;
    let request = fixture.request(Task::Simplify, &[variable])?;
    for case in 0..4 {
        let mut costs = CostPolicy {
            key: "invalid-fixture".into(),
            version: 1,
            ..CostPolicy::default()
        };
        match case {
            0 => costs.weights.clear(),
            1 => *costs.weights.first_mut().ok_or("missing weight")? = 0,
            2 => costs.version = 0,
            _ => costs.key.clear(),
        }
        let before = fixture.cas.counts();
        assert!(matches!(
            fixture.cas.run_with_policy(
                &request,
                &SearchPolicy {
                    costs,
                    allow_conditional: true
                }
            ),
            Outcome::Invalid { .. }
        ));
        assert_eq!(fixture.cas.counts(), before);
    }
    Ok(())
}

/// No match or replay allowance can silently mint a successful new derivation.
/// ´test:integration:search-resource-refusals´
#[test]
fn search_resource_refusals() -> TestResult {
    for (kind, no_replay) in [
        (BudgetKind::MatchAttempts, false),
        (BudgetKind::ReplayWork, true),
    ] {
        let mut fixture = Fixture::new(&["x"])?;
        let variable = fixture.variable("x")?;
        let one = fixture.number("1")?;
        let quotient = fixture.term(Operator::Div, &[variable.clone(), variable])?;
        let mut request = fixture.request(
            Task::Equal {
                domain: fixture.domain.clone(),
                domain_contract: nrz_cas::proof::DomainContract::DefinedAndEqual,
            },
            &[quotient, one],
        )?;
        if no_replay {
            request.limits.replay_work = Some(0);
        } else {
            request.limits.match_attempts = Some(0);
        }
        let outcome = fixture.cas.run(&request);
        let Outcome::Exhausted { fault, .. } = outcome else {
            return Err(format!("expected named exhaustion: {outcome:?}").into());
        };
        assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind));
    }
    Ok(())
}

/// The output boundary preserves an in-store verified handle and marks it `NotExported`.
/// ´test:integration:generated-output-boundary´
#[test]
fn generated_output_boundary() -> TestResult {
    let mut fixture = Fixture::new(&["x"])?;
    let variable = fixture.variable("x")?;
    let mut request = fixture.request(
        Task::Differentiate {
            coordinate: "x".into(),
            order: 1,
            layer: nrz_cas::domain::Layer::Pointwise,
            domain: fixture.domain.clone(),
        },
        &[variable],
    )?;
    request.limits.output_bytes = Some(0);
    let result = completed(fixture.cas.run(&request))?;
    assert!(result.response.is_none());
    assert!(result.witness.is_discharged());
    assert_ne!(
        fixture
            .cas
            .export_witness(&result.witness, &mut budget()?)?,
        [] as [u8; 0]
    );
    Ok(())
}

/// Conditional-result refusal does not erase a denominator obligation.
/// ´test:integration:search-requires-discharged-domain´
#[test]
fn search_requires_discharged_domain() -> TestResult {
    let mut fixture = Fixture::new(&["x"])?;
    let variable = fixture.variable("x")?;
    let one = fixture.number("1")?;
    let quotient = fixture.term(Operator::Div, &[variable.clone(), variable])?;
    let request = fixture.request(
        Task::Equal {
            domain: fixture.domain.clone(),
            domain_contract: nrz_cas::proof::DomainContract::DefinedAndEqual,
        },
        &[quotient, one],
    )?;
    let outcome = fixture.cas.run_with_policy(
        &request,
        &SearchPolicy {
            allow_conditional: false,
            ..SearchPolicy::default()
        },
    );
    assert!(matches!(outcome, Outcome::Unresolved { .. }));
    Ok(())
}

/// Independent stores with identical request order emit identical response and witness bytes.
/// ´test:integration:generated-request-determinism´
#[test]
fn generated_request_determinism() -> TestResult {
    let build = || -> TestResult<(Vec<u8>, Vec<u8>)> {
        let mut fixture = Fixture::new(&["x"])?;
        let variable = fixture.variable("x")?;
        let cubic = fixture.term(Operator::IntegerPower(3), &[variable])?;
        let request = fixture.request(
            Task::Differentiate {
                coordinate: "x".into(),
                order: 1,
                layer: nrz_cas::domain::Layer::Pointwise,
                domain: fixture.domain.clone(),
            },
            &[cubic],
        )?;
        let result = completed(fixture.cas.run(&request))?;
        let bytes = replay(&fixture, &result)?;
        Ok((bytes, result.response.ok_or("response was not exported")?))
    };
    assert_eq!(build()?, build()?);
    Ok(())
}

/// A missing elementary function rule reports its exact pending expression, without a whole result.
/// ´test:integration:generated-function-remains-pending´
#[test]
fn generated_function_remains_pending() -> TestResult {
    let mut fixture = Fixture::new(&["x"])?;
    let handles = fixture.cas.admit(
        &[
            Draft::Header(nrz_cas::domain::HeaderDraft::Core(Environment {
                declarations: vec![
                    Declaration {
                        key: "x".into(),
                        display_name: "x".into(),
                        sort: real(),
                    },
                    Declaration {
                        key: "f".into(),
                        display_name: "f".into(),
                        sort: Sort::Function {
                            inputs: vec![AtomicSort::Scalar(Field::Real)],
                            output: AtomicSort::Scalar(Field::Real),
                        },
                    },
                ],
                ..Environment::default()
            })),
            Draft::Region(nrz_cas::domain::RegionDraft {
                header: Reference::Local(0),
                scope: Vec::new(),
                ambient: nrz_cas::domain::AmbientDomain::Assignments,
                expression: nrz_cas::domain::RegionExpr::Ambient,
            }),
            Draft::Context(nrz_cas::domain::ContextDraft::Root {
                header: Reference::Local(0),
                admissible: Reference::Local(1),
                assumptions: Vec::new(),
            }),
        ],
        &mut budget()?,
    )?;
    fixture.context = at(&handles, 2)?;
    fixture.domain = fixture.cas.admissible_region(&fixture.context)?;
    let variable = fixture.variable("x")?;
    let call = fixture.term(Operator::Call("f".into()), &[variable])?;
    let request = fixture.request(
        Task::Differentiate {
            coordinate: "x".into(),
            order: 1,
            layer: nrz_cas::domain::Layer::Pointwise,
            domain: fixture.domain.clone(),
        },
        std::slice::from_ref(&call),
    )?;
    let outcome = fixture.cas.run(&request);
    let Outcome::Unresolved {
        blocker: Some(fault),
        ..
    } = outcome
    else {
        return Err(format!("expected pending function: {outcome:?}").into());
    };
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedFragment)
    );
    assert_eq!(fault.location.logical_id, Some(call.logical_id().serial));
    Ok(())
}

/// A different caller endpoint still fails portable replay after automatic generation.
/// ´test:integration:generated-expected-claim-refusal´
#[test]
fn generated_expected_claim_refusal() -> TestResult {
    let mut fixture = Fixture::new(&["x"])?;
    let variable = fixture.variable("x")?;
    let zero = fixture.number("0")?;
    let request = fixture.request(
        Task::Differentiate {
            coordinate: "x".into(),
            order: 1,
            layer: nrz_cas::domain::Layer::Pointwise,
            domain: fixture.domain.clone(),
        },
        std::slice::from_ref(&variable),
    )?;
    let result = completed(fixture.cas.run(&request))?;
    let bytes = fixture
        .cas
        .export_witness(&result.witness, &mut budget()?)?;
    let wrong = Judgment {
        context: fixture.context.clone(),
        layer: nrz_cas::domain::Layer::Pointwise,
        space: None,
        statement: Statement::DerivativeOn {
            function: variable,
            derivative: zero,
            coordinate: "x".into(),
            domain: fixture.domain.clone(),
        },
        residuals: Vec::new(),
    };
    assert!(fixture.cas.replay(&bytes, &wrong, &mut budget()?).is_err());
    Ok(())
}

/// Even an empty frozen path cannot discard a nonzero domain requirement.
/// ´test:integration:frozen-domain-refusal´
#[test]
fn frozen_domain_refusal() -> TestResult {
    let mut fixture = Fixture::new(&["x"])?;
    let variable = fixture.variable("x")?;
    let quotient = fixture.term(Operator::Div, &[variable.clone(), variable])?;
    let claim = Judgment {
        context: fixture.context.clone(),
        layer: nrz_cas::domain::Layer::Pointwise,
        space: None,
        statement: Statement::Equality {
            left: quotient.clone(),
            right: quotient,
            domain: fixture.domain.clone(),
            domain_contract: nrz_cas::proof::DomainContract::DefinedAndEqual,
        },
        residuals: Vec::new(),
    };
    let before = fixture.cas.counts();
    assert!(
        fixture
            .cas
            .certify_frozen_cost(&claim, &CostPolicy::default(), &mut budget()?,)
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}
