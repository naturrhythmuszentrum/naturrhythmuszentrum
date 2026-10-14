// SPDX-License-Identifier: AGPL-3.0-only
//! Independent generated-calculus, expansion, policy and finite-cost acceptance fixtures.

use nrz_cas::{
    AtomicSort, Budget, BudgetKind, Cas, Claim, Constant, CostPolicy, Declaration, Draft,
    Environment, ExpressionDraft, Fault, FaultKind, Field, GuardKind, Handle, InvalidCode, Limits,
    LimitsOverride, Operation, Operator, Outcome, Primitive, ProofStep, Rational, Reference,
    Relation, Request, SearchPolicy, SearchResult, Sort, Substitution,
};

/// Setup failures are reported instead of panicking.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Deliberately generous work for fixtures with many independent setup transactions.
fn budget() -> TestResult<Budget> {
    Ok(Budget::new(Limits {
        work: 2_000_000_000,
        replay_work: 2_000_000_000,
        ..Limits::default()
    })?)
}

/// Scalar real sort, without any physical interpretation.
const fn real() -> Sort {
    Sort::Value(AtomicSort::Scalar(Field::Real))
}

/// Checked extraction of one setup result.
fn at(handles: &[Handle], index: usize) -> TestResult<Handle> {
    handles
        .get(index)
        .cloned()
        .ok_or_else(|| "missing fixture handle".into())
}

/// A minimal independent request builder, with a complete generated occurrence walk.
struct Fixture {
    /// Store under test.
    cas: Cas,
    /// Shared mathematical context.
    context: Handle,
}

impl Fixture {
    /// Declare only the named real coordinates.
    fn new(keys: &[&str]) -> TestResult<Self> {
        let mut cas = Cas::new(Limits::default())?;
        let handles = cas.admit(
            &[Draft::Context {
                environment: Environment {
                    declarations: keys
                        .iter()
                        .map(|key| Declaration {
                            key: (*key).into(),
                            display_name: (*key).into(),
                            sort: real(),
                        })
                        .collect(),
                    ..Environment::default()
                },
                assumptions: Vec::new(),
            }],
            &mut budget()?,
        )?;
        Ok(Self {
            cas,
            context: at(&handles, 0)?,
        })
    }

    /// Retain the caller's exact ordered expression spelling.
    fn term(&mut self, operator: Operator, operands: &[Handle]) -> TestResult<Handle> {
        let handles = self.cas.admit(
            &[Draft::Expression(ExpressionDraft {
                operator,
                sort: real(),
                context: Reference::Existing(self.context.clone()),
                scope: Vec::new(),
                binding: None,
                operands: operands.iter().cloned().map(Reference::Existing).collect(),
            })],
            &mut budget()?,
        )?;
        at(&handles, 0)
    }

    /// Exact scalar literal setup.
    fn number(&mut self, text: &str) -> TestResult<Handle> {
        let value = Rational::parse(text, &mut budget()?)?;
        self.term(Operator::Rational(value), &[])
    }

    /// Distinct declared free identity, unaffected by display names.
    fn variable(&mut self, key: &str) -> TestResult<Handle> {
        self.term(Operator::Free(key.into()), &[])
    }

    /// Each request receives all its occurrence receipts before execution.
    fn request(&mut self, operation: Operation, expressions: &[Handle]) -> TestResult<Request> {
        let mut inputs = Vec::new();
        let mut receipts = Vec::new();
        for expression in expressions {
            let root = self
                .cas
                .materialize_derived(expression, expression, &mut budget()?)?;
            let receipt = self.cas.walk_derived(&root, &mut budget()?)?.receipt;
            inputs.push(root);
            receipts.push(receipt);
        }
        Ok(Request {
            operation,
            context: self.context.clone(),
            inputs,
            receipts,
            limits: LimitsOverride {
                work: Some(2_000_000_000),
                replay_work: Some(2_000_000_000),
                ..LimitsOverride::default()
            },
        })
    }
}

/// Only a completed successful outcome is acceptable for a positive fixture.
fn completed(outcome: Outcome) -> TestResult<SearchResult> {
    match outcome {
        Outcome::Verified { result, .. } | Outcome::Conditional { result, .. } => Ok(result),
        other => Err(format!("expected completed derivation, got {other:?}").into()),
    }
}

/// One untrusted exact equality proposal, used for independently supplied target formulas.
fn equality(primitive: Primitive, left: &Handle, right: &Handle) -> ProofStep {
    ProofStep {
        primitive,
        relation: Relation::EqualOnDomain,
        left: left.clone(),
        right: right.clone(),
        premises: Vec::new(),
        path: Vec::new(),
        substitutions: Vec::new(),
    }
}

/// Check a produced expression against an independently written exact formula.
fn equal(fixture: &mut Fixture, left: &Handle, right: &Handle) -> TestResult {
    fixture.cas.check_proof(
        &fixture.context,
        &[equality(Primitive::RationalIdentity, left, right)],
        &mut budget()?,
    )?;
    Ok(())
}

/// Full proof replay works in an empty store without the generator's memo table.
fn replay(fixture: &Fixture, result: &SearchResult) -> TestResult<Vec<u8>> {
    let bytes = fixture
        .cas
        .export_witness(&result.witness, &mut budget()?)?;
    let mut fresh = Cas::new(Limits::default())?;
    let checked = fresh.inspect(&bytes, &mut budget()?)?;
    assert_eq!(fresh.export_witness(&checked, &mut budget()?)?, bytes);
    Ok(bytes)
}

/// A shared square differentiates to the hand-derived 2x and replays without producer state.
/// ´test:integration:generated-square-derivative´
#[test]
fn generated_square_derivative() -> TestResult {
    let mut fixture = Fixture::new(&["x"])?;
    let variable = fixture.variable("x")?;
    let square = fixture.term(Operator::Mul, &[variable.clone(), variable.clone()])?;
    let two = fixture.number("2")?;
    let expected = fixture.term(Operator::Mul, &[two, variable])?;
    let request = fixture.request(Operation::Differentiate("x".into()), &[square])?;
    let result = completed(fixture.cas.run(&request))?;
    assert!(result.witness.is_discharged());
    equal(&mut fixture, &result.witness.claim().right, &expected)?;
    assert_eq!(
        result.witness.claim().relation,
        Relation::Derivative {
            variable: "x".into()
        }
    );
    replay(&fixture, &result)?;
    Ok(())
}

/// An actual larger intermediate appears before the difference quotient reaches 2x+h.
/// ´test:integration:generated-difference-quotient´
#[test]
fn generated_difference_quotient() -> TestResult {
    let mut fixture = Fixture::new(&["x", "h"])?;
    let variable = fixture.variable("x")?;
    let increment = fixture.variable("h")?;
    let sum = fixture.term(Operator::Add, &[variable.clone(), increment.clone()])?;
    let raised = fixture.term(Operator::IntegerPower(2), &[sum])?;
    let square = fixture.term(Operator::IntegerPower(2), std::slice::from_ref(&variable))?;
    let difference = fixture.term(Operator::Sub, &[raised, square])?;
    let quotient = fixture.term(Operator::Div, &[difference, increment.clone()])?;
    let two = fixture.number("2")?;
    let twice = fixture.term(Operator::Mul, &[two, variable])?;
    let expected = fixture.term(Operator::Add, &[twice, increment.clone()])?;
    let mut request = fixture.request(Operation::Equal, &[quotient, expected.clone()])?;
    request.limits = LimitsOverride::default();
    let result = completed(fixture.cas.run(&request))?;
    assert_eq!(result.witness.claim().right, expected);
    assert!(result.expansion_credits > 0);
    assert!(result.trace.len() > 2);
    assert_eq!(result.witness.claim().residuals.len(), 1);
    let guard = result
        .witness
        .claim()
        .residuals
        .first()
        .ok_or("missing increment guard")?;
    assert_eq!(guard.kind, GuardKind::NonZero);
    assert_eq!(guard.term, increment);
    replay(&fixture, &result)?;
    Ok(())
}

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
        let request = fixture.request(Operation::Differentiate("x".into()), &[input])?;
        let result = completed(fixture.cas.run(&request))?;
        equal(&mut fixture, &result.witness.claim().right, &expected)?;
        replay(&fixture, &result)?;
    }
    let reciprocal_square = fixture.term(Operator::Div, &[one, denominator])?;
    let request = fixture.request(Operation::Differentiate("x".into()), &[reciprocal_square])?;
    let result = completed(fixture.cas.run(&request))?;
    assert!(
        matches!(fixture.cas.expression(&result.witness.claim().right)?.0,
        Operator::Rational(value) if value.is_zero())
    );
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
        let request = fixture.request(Operation::Differentiate("x".into()), &[power])?;
        let result = completed(fixture.cas.run(&request))?;
        assert_eq!(result.witness.claim().residuals.is_empty(), exponent == 1);
        if exponent != 1 {
            assert_eq!(
                result
                    .witness
                    .claim()
                    .residuals
                    .first()
                    .ok_or("missing strict guard")?
                    .term,
                variable,
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
    for operation in [Operation::Simplify, Operation::Differentiate("x".into())] {
        let request = fixture.request(operation, std::slice::from_ref(&input))?;
        assert!(matches!(
            fixture.cas.run(&request),
            Outcome::Unresolved {
                blocker: Some(Fault {
                    kind: FaultKind::Invalid(InvalidCode::RefutedDomain),
                    ..
                }),
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
        Operation::Jacobian(vec!["y".into(), "x".into()]),
        &[variable.clone(), other],
    )?;
    let result = completed(fixture.cas.run(&request))?;
    let cells = fixture
        .cas
        .operands(&result.witness.claim().right, &mut budget()?)?;
    for (cell, expected) in cells.iter().zip([0, 1, 1, 0]) {
        let (operator, _) = fixture.cas.expression(cell)?;
        assert_eq!(
            *operator,
            Operator::Rational(Rational::from_ratio(expected, 1, &mut budget()?)?),
        );
    }
    replay(&fixture, &result)?;
    let repeated = fixture.request(
        Operation::Jacobian(vec!["x".into(), "x".into()]),
        &[variable],
    )?;
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
        &[equality(Primitive::Jets, &sum, &variable)],
        &mut budget()?,
    )?;
    let first =
        fixture
            .cas
            .certify_frozen_cost(proof.claim(), &CostPolicy::default(), &mut budget()?)?;
    assert_eq!(first.shortest_cost(), Some(4));
    assert_eq!(first.settled(), 2);
    fixture.cas.register_derived_with_policy(
        &proof,
        proof.claim(),
        &CostPolicy::default(),
        1,
        &mut budget()?,
    )?;
    let second =
        fixture
            .cas
            .certify_frozen_cost(proof.claim(), &CostPolicy::default(), &mut budget()?)?;
    assert_eq!(second.shortest_cost(), Some(1));
    assert_ne!(first.manifest(), second.manifest());
    assert_eq!(first.shortest_cost(), Some(4));
    assert!(
        fixture
            .cas
            .register_derived_with_policy(
                &proof,
                proof.claim(),
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
            .certify_frozen_cost(proof.claim(), &CostPolicy::default(), &mut tiny,)
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
        &[equality(Primitive::Jets, &sum, &variable)],
        &mut budget()?,
    )?;
    fixture
        .cas
        .register_derived(&proof, proof.claim(), 1, &mut budget()?)?;
    let mut policy = CostPolicy {
        key: "fixture".into(),
        version: 1,
        ..CostPolicy::default()
    };
    policy.weights.fill(7);
    let certificate = fixture
        .cas
        .certify_frozen_cost(proof.claim(), &policy, &mut budget()?)?;
    assert_eq!(certificate.shortest_cost(), Some(7));
    fixture
        .cas
        .register_derived_with_policy(&proof, proof.claim(), &policy, 2, &mut budget()?)?;
    assert_eq!(
        fixture
            .cas
            .certify_frozen_cost(proof.claim(), &policy, &mut budget()?)?
            .shortest_cost(),
        Some(2),
    );
    let old = fixture.cas.counts();
    *policy.weights.first_mut().ok_or("missing policy weight")? = 8;
    assert!(
        fixture
            .cas
            .certify_frozen_cost(proof.claim(), &policy, &mut budget()?)
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), old);
    policy.version = 2;
    assert_eq!(
        fixture
            .cas
            .certify_frozen_cost(proof.claim(), &policy, &mut budget()?)?
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
    let request = fixture.request(Operation::Simplify, &[variable])?;
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

/// A derived equality is expanded from graph proof records and rechecked during search.
/// ´test:integration:search-reuses-checked-shortcut´
#[test]
fn search_reuses_checked_shortcut() -> TestResult {
    let mut fixture = Fixture::new(&["x"])?;
    let variable = fixture.variable("x")?;
    let zero = fixture.number("0")?;
    let sum = fixture.term(Operator::Add, &[variable.clone(), zero])?;
    let proof = fixture.cas.check_proof(
        &fixture.context,
        &[equality(Primitive::Jets, &sum, &variable)],
        &mut budget()?,
    )?;
    fixture
        .cas
        .register_derived(&proof, proof.claim(), 1, &mut budget()?)?;
    let request = fixture.request(Operation::Equal, &[sum, variable])?;
    let result = completed(fixture.cas.run(&request))?;
    assert_eq!(result.cost, 1);
    replay(&fixture, &result)?;
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
        let mut request = fixture.request(Operation::Equal, &[quotient, one])?;
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
    let mut request = fixture.request(Operation::Differentiate("x".into()), &[variable])?;
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
    let request = fixture.request(Operation::Equal, &[quotient, one])?;
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
        let request = fixture.request(Operation::Differentiate("x".into()), &[cubic])?;
        let result = completed(fixture.cas.run(&request))?;
        let bytes = replay(&fixture, &result)?;
        Ok((bytes, result.response.ok_or("response was not exported")?))
    };
    assert_eq!(build()?, build()?);
    Ok(())
}

/// Independently construct one of the three pendulum outputs from its defining formula.
fn pendulum_row(fixture: &mut Fixture, length: &str, time: &str) -> TestResult<Handle> {
    let length = fixture.variable(length)?;
    let time = fixture.variable(time)?;
    let shared_length = fixture.variable("r")?;
    let shared_time = fixture.variable("q")?;
    let one = fixture.number("1")?;
    let four = fixture.number("4")?;
    let pi = fixture.term(Operator::Constant(Constant::Pi), &[])?;
    let pi_squared = fixture.term(Operator::IntegerPower(2), &[pi])?;
    let scale = fixture.term(Operator::Mul, &[four, pi_squared])?;
    let offset = fixture.term(Operator::Add, &[length, shared_length])?;
    let numerator = fixture.term(Operator::Mul, &[scale, offset])?;
    let period = fixture.term(Operator::IntegerPower(2), &[time])?;
    let relative = fixture.term(Operator::Add, &[one, shared_time])?;
    let timing = fixture.term(Operator::IntegerPower(2), &[relative])?;
    let denominator = fixture.term(Operator::Mul, &[period, timing])?;
    fixture.term(Operator::Div, &[numerator, denominator])
}

/// Named coordinates remain separate even when several share the same nominal rational.
fn operating_point(fixture: &mut Fixture) -> TestResult<Vec<Substitution>> {
    let length = fixture.number("621/625")?;
    let time = fixture.number("2")?;
    let zero = fixture.number("0")?;
    let mut substitutions = Vec::new();
    for (key, value) in [
        ("l1", &length),
        ("l2", &length),
        ("l3", &length),
        ("q", &zero),
        ("r", &zero),
        ("t1", &time),
        ("t2", &time),
        ("t3", &time),
    ] {
        substitutions.push(Substitution {
            key: key.into(),
            replacement: value.clone(),
        });
    }
    Ok(substitutions)
}

/// The equality-instantiation schema checks original domain requirements at the operating point.
fn check_point_domain(fixture: &mut Fixture, input: &Handle, point: &[Substitution]) -> TestResult {
    let substituted = fixture.cas.substitute(input, point, &mut budget()?)?;
    let mut instance = equality(Primitive::Substitute, &substituted, &substituted);
    instance.premises = vec![0];
    instance.substitutions = point.to_vec();
    let proof = fixture.cas.check_proof(
        &fixture.context,
        &[equality(Primitive::Reflexivity, input, input), instance],
        &mut budget()?,
    )?;
    assert!(proof.is_discharged());
    Ok(())
}

/// Three by eight exact partials preserve the shared final two columns and nominal coefficients.
/// ´test:integration:pendulum-shared-jacobian´
#[test]
fn pendulum_shared_jacobian() -> TestResult {
    let keys = ["l1", "l2", "l3", "t1", "t2", "t3", "r", "q"];
    let mut fixture = Fixture::new(&keys)?;
    let mut outputs = Vec::new();
    for (length, time) in [("l1", "t1"), ("l2", "t2"), ("l3", "t3")] {
        outputs.push(pendulum_row(&mut fixture, length, time)?);
    }
    let variables = keys.iter().map(|key| (*key).to_owned()).collect();
    let request = fixture.request(Operation::Jacobian(variables), &outputs)?;
    let result = completed(fixture.cas.run(&request))?;
    assert_eq!(
        *fixture.cas.expression(&result.witness.claim().right)?.0,
        Operator::Matrix {
            rows: 3,
            columns: 8
        }
    );
    assert_ne!(result.witness.claim().residuals, [] as [nrz_cas::Guard; 0]);
    assert!(
        result
            .witness
            .claim()
            .residuals
            .iter()
            .all(|guard| guard.kind == GuardKind::NonZero)
    );
    replay(&fixture, &result)?;
    let cells = fixture
        .cas
        .operands(&result.witness.claim().right, &mut budget()?)?;
    assert_eq!(cells.len(), 24);
    let point = operating_point(&mut fixture)?;
    for input in &outputs {
        check_point_domain(&mut fixture, input, &point)?;
    }
    let pi = fixture.term(Operator::Constant(Constant::Pi), &[])?;
    let pi_squared = fixture.term(Operator::IntegerPower(2), &[pi])?;
    let timing = fixture.number("-621/625")?;
    let common_timing = fixture.number("-1242/625")?;
    let zero = fixture.number("0")?;
    let time_cell = fixture.term(Operator::Mul, &[timing, pi_squared.clone()])?;
    let common_cell = fixture.term(Operator::Mul, &[common_timing, pi_squared.clone()])?;
    for (position, cell) in cells.iter().enumerate() {
        let row = position / 8;
        let column = position % 8;
        let expected = if column == row || column == 6 {
            &pi_squared
        } else if column == row + 3 {
            &time_cell
        } else if column == 7 {
            &common_cell
        } else {
            &zero
        };
        let specialized = fixture.cas.substitute(cell, &point, &mut budget()?)?;
        equal(&mut fixture, &specialized, expected)?;
    }
    Ok(())
}

/// A missing elementary function rule reports its exact pending expression, without a whole result.
/// ´test:integration:generated-function-remains-pending´
#[test]
fn generated_function_remains_pending() -> TestResult {
    let mut fixture = Fixture::new(&["x"])?;
    let handles = fixture.cas.admit(
        &[Draft::Context {
            environment: Environment {
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
            },
            assumptions: Vec::new(),
        }],
        &mut budget()?,
    )?;
    fixture.context = at(&handles, 0)?;
    let variable = fixture.variable("x")?;
    let call = fixture.term(Operator::Call("f".into()), &[variable])?;
    let request = fixture.request(
        Operation::Differentiate("x".into()),
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
        Operation::Differentiate("x".into()),
        std::slice::from_ref(&variable),
    )?;
    let result = completed(fixture.cas.run(&request))?;
    let bytes = fixture
        .cas
        .export_witness(&result.witness, &mut budget()?)?;
    let wrong = Claim {
        context: fixture.context.clone(),
        relation: Relation::Derivative {
            variable: "x".into(),
        },
        left: variable,
        right: zero,
        residuals: Vec::new(),
    };
    assert!(fixture.cas.replay(&bytes, &wrong, &mut budget()?).is_err());
    Ok(())
}

/// Polynomial expansion produces the independently supplied square coefficients.
/// ´test:integration:generated-polynomial-expansion´
#[test]
fn generated_polynomial_expansion() -> TestResult {
    let mut fixture = Fixture::new(&["x"])?;
    let variable = fixture.variable("x")?;
    let one = fixture.number("1")?;
    let two = fixture.number("2")?;
    let shifted = fixture.term(Operator::Add, &[variable.clone(), one.clone()])?;
    let input = fixture.term(Operator::IntegerPower(2), &[shifted])?;
    let squared = fixture.term(Operator::IntegerPower(2), std::slice::from_ref(&variable))?;
    let linear = fixture.term(Operator::Mul, &[two, variable])?;
    let tail = fixture.term(Operator::Add, &[linear, squared])?;
    let expected = fixture.term(Operator::Add, &[one, tail])?;
    let request = fixture.request(Operation::Expand, &[input])?;
    let result = completed(fixture.cas.run(&request))?;
    equal(&mut fixture, &result.witness.claim().right, &expected)?;
    replay(&fixture, &result)?;
    Ok(())
}

/// Even an empty frozen path cannot discard a nonzero domain requirement.
/// ´test:integration:frozen-domain-refusal´
#[test]
fn frozen_domain_refusal() -> TestResult {
    let mut fixture = Fixture::new(&["x"])?;
    let variable = fixture.variable("x")?;
    let quotient = fixture.term(Operator::Div, &[variable.clone(), variable])?;
    let claim = Claim {
        context: fixture.context.clone(),
        relation: Relation::EqualOnDomain,
        left: quotient.clone(),
        right: quotient,
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
