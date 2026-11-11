// SPDX-License-Identifier: AGPL-3.0-only
//! Original-operation correspondence and complete standalone sigmoid readings.
use super::*;
use nrz_matrix::{PreparedSigmoid, PreparedSigmoidError, SigmoidPreparationPolicy};
use nrz_scalar::{SigmoidFailure, SigmoidInput, SigmoidOutput};

fn policy(bits: u32) -> SigmoidPreparationPolicy {
    SigmoidPreparationPolicy {
        bits,
        requested_bits: bits.min(40) + 4,
        revision: 1,
    }
}
fn original(f: &mut Fixture, op: Operator) -> TestResult<EnclosureRequest> {
    let argument = f.term(Operator::Free("x".into()), &[])?;
    let target = f.term(op, std::slice::from_ref(&argument))?;
    let mut request = f.request(target);
    request.coordinates.push(CoordinateBound {
        coordinate: argument,
        interval: Interval::new(number(0, 1)?, number(2, 1)?, &mut ledger()?)?,
    });
    Ok(request)
}
const fn input(value: f64) -> SigmoidInput {
    SigmoidInput {
        endpoints: [value, value],
        center: value,
        bits: 40,
    }
}
fn snapshot(result: Result<SigmoidOutput, SigmoidFailure>) -> Result<[u64; 5], SigmoidFailure> {
    result.map(|r| {
        [
            r.endpoints[0].to_bits(),
            r.endpoints[1].to_bits(),
            r.center.to_bits(),
            r.total_radius.to_bits(),
            r.numerical_error.to_bits(),
        ]
    })
}

/// Operation, exact original box, target, auxiliary policy and revision refuse separately before preparation.
/// # Panics
/// Panics if a mismatched request reaches the basis producer or loses its typed cause.
/// ´claim:matrix:owner-sigmoid-correspondence´
/// ´test:integration:owner-sigmoid-correspondence´
#[test]
fn owner_sigmoid_correspondence() -> TestResult {
    let context = ArithmeticContext::new()?;
    for case in [3, 0, 4, 1, 2] {
        let mut f = Fixture::new()?;
        let r = original(
            &mut f,
            if case == 0 {
                Operator::Tanh
            } else {
                Operator::Sigmoid
            },
        )?;
        let mut p = policy(40);
        if case == 2 {
            p.bits = 41;
        }
        if case == 3 {
            p.requested_bits = 43;
        }
        if case == 4 {
            p.revision = 2;
        }
        let mut counts = Counts::default();
        let result = PreparedSigmoid::new(
            &mut f.cas,
            &r,
            if case == 1 { [0.0, 1.0] } else { [0.0, 2.0] },
            40,
            p,
            Policy::default(),
            &context,
            &mut ledger()?,
            &mut counts,
        );
        let error = result.expect_err("original correspondence before basis preparation");
        assert!(matches!(
            (case, error),
            (0, PreparedSigmoidError::Operation)
                | (1, PreparedSigmoidError::Domain)
                | (2, PreparedSigmoidError::Target)
                | (3, PreparedSigmoidError::Policy)
                | (4, PreparedSigmoidError::Revision)
        ));
        assert_eq!(counts, Counts::default());
    }
    Ok(())
}

/// Unsorted points and local refusals retain complete readings and positions across single, reused and batch evaluation.
/// # Panics
/// Panics if an input is reordered, a refusal displaced, a reading stale or native reuse spends exact Work.
/// ´claim:matrix:owner-sigmoid-reuse´
/// ´test:integration:owner-sigmoid-reuse´
#[test]
fn owner_sigmoid_reuse() -> TestResult {
    let mut f = Fixture::new()?;
    let r = original(&mut f, Operator::Sigmoid)?;
    let context = ArithmeticContext::new()?;
    let mut budget = ledger()?;
    let mut counts = Counts::default();
    let owner = PreparedSigmoid::new(
        &mut f.cas,
        &r,
        [0.0, 2.0],
        40,
        policy(40),
        Policy::default(),
        &context,
        &mut budget,
        &mut counts,
    )?;
    let mut inputs = vec![
        input(2.0),
        input(f64::from_bits(1)),
        input(0.0),
        input(3.0),
        input(2.0_f64.powi(-600)),
    ];
    for index in 0..17 {
        inputs.push(input(f64::from((index * 7 + 3) % 17 + 1) / 16.0));
    }
    let distinct: std::collections::BTreeSet<_> =
        inputs.iter().map(|value| value.center.to_bits()).collect();
    assert_eq!(distinct.len(), 22);
    let before = budget.usage();
    let before_counts = counts.clone();
    let expected: Vec<_> = inputs
        .iter()
        .map(|&x| snapshot(owner.evaluate(x, &context).map(|v| v.reading())))
        .collect();
    assert_eq!(budget.usage(), before);
    assert_eq!(counts, before_counts);
    for &x in inputs.iter().rev() {
        let mut fresh = Fixture::new()?;
        let request = original(&mut fresh, Operator::Sigmoid)?;
        let fresh_owner = PreparedSigmoid::new(
            &mut fresh.cas,
            &request,
            [0.0, 2.0],
            40,
            policy(40),
            Policy::default(),
            &context,
            &mut ledger()?,
            &mut Counts::default(),
        )?;
        assert_eq!(
            snapshot(owner.evaluate(x, &context).map(|v| v.reading())),
            snapshot(fresh_owner.evaluate(x, &context).map(|v| v.reading()))
        );
    }
    let batch = owner.evaluate_batch(&inputs, &context, &mut budget)?;
    let actual: Vec<_> = batch
        .iter()
        .map(|v| {
            snapshot(
                v.as_ref()
                    .map(nrz_matrix::SigmoidEvaluation::reading)
                    .map_err(|e| *e),
            )
        })
        .collect();
    assert_eq!(actual, expected);
    assert!(matches!(actual[1], Err(SigmoidFailure::BelowNormalOperand)));
    assert!(matches!(actual[3], Err(SigmoidFailure::OutsideDomain)));
    assert_eq!(counts, before_counts);
    Ok(())
}

/// Require an exact binary64 encoding by integer-ordered search; never round an operand.
fn exact_float(q: &Rational) -> TestResult<f64> {
    let mut b = ledger()?;
    let mut lo = 0_u64;
    let mut hi = 4.0_f64.to_bits();
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if oracles::value(mid, &mut b)?.compare(q, &mut b)? == std::cmp::Ordering::Greater {
            hi = mid - 1;
        } else {
            lo = mid;
        }
    }
    if oracles::value(lo, &mut b)? != *q {
        return Err("operand is not exactly binary64".into());
    }
    Ok(f64::from_bits(lo))
}

/// Every represented sigmoid row contains its entire golden interval and meets its own absolute target.
/// Refusal rows preserve their named cause; the nonrepresented mathematical box is not rounded into a native claim.
/// # Panics
/// Panics if containment, slack, row counts or a typed refusal changes.
/// ´claim:matrix:owner-sigmoid-golden-values´
/// ´test:integration:owner-sigmoid-golden-values´
#[test]
fn owner_sigmoid_golden_values() -> TestResult {
    let payload = nrz_docgen::golden::read(
        include_bytes!("../../../../data/golden/prepared-binary64.json").as_slice(),
        nrz_docgen::golden::Limits::default(),
        &mut ledger()?,
    )?;
    let context = ArithmeticContext::new()?;
    let mut successes = 0;
    let mut refusals = 0;
    let mut reference = 0;
    for row in payload.value()["entries"].as_array().ok_or("entries")? {
        if row["operation"] != "sigma" {
            continue;
        }
        let id = row["id"].as_str().ok_or("id")?;
        if id == "sigma-ln-two-box" {
            reference += 1;
            continue;
        }
        let bits = u32::try_from(row["target"]["bits"].as_u64().ok_or("bits")?)?;
        let mut f = Fixture::new()?;
        let request = original(&mut f, Operator::Sigmoid)?;
        let program = PreparedSigmoid::new(
            &mut f.cas,
            &request,
            [0.0, 2.0],
            bits,
            policy(bits),
            Policy::default(),
            &context,
            &mut ledger()?,
            &mut Counts::default(),
        )?;
        let texts = if let Some(point) = row["input"]["point"].as_str() {
            [point, point]
        } else {
            [
                row["input"]["box"][0].as_str().ok_or("lower")?,
                row["input"]["box"][1].as_str().ok_or("upper")?,
            ]
        };
        let mut operands = [0.0; 2];
        for (slot, text) in operands.iter_mut().zip(texts) {
            *slot = exact_float(&Rational::parse(text, &mut ledger()?)?)?;
        }
        let input = SigmoidInput {
            endpoints: operands,
            center: operands[0] + (operands[1] - operands[0]) / 2.0,
            bits,
        };
        let result = program.evaluate(input, &context).map(|v| v.reading());
        if let Some(reason) = row["expected"]["refusal"].as_str() {
            let error = result.expect_err("golden refusal");
            assert!(
                match reason {
                    "below-normal-operand" => error == SigmoidFailure::BelowNormalOperand,
                    "outside-domain" => error == SigmoidFailure::OutsideDomain,
                    "target-not-certifiable" =>
                        matches!(error, SigmoidFailure::TargetNotCertifiable { .. }),
                    _ => false,
                },
                "{id}: {error:?}"
            );
            refusals += 1;
            continue;
        }
        let output = result?;
        for (index, name) in if row["input"]["point"].is_string() {
            [(0, "point"), (1, "point")]
        } else {
            [(0, "lower_endpoint"), (1, "upper_endpoint")]
        } {
            let pair = row["golden"][name].as_array().ok_or("golden")?;
            let mut b = ledger()?;
            let lo = Rational::parse(pair[0].as_str().ok_or("low")?, &mut b)?;
            let hi = Rational::parse(pair[1].as_str().ok_or("high")?, &mut b)?;
            let endpoint = represented(output.endpoints[index], &mut b)?;
            let slack = if index == 0 {
                oracles::le(&endpoint, &lo, &mut b)?;
                hi.sub(&endpoint, &mut b)?
            } else {
                oracles::le(&hi, &endpoint, &mut b)?;
                endpoint.sub(&lo, &mut b)?
            };
            oracles::le(&slack, &power(-i64::from(bits))?, &mut b)?;
        }
        successes += 1;
    }
    assert_eq!((successes, refusals, reference), (10, 4, 1));
    Ok(())
}

/// Independent byte boundary: G plus returned scope/key, fixed native owner,
/// coefficients (32 each), six bound rationals, two per stage, and coordinate
/// inventories (256 plus two rationals each). Rational byte lengths come from
/// decimal long division, not the production estimator. Registered ledgers add
/// 64; a retained caller prefix leaves exactly S+64 or S+63 on a warm graph.
/// # Panics
/// Panics if any inventory is omitted, counted twice, or traversed before admission.
/// ´claim:matrix:owner-sigmoid-resources´
/// ´test:integration:owner-sigmoid-resources´
#[test]
fn owner_sigmoid_resources() -> TestResult {
    let mut f = Fixture::new()?;
    let r = original(&mut f, Operator::Sigmoid)?;
    let context = ArithmeticContext::new()?;
    drop(
        PreparedSigmoid::new(
            &mut f.cas,
            &r,
            [0.0, 2.0],
            40,
            policy(40),
            Policy::default(),
            &context,
            &mut ledger()?,
            &mut Counts::default(),
        )
        .map_err(|e| e.to_string())?,
    );
    let graph = f.cas.counts().payload_bytes;
    let judgments = f.cas.retained_judgment_bytes();
    let inputs = [input(0.25), input(0.0), input(1.0)];
    let output = 3 * size_of::<Result<nrz_matrix::SigmoidEvaluation<'_>, SigmoidFailure>>() as u64;
    let cells = size_of_val(&inputs) as u64 + output;
    for reused in [false, true] {
        for short in [0, 1] {
            let mut preparation = ledger()?;
            let plan = PreparedSigmoid::new(
                &mut f.cas,
                &r,
                [0.0, 2.0],
                40,
                policy(40),
                Policy::default(),
                &context,
                &mut preparation,
                &mut Counts::default(),
            )
            .map_err(|e| e.to_string())?;
            let owned = sigmoid_owned(&plan)?;
            let peak = graph + owned + cells + u64::from(reused) * 64;
            let mut fresh = ledger()?;
            let b = if reused { &mut preparation } else { &mut fresh };
            b.work(7, nrz_bindings::readings::algebra::Location::new("caller"))?;
            let before = b.usage().work;
            let prefix = b.limits().staged_payload_bytes - peak + short;
            let result = b.with_retained_payload(
                prefix,
                nrz_bindings::readings::algebra::Location::new("caller"),
                |b| plan.evaluate_batch(&inputs, &context, b),
            );
            if short == 0 {
                let values = result?;
                assert_eq!(values.len(), 3);
                for (value, input) in values.into_iter().zip(inputs) {
                    assert_eq!(
                        value?.reading().endpoints,
                        plan.evaluate(input, &context)?.reading().endpoints
                    );
                }
                assert_eq!(b.usage().work, before + 3 + u64::from(reused));
            } else {
                let needed = peak - if reused { graph - judgments } else { graph };
                assert_eq!(
                    result.expect_err("payload").kind,
                    FaultKind::Exhausted {
                        reason: nrz_bindings::readings::algebra::Exhaustion::Limit,
                        budget: BudgetKind::StagedPayloadBytes,
                        limit: needed - 1,
                        needed,
                    }
                );
                assert_eq!(b.usage().work, before + u64::from(reused));
            }
            ceilings(&plan, &inputs, &context, output)?;
        }
    }
    Ok(())
}

/// Exact independent cell tariffs with a spent prefix survive each typed refusal.
/// # Panics
/// Panics if limits or consumption differ from the declared inventory costs.
fn ceilings(
    plan: &PreparedSigmoid<'_>,
    inputs: &[SigmoidInput],
    context: &ArithmeticContext,
    output: u64,
) -> TestResult {
    for (kind, ceiling) in [
        (BudgetKind::Work, 10),
        (BudgetKind::MatrixEntries, 3),
        (BudgetKind::OutputBytes, output),
    ] {
        for limit in [ceiling, ceiling - 1] {
            let mut limits = ledger()?.limits().clone();
            match kind {
                BudgetKind::Work => limits.work = limit,
                BudgetKind::MatrixEntries => limits.matrix_entries = limit,
                BudgetKind::OutputBytes => limits.output_bytes = limit,
                _ => unreachable!(),
            }
            let mut b = Budget::new(limits)?;
            b.work(7, nrz_bindings::readings::algebra::Location::new("caller"))?;
            let result = plan.evaluate_batch(inputs, context, &mut b);
            if limit == ceiling {
                assert_eq!(result?.len(), 3);
                assert_eq!(b.usage().work, 10);
            } else {
                assert_eq!(
                    result.expect_err("ceiling").kind,
                    FaultKind::Exhausted {
                        reason: nrz_bindings::readings::algebra::Exhaustion::Limit,
                        budget: kind,
                        limit,
                        needed: ceiling
                    }
                );
                assert_eq!(b.usage().work, 7);
            }
        }
    }
    Ok(())
}

/// Inventory derivation stated beside the resource control, independent of production estimators.
fn sigmoid_owned(plan: &PreparedSigmoid<'_>) -> TestResult<u64> {
    let authority = plan.basis();
    let proof = authority.proof();
    let mut owned = size_of::<PreparedSigmoid<'_>>() as u64
        + size_of::<EnclosureRequest>() as u64
        + 1600
        + proof.scope.len() as u64
        + authority.statement_key().len() as u64
        + 1024
        + 32 * proof.coefficients.len() as u64
        + 256 * authority.request().original.coordinates.len() as u64;
    for q in [
        &proof.absolute_tail,
        &proof.relative_tail,
        &proof.import_ratio,
        &proof.central_ratio,
        &proof.endpoint_ratio,
        &proof.restoration_error,
    ]
    .into_iter()
    .chain(proof.stages.iter().flat_map(|s| [&s.magnitude, &s.error]))
    .chain(
        authority
            .request()
            .original
            .coordinates
            .iter()
            .flat_map(|c| [c.interval.lower(), c.interval.upper()]),
    ) {
        let text = q.exact_string(&mut ledger()?)?;
        let (numerator, denominator) = text.split_once('/').unwrap_or((&text, "1"));
        owned += 128 + 4 * (decimal_bytes(numerator) + decimal_bytes(denominator));
    }
    Ok(owned)
}

/// Fresh-store independent replay preserves the auxiliary claim, while complete native snapshots leave all ledger counters unchanged.
/// # Panics
/// Panics if replay changes the claim or evaluation consumes exact resources.
/// ´claim:matrix:owner-sigmoid-replay-and-accounting´
/// ´test:integration:owner-sigmoid-replay-and-accounting´
#[test]
fn owner_sigmoid_replay_and_accounting() -> TestResult {
    use std::io::Write as _;
    let mut f = Fixture::new()?;
    let r = original(&mut f, Operator::Sigmoid)?;
    let context = ArithmeticContext::new()?;
    let mut budget = ledger()?;
    let mut counts = Counts::default();
    let before = budget.usage();
    let plan = PreparedSigmoid::new(
        &mut f.cas,
        &r,
        [0.0, 2.0],
        40,
        policy(40),
        Policy::default(),
        &context,
        &mut budget,
        &mut counts,
    )?;
    let prepared = budget.usage();
    let proof = plan.basis().proof().clone();
    let statement = plan.basis().statement_key().to_vec();
    let owned = sigmoid_owned(&plan)?;
    let expected = snapshot(plan.evaluate(input(1.0), &context).map(|v| v.reading()));
    assert!(expected.is_ok());
    assert_eq!(budget.usage(), prepared, "all native counters unchanged");
    writeln!(
        std::io::stdout(),
        "SIGMOID_PREPARATION before={before:?} after={prepared:?} counts={counts:?} owned={owned}"
    )?;
    drop(plan);
    let mut warm = ledger()?;
    writeln!(
        std::io::stdout(),
        "SIGMOID_INVENTORY graph={} judgments={} input={} output={}",
        f.cas.counts().payload_bytes,
        f.cas.retained_judgment_bytes(),
        size_of::<SigmoidInput>(),
        size_of::<Result<nrz_matrix::SigmoidEvaluation<'_>, SigmoidFailure>>()
    )?;
    let mut warm_counts = Counts::default();
    let plan = PreparedSigmoid::new(
        &mut f.cas,
        &r,
        [0.0, 2.0],
        40,
        policy(40),
        Policy::default(),
        &context,
        &mut warm,
        &mut warm_counts,
    )?;
    assert_eq!(warm_counts.proposals, 0);
    assert_eq!(
        snapshot(plan.evaluate(input(1.0), &context).map(|v| v.reading())),
        expected
    );
    let warm_before = warm.usage();
    let values = plan.evaluate_batch(&[input(1.0), input(0.0), input(2.0)], &context, &mut warm)?;
    assert_eq!(values.len(), 3);
    assert_eq!(warm.usage().work - warm_before.work, 4);
    writeln!(
        std::io::stdout(),
        "SIGMOID_WARM before_batch={warm_before:?} after_batch={:?} counts={warm_counts:?}",
        warm.usage()
    )?;
    let mut receiver = Fixture::new()?;
    let auxiliary = broad(&mut receiver, 44)?;
    let mut replay = ledger()?;
    let mut replay_counts = Counts::default();
    let checked = receiver.cas.check_expm1(
        &auxiliary,
        Policy::default(),
        &proof,
        &mut replay,
        &mut replay_counts,
    )?;
    assert_eq!(checked.statement_key(), statement);
    assert_eq!(replay_counts.checks, 1);
    writeln!(
        std::io::stdout(),
        "SIGMOID_REPLAY usage={:?} counts={replay_counts:?}",
        replay.usage()
    )?;
    Ok(())
}

/// An unavailable basis retains the actual transformed request, precision policy and finite search without an impossibility claim.
/// # Panics
/// Panics if finite search failure loses its request or policy.
/// ´claim:matrix:owner-sigmoid-basis-failure´
/// ´test:integration:owner-sigmoid-basis-failure´
#[test]
fn owner_sigmoid_basis_failure() -> TestResult {
    let context = ArithmeticContext::new()?;
    let mut unavailable = Fixture::new()?;
    let request = original(&mut unavailable, Operator::Sigmoid)?;
    let failure = PreparedSigmoid::new(
        &mut unavailable.cas,
        &request,
        [0.0, 2.0],
        40,
        policy(40),
        Policy {
            degree: 1,
            attempts: 1,
            ..Policy::default()
        },
        &context,
        &mut ledger()?,
        &mut Counts::default(),
    )
    .expect_err("finite search unavailable");
    let PreparedSigmoidError::BasisPreparation {
        auxiliary,
        policy: actual,
        search,
    } = failure
    else {
        panic!("basis failure distinct");
    };
    assert_eq!(actual, policy(40));
    assert_eq!((search.degree, search.attempts), (1, 1));
    assert_eq!(auxiliary.bits, 44);
    assert_eq!(auxiliary.environment, 1);
    assert_eq!(
        auxiliary.original.coordinates[0].interval,
        Interval::new(number(-2, 1)?, Rational::zero(), &mut ledger()?)?
    );
    Ok(())
}

/// A direct constant is negated exactly; validated zero and nonzero nominal readings agree with the corresponding coordinate owner.
/// # Panics
/// Panics if constant correspondence or complete readings differ from the coordinate route.
/// ´claim:matrix:owner-sigmoid-constant´
/// ´test:integration:owner-sigmoid-constant´
#[test]
fn owner_sigmoid_constant() -> TestResult {
    let context = ArithmeticContext::new()?;
    for x in [1.0, 0.0, 2.0] {
        let mut f = Fixture::new()?;
        let argument = f.term(Operator::Rational(represented(x, &mut ledger()?)?), &[])?;
        let target = f.term(Operator::Sigmoid, &[argument])?;
        let request = f.request(target);
        let plan = PreparedSigmoid::new(
            &mut f.cas,
            &request,
            [x, x],
            40,
            policy(40),
            Policy::default(),
            &context,
            &mut ledger()?,
            &mut Counts::default(),
        )?;
        let result = snapshot(plan.evaluate(input(x), &context).map(|v| v.reading()));
        let mut other = Fixture::new()?;
        let r = original(&mut other, Operator::Sigmoid)?;
        let full = PreparedSigmoid::new(
            &mut other.cas,
            &r,
            [0.0, 2.0],
            40,
            policy(40),
            Policy::default(),
            &context,
            &mut ledger()?,
            &mut Counts::default(),
        )?;
        assert_eq!(
            result,
            snapshot(full.evaluate(input(x), &context).map(|v| v.reading()))
        );
    }
    Ok(())
}

/// Warm constructor output admits the larger of the returned-owner inventory and retained judgment, refusing one below without refunding the caller prefix.
/// # Panics
/// Panics if returned ownership omits bytes or a refusal changes its resource cause.
/// ´claim:matrix:owner-sigmoid-constructor-output´
/// ´test:integration:owner-sigmoid-constructor-output´
#[test]
fn owner_sigmoid_constructor_output() -> TestResult {
    let mut f = Fixture::new()?;
    let r = original(&mut f, Operator::Sigmoid)?;
    let context = ArithmeticContext::new()?;
    let plan = PreparedSigmoid::new(
        &mut f.cas,
        &r,
        [0.0, 2.0],
        40,
        policy(40),
        Policy::default(),
        &context,
        &mut ledger()?,
        &mut Counts::default(),
    )?;
    let owned = sigmoid_owned(&plan)?;
    drop(plan);
    let needed = owned.max(f.cas.retained_judgment_bytes());
    for limit in [needed, needed - 1] {
        let mut limits = ledger()?.limits().clone();
        limits.output_bytes = limit;
        let mut budget = Budget::new(limits)?;
        budget.work(7, nrz_bindings::readings::algebra::Location::new("caller"))?;
        let result = PreparedSigmoid::new(
            &mut f.cas,
            &r,
            [0.0, 2.0],
            40,
            policy(40),
            Policy::default(),
            &context,
            &mut budget,
            &mut Counts::default(),
        );
        if limit == needed {
            drop(result?);
        } else {
            let fault = match result.expect_err("complete returned ownership") {
                PreparedSigmoidError::Exact(fault)
                | PreparedSigmoidError::Admission(PreparedExpm1Error::Exact(fault)) => fault,
                other => panic!("typed output cause: {other:?}"),
            };
            assert_eq!(
                fault.kind,
                FaultKind::Exhausted {
                    reason: nrz_bindings::readings::algebra::Exhaustion::Limit,
                    budget: BudgetKind::OutputBytes,
                    limit,
                    needed
                }
            );
        }
        assert!(budget.usage().work > 7);
    }
    Ok(())
}
