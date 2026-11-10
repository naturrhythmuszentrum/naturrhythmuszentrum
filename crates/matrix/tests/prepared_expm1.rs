// SPDX-License-Identifier: AGPL-3.0-only
//! Public native admission, independent endpoint oracles and finite resource controls.
use nrz_bindings::readings::algebra::domain::{
    AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr,
};
use nrz_bindings::readings::algebra::enclosure::prepared_expm1::{Counts, Policy, Request};
use nrz_bindings::readings::algebra::enclosure::{
    CoordinateBound, EnclosureGoal, EnclosureRequest, Interval,
};
use nrz_bindings::readings::algebra::limits::{Budget, BudgetKind, Limits};
use nrz_bindings::readings::algebra::syntax::{
    AtomicSort, Declaration, Environment, Field, Operator, Sort,
};
use nrz_bindings::readings::algebra::syntax::{Draft, ExpressionDraft};
use nrz_bindings::readings::algebra::{Cas, Fault, FaultKind, Handle, Rational, Reference};
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
fn ledger() -> Result<Budget, Fault> {
    Budget::new(Limits {
        work: 2_000_000_000,
        integer_exponent: 4096,
        arithmetic_steps: 50_000_000,
        name_bytes: 131_072,
        literal_bytes: 618,
        source_bytes: 4_194_304,
        output_bytes: 4_194_304,
        witness_bytes: 4_194_304,
        ..Limits::default()
    })
}
fn number(n: i64, d: u64) -> Result<Rational, Fault> {
    Rational::from_ratio(n, d, &mut ledger()?)
}
fn power(e: i64) -> Result<Rational, Fault> {
    number(2, 1)?.pow(e, &mut ledger()?)
}
struct Fixture {
    cas: Cas,
    header: Handle,
    context: Handle,
}
impl Fixture {
    fn new() -> TestResult<Self> {
        Self::with_names(&["x"])
    }
    fn with_names(names: &[&str]) -> TestResult<Self> {
        Self::with_names_and_limits(
            names,
            Limits {
                work: 2_000_000_000,
                integer_exponent: 4096,
                arithmetic_steps: 50_000_000,
                ..Limits::default()
            },
        )
    }
    fn with_names_and_limits(names: &[&str], limits: Limits) -> TestResult<Self> {
        let mut environment = Environment::default();
        for name in names {
            environment.declarations.push(Declaration {
                key: (*name).into(),
                display_name: (*name).into(),
                sort: real(),
            });
        }
        let mut cas = Cas::new(limits)?;
        let drafts = [
            Draft::Header(HeaderDraft::Core(environment)),
            Draft::Region(RegionDraft {
                header: Reference::Local(0),
                scope: Vec::new(),
                ambient: AmbientDomain::Assignments,
                expression: RegionExpr::Ambient,
            }),
            Draft::Context(ContextDraft::Root {
                header: Reference::Local(0),
                admissible: Reference::Local(1),
                assumptions: Vec::new(),
            }),
        ];
        let [header, _, context]: [Handle; 3] = cas
            .admit(&drafts, &mut ledger()?)?
            .try_into()
            .map_err(|_| "fixture")?;
        Ok(Self {
            cas,
            header,
            context,
        })
    }
    fn term(&mut self, operator: Operator, operands: &[Handle]) -> TestResult<Handle> {
        let draft = Draft::Expression(ExpressionDraft {
            space: None,
            operator,
            sort: real(),
            header: Reference::Existing(self.header.clone()),
            scope: Vec::new(),
            binding: None,
            operands: operands.iter().cloned().map(Reference::Existing).collect(),
        });
        self.cas
            .admit(&[draft], &mut ledger()?)?
            .into_iter()
            .next()
            .ok_or_else(|| "term".into())
    }

    fn request(&self, target: Handle) -> EnclosureRequest {
        EnclosureRequest {
            target,
            context: self.context.clone(),
            coordinates: Vec::new(),
            goal: EnclosureGoal::Width,
        }
    }
}
const fn real() -> Sort {
    Sort::Value(AtomicSort::Scalar(Field::Real))
}

fn request(f: &mut Fixture, value: Rational, bits: u32) -> TestResult<Request> {
    let argument = f.term(Operator::Rational(value), &[])?;
    expression(f, argument, bits)
}
fn expression(f: &mut Fixture, argument: Handle, bits: u32) -> TestResult<Request> {
    let e = f.term(Operator::Exponential, &[argument])?;
    let one = f.term(Operator::Rational(Rational::one()), &[])?;
    let target = f.term(Operator::Sub, &[e, one])?;
    Ok(Request {
        original: f.request(target),
        bits,
        environment: 1,
    })
}

use nrz_matrix::{PreparedExpm1, PreparedExpm1Error};
use nrz_scalar::{ArithmeticContext, Expm1Failure, Expm1Input};
#[path = "prepared_expm1/application.rs"]
mod application;
#[path = "prepared_expm1/exp.rs"]
mod exp;
#[expect(
    clippy::redundant_pub_crate,
    reason = "independent oracle helpers remain private to this test target"
)]
#[path = "prepared_expm1/oracles.rs"]
mod oracles;
#[path = "prepared_expm1/retained.rs"]
mod retained;

/// Read the validated payload structurally, retaining each row's associations.
/// # Errors
/// Returns admission, exact arithmetic or preparation failures.
/// # Panics
/// Panics if the three distinct expm1 rows lose identity, target or containment.
/// ´claim:matrix:prepared-expm1-validated-fixture´
/// ´test:integration:prepared-expm1-validated-fixture´
#[test]
fn prepared_expm1_validated_fixture() -> TestResult {
    check_fixture(include_bytes!(
        "../../../data/golden/prepared-binary64.json"
    ))
}

/// Moving M1's operation member after its evidence cannot select another row's input.
/// # Errors
/// Returns payload or native admission failures.
/// # Panics
/// Panics if reordering changes the checked value or row associations.
/// ´claim:matrix:prepared-expm1-fixture-member-order´
/// ´test:integration:prepared-expm1-fixture-member-order´
#[test]
fn prepared_expm1_fixture_member_order() -> TestResult {
    let bytes = include_bytes!("../../../data/golden/prepared-binary64.json");
    let payload = nrz_docgen::golden::read(
        bytes.as_slice(),
        nrz_docgen::golden::Limits::default(),
        &mut ledger()?,
    )?;
    // This textual edit changes only the control fixture, never how rows are read.
    let source = std::str::from_utf8(bytes)?;
    let begin = source.find("\"id\": \"M1\"").ok_or("M1 member")?;
    let end = begin + source[begin..].find("\n    }").ok_or("M1 close")?;
    let original = &source[begin..end];
    let member = "      \"operation\": \"expm1\",\n";
    assert_eq!(original.matches(member).count(), 1);
    let mut reordered = original.replacen(member, "", 1);
    reordered.push_str(",\n      \"operation\": \"expm1\"");
    let changed = source.replacen(original, &reordered, 1);
    let admitted = nrz_docgen::golden::read(
        changed.as_bytes(),
        nrz_docgen::golden::Limits::default(),
        &mut ledger()?,
    )?;
    assert_eq!(admitted.value(), payload.value());
    check_fixture(changed.as_bytes())
}

/// Use only checked structured fields, with the same object supplying every operand.
/// # Panics
/// Panics if fixture identity, input, target or endpoint containment changes.
fn check_fixture(bytes: &[u8]) -> TestResult {
    let payload =
        nrz_docgen::golden::read(bytes, nrz_docgen::golden::Limits::default(), &mut ledger()?)?;
    let mut f = Fixture::new()?;
    let r = broad(&mut f, 40)?;
    let context = ArithmeticContext::new()?;
    let plan = PreparedExpm1::new(
        &mut f.cas,
        &r,
        [-2.0, 0.0],
        Policy::default(),
        &context,
        &mut ledger()?,
        &mut Counts::default(),
    )?;
    let mut identities = Vec::new();
    for row in payload.value()["entries"].as_array().ok_or("entries")? {
        if row["operation"] != "expm1" {
            continue;
        }
        let id = row["id"].as_str().ok_or("id")?;
        let input = row["input"]["point"].as_str().ok_or("point")?;
        let expected = match id {
            "M1" => "-1/1024",
            "M2" => "-1/1099511627776",
            "M0" => "0",
            _ => return Err("unexpected expm1 row".into()),
        };
        assert_eq!(input, expected);
        assert_eq!(row["target"]["kind"], "relative");
        assert_eq!(row["target"]["bits"], 40);
        let bits = u32::try_from(row["target"]["bits"].as_u64().ok_or("bits")?)?;
        let (top, bottom) = input.split_once('/').unwrap_or((input, "1"));
        let x = top.parse::<f64>()? / bottom.parse::<f64>()?;
        assert_eq!(
            represented(x, &mut ledger()?)?,
            Rational::parse(input, &mut ledger()?)?
        );
        let golden = row["golden"]["point"].as_array().ok_or("point golden")?;
        let low = Rational::parse(golden[0].as_str().ok_or("lower")?, &mut ledger()?)?;
        let high = Rational::parse(golden[1].as_str().ok_or("upper")?, &mut ledger()?)?;
        let result = plan
            .evaluate(Expm1Input { bits, ..point(x) }, &context)?
            .reading();
        oracles::le(
            &represented(result.endpoints[0], &mut ledger()?)?,
            &low,
            &mut ledger()?,
        )?;
        oracles::le(
            &high,
            &represented(result.endpoints[1], &mut ledger()?)?,
            &mut ledger()?,
        )?;
        identities.push(id);
    }
    assert_eq!(identities, ["M1", "M2", "M0"]);
    Ok(())
}

fn broad(f: &mut Fixture, bits: u32) -> TestResult<Request> {
    let x = f.term(Operator::Free("x".into()), &[])?;
    let mut r = expression(f, x.clone(), bits)?;
    r.original.coordinates.push(CoordinateBound {
        coordinate: x,
        interval: Interval::new(number(-2, 1)?, Rational::zero(), &mut ledger()?)?,
    });
    Ok(r)
}

fn represented(x: f64, b: &mut Budget) -> TestResult<Rational> {
    let q = oracles::value(x.abs().to_bits(), b)?;
    Ok(if x.is_sign_negative() {
        q.negated(b)?
    } else {
        q
    })
}

const fn point(x: f64) -> Expm1Input {
    Expm1Input {
        endpoints: [x, x],
        center: x,
        bits: 40,
    }
}

/// Independent alternating sums and exact rounding certify endpoints and nominal readings.
/// # Errors
/// Returns preparation or bounded exact oracle failures.
/// # Panics
/// Panics if any native enclosure, target or nominal error misses the independent oracle.
/// ´claim:matrix:prepared-expm1-native-oracles´
/// ´test:integration:prepared-expm1-native-oracles´
#[test]
fn prepared_expm1_native_oracles() -> TestResult {
    let mut f = Fixture::new()?;
    let request = broad(&mut f, 40)?;
    let context = ArithmeticContext::new()?;
    let mut counts = Counts::default();
    let plan = PreparedExpm1::new(
        &mut f.cas,
        &request,
        [-2.0, 0.0],
        Policy::default(),
        &context,
        &mut ledger()?,
        &mut counts,
    )?;
    let mut budget = Budget::new(Limits {
        work: 40_000_000_000,
        arithmetic_steps: 100_000_000,
        component_bits: 32_768,
        intermediate_bits: 131_072,
        integer_exponent: 16_384,
        ..Limits::default()
    })?;
    // M1/M2/M0 are expm1 inputs; E1/E2/E3 in the accepted sample are exp.
    let mut inputs = vec![
        -2.0_f64.powi(-40),
        -2.0,
        0.0,
        -f64::MIN_POSITIVE,
        -2.0_f64.powi(-10),
        -1.0,
        -f64::MIN_POSITIVE.next_up(),
        (-2.0_f64).next_up(),
    ];
    // Odd multiplication permutes 64 distinct dyadics; no pre-sorting reaches the kernel.
    inputs.extend((0..64).map(|i| -f64::from((i * 37) % 64 + 1) / 64.0));
    let mut distinct = inputs
        .iter()
        .map(|x| x.to_bits())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(distinct.len(), 71);
    distinct.clear();
    let mut boxes: Vec<_> = inputs.into_iter().map(point).collect();
    boxes.extend([
        Expm1Input {
            endpoints: [-2.0, 0.0],
            center: -1.0,
            bits: 40,
        },
        Expm1Input {
            endpoints: [-f64::MIN_POSITIVE, 0.0],
            center: -f64::MIN_POSITIVE / 2.0,
            bits: 40,
        },
        Expm1Input {
            endpoints: [-0.75, -0.25],
            center: -0.375,
            bits: 40,
        },
    ]);
    for input in boxes {
        check_oracle(&plan, input, &context, &mut budget)?;
    }
    Ok(())
}

/// Validation precedes zero shortcuts and isolates original endpoint refusal causes.
/// # Errors
/// Returns preparation failures.
/// # Panics
/// Panics if a domain, target, normal threshold or exact-zero contract changes.
/// ´claim:matrix:prepared-expm1-native-refusals´
/// ´test:integration:prepared-expm1-native-refusals´
#[test]
fn prepared_expm1_native_refusals() -> TestResult {
    let mut f = Fixture::new()?;
    let r = broad(&mut f, 40)?;
    let context = ArithmeticContext::new()?;
    let plan = PreparedExpm1::new(
        &mut f.cas,
        &r,
        [-2.0, 0.0],
        Policy::default(),
        &context,
        &mut ledger()?,
        &mut Counts::default(),
    )?;
    for (input, cause) in [
        (
            point(-f64::MIN_POSITIVE.next_down()),
            Expm1Failure::BelowNormalOperand,
        ),
        (point(-f64::from_bits(1)), Expm1Failure::BelowNormalOperand),
        (point(f64::from_bits(1)), Expm1Failure::OutsideDomain),
        (point((-2.0_f64).next_down()), Expm1Failure::OutsideDomain),
        (point(f64::NAN), Expm1Failure::OutsideDomain),
        (
            Expm1Input {
                bits: 41,
                ..point(0.0)
            },
            Expm1Failure::Target,
        ),
        (
            Expm1Input {
                endpoints: [-1.0, 0.0],
                center: -1.5,
                bits: 40,
            },
            Expm1Failure::OutsideDomain,
        ),
        (
            Expm1Input {
                endpoints: [0.0, -1.0],
                center: -0.5,
                bits: 40,
            },
            Expm1Failure::OutsideDomain,
        ),
        (
            Expm1Input {
                endpoints: [-f64::MIN_POSITIVE, f64::MIN_POSITIVE],
                center: 0.0,
                bits: 40,
            },
            Expm1Failure::OutsideDomain,
        ),
    ] {
        assert_eq!(
            plan.evaluate(input, &context)
                .expect_err("isolated refusal"),
            cause
        );
    }
    let zero = plan.evaluate(point(-0.0), &context)?.reading();
    assert_eq!(zero.endpoints.map(f64::to_bits), [0, 0]);
    assert_eq!(
        [zero.center, zero.total_radius, zero.numerical_error].map(f64::to_bits),
        [0, 0, 0]
    );
    plan.evaluate(point(-f64::MIN_POSITIVE), &context)?;
    Ok(())
}

/// Native reuse retains construction counts and separately checks the rounded nominal program.
/// # Errors
/// Returns exact preparation or native failures.
/// # Panics
/// Panics if native applications reconstruct authority or change the executed separate schedule.
/// ´claim:matrix:prepared-expm1-native-reuse´
/// ´test:integration:prepared-expm1-native-reuse´
#[test]
fn prepared_expm1_native_reuse() -> TestResult {
    let mut f = Fixture::new()?;
    let r = broad(&mut f, 40)?;
    let context = ArithmeticContext::new()?;
    let mut counts = Counts::default();
    let mut budget = ledger()?;
    let plan = PreparedExpm1::new(
        &mut f.cas,
        &r,
        [-2.0, 0.0],
        Policy::default(),
        &context,
        &mut budget,
        &mut counts,
    )?;
    let before = counts.clone();
    let work = budget.usage().work;
    for x in [-0.25, -1.0, -0.75, -0.5] {
        let evaluated = plan.evaluate(point(x), &context)?;
        assert!(std::ptr::eq(evaluated.authority(), plan.authority()));
        let result = evaluated.reading();
        let proof = plan.authority().proof();
        let mut expected = 0.0;
        for c in proof.coefficients.iter().rev() {
            let parsed = c.exact(&mut ledger()?)?.exact_string(&mut ledger()?)?;
            let pieces = parsed.split('/').collect::<Vec<_>>();
            let coefficient =
                pieces[0].parse::<f64>()? / pieces.get(1).unwrap_or(&"1").parse::<f64>()?;
            expected *= x;
            expected += coefficient;
        }
        expected *= x;
        // Schedule regression comparison, not the independent truth oracle above.
        assert_eq!(result.center.to_bits(), expected.to_bits());
    }
    assert_eq!(counts, before);
    assert_eq!(budget.usage().work, work);
    Ok(())
}

/// Count, output, live payload and Work limits refuse before walking a batch.
/// # Errors
/// Returns preparation or budget construction failures.
/// # Panics
/// Panics if a boundary loses its typed cause or earlier consumption.
/// ´claim:matrix:prepared-expm1-native-resources´
/// ´test:integration:prepared-expm1-native-resources´
#[test]
fn prepared_expm1_native_resources() -> TestResult {
    let mut f = Fixture::new()?;
    let r = broad(&mut f, 40)?;
    let context = ArithmeticContext::new()?;
    let plan = PreparedExpm1::new(
        &mut f.cas,
        &r,
        [-2.0, 0.0],
        Policy::default(),
        &context,
        &mut ledger()?,
        &mut Counts::default(),
    )?;
    let owned = fixture_owned(&plan)?;
    drop(plan);
    let graph = f.cas.counts().payload_bytes;
    let plan = PreparedExpm1::new(
        &mut f.cas,
        &r,
        [-2.0, 0.0],
        Policy::default(),
        &context,
        &mut ledger()?,
        &mut Counts::default(),
    )?;
    assert_eq!(plan.owned_bytes(), owned);
    let inputs = [point(-0.25), point(0.0), point(-1.0)];
    let output = (3 * size_of::<Result<nrz_matrix::Expm1Evaluation<'_>, Expm1Failure>>()) as u64;
    let peak = graph + owned + output + (3 * size_of::<Expm1Input>()) as u64;
    for (kind, ceiling) in [
        (BudgetKind::MatrixEntries, 3),
        (BudgetKind::OutputBytes, output),
        (BudgetKind::Work, 10),
        (BudgetKind::StagedPayloadBytes, peak),
    ] {
        for cap in [ceiling, ceiling - 1] {
            let mut limits = ledger()?.limits().clone();
            match kind {
                BudgetKind::MatrixEntries => limits.matrix_entries = cap,
                BudgetKind::OutputBytes => limits.output_bytes = cap,
                BudgetKind::Work => limits.work = cap,
                BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = cap,
                _ => return Err("unexpected capacity".into()),
            }
            let mut b = Budget::new(limits)?;
            b.work(
                7,
                nrz_bindings::readings::algebra::Location::new("caller prefix"),
            )?;
            let result = plan.evaluate_batch(&inputs, &context, &mut b);
            if cap == ceiling {
                assert_eq!(result?.len(), 3);
                assert_eq!(b.usage().work, 10);
            } else {
                assert_eq!(
                    result.expect_err("one below").kind,
                    FaultKind::Exhausted {
                        reason: nrz_bindings::readings::algebra::Exhaustion::Limit,
                        budget: kind,
                        limit: if kind == BudgetKind::StagedPayloadBytes {
                            cap - graph
                        } else {
                            cap
                        },
                        needed: if kind == BudgetKind::StagedPayloadBytes {
                            ceiling - graph
                        } else {
                            ceiling
                        },
                    }
                );
                assert_eq!(b.usage().work, 7);
            }
        }
    }
    Ok(())
}

/// The bridge uses its returned authority and refuses a mismatched represented subdomain.
/// # Errors
/// Returns fixture or arithmetic failures.
/// # Panics
/// Panics if admission broadens the original claim or conflates bounded target failure.
/// ´claim:matrix:prepared-expm1-admission´
/// ´test:integration:prepared-expm1-admission´
#[test]
fn prepared_expm1_admission() -> TestResult {
    let context = ArithmeticContext::new()?;
    coordinate_admission(&context)?;
    let mut f = Fixture::new()?;
    let r = request(&mut f, number(-1, 1)?, 40)?;
    assert!(matches!(
        PreparedExpm1::new(
            &mut f.cas,
            &r,
            [-2.0, 0.0],
            Policy::default(),
            &context,
            &mut ledger()?,
            &mut Counts::default()
        ),
        Err(PreparedExpm1Error::Domain)
    ));
    let r = broad(&mut f, 60)?;
    assert!(matches!(
        PreparedExpm1::new(
            &mut f.cas,
            &r,
            [-2.0, 0.0],
            Policy::default(),
            &context,
            &mut ledger()?,
            &mut Counts::default()
        ),
        Err(PreparedExpm1Error::TargetNotCertifiable)
    ));
    let r = request(&mut f, Rational::zero(), 60)?;
    let plan = PreparedExpm1::new(
        &mut f.cas,
        &r,
        [0.0, 0.0],
        Policy::default(),
        &context,
        &mut ledger()?,
        &mut Counts::default(),
    )?;
    let output = plan
        .evaluate(
            Expm1Input {
                bits: 60,
                ..point(0.0)
            },
            &context,
        )?
        .reading();
    assert_eq!(output.endpoints.map(f64::to_bits), [0, 0]);
    Ok(())
}

fn coordinate_admission(context: &ArithmeticContext) -> TestResult {
    let mut f = Fixture::new()?;
    let mut r = broad(&mut f, 40)?;
    r.original.coordinates[0].interval =
        Interval::new(number(-1, 1)?, number(-1, 4)?, &mut ledger()?)?;
    for domain in [[-0.5, 0.0], [-2.0, -0.5]] {
        assert!(matches!(
            PreparedExpm1::new(
                &mut f.cas,
                &r,
                domain,
                Policy::default(),
                context,
                &mut ledger()?,
                &mut Counts::default()
            ),
            Err(PreparedExpm1Error::Domain)
        ));
    }
    let plan = PreparedExpm1::new(
        &mut f.cas,
        &r,
        [-0.75, -0.5],
        Policy::default(),
        context,
        &mut ledger()?,
        &mut Counts::default(),
    )?;
    plan.evaluate(point(-0.5), context)?;
    Ok(())
}

/// A large original scope gives an ownership lower bound independent of the byte estimator.
/// # Errors
/// Returns bounded preparation or declaration failures.
/// # Panics
/// Panics if the retained proof scope and statement-key scope are not both accounted.
/// ´claim:matrix:prepared-expm1-owned-scope´
/// ´test:integration:prepared-expm1-owned-scope´
#[test]
fn prepared_expm1_owned_scope() -> TestResult {
    let unused = "u".repeat(65_536);
    let mut f = Fixture::with_names_and_limits(&["x", &unused], ledger()?.limits().clone())?;
    let request = request(&mut f, Rational::zero(), 40)?;
    let context = ArithmeticContext::new()?;
    let plan = PreparedExpm1::new(
        &mut f.cas,
        &request,
        [0.0, 0.0],
        Policy::default(),
        &context,
        &mut ledger()?,
        &mut Counts::default(),
    )?;
    let scope = plan.authority().proof().scope.len() as u64;
    assert!(scope >= 65_536);
    // Two physically owned byte vectors each contain the complete scope; the
    // native fixed array adds 128 binary64 coefficients regardless of degree.
    assert!(plan.owned_bytes() >= 2 * scope + 128 * 8);
    Ok(())
}

/// Independent endpoint, nominal and exact-rounded-schedule comparisons.
/// # Panics
/// Panics if an independently computed containment or bound fails.
fn check_oracle(
    plan: &PreparedExpm1<'_>,
    input: Expm1Input,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> TestResult {
    check_reading(
        plan.authority(),
        input,
        plan.evaluate(input, context)?.reading(),
        budget,
    )
}

/// Shared independent rational truth accepts either ownership route's actual reading.
fn check_reading(
    authority: &nrz_bindings::readings::algebra::enclosure::prepared_expm1::Prepared,
    input: Expm1Input,
    output: nrz_scalar::Expm1Output,
    budget: &mut Budget,
) -> TestResult {
    let truth = input
        .endpoints
        .map(|x| represented(-x, budget))
        .into_iter()
        .collect::<TestResult<Vec<_>>>()?
        .iter()
        .map(|a| oracles::truth(a, budget))
        .collect::<TestResult<Vec<_>>>()?;
    let lower = represented(output.endpoints[0], budget)?;
    let upper = represented(output.endpoints[1], budget)?;
    oracles::le(&lower, &truth[0][0], budget)?;
    oracles::le(&truth[1][1], &upper, budget)?;
    for (oracle, slack) in [
        (&truth[0], truth[0][1].sub(&lower, budget)?),
        (&truth[1], upper.sub(&truth[1][0], budget)?),
    ] {
        let allowed = oracle[1].negated(budget)?.mul(&power(-40)?, budget)?;
        oracles::le(&slack, &allowed, budget)?;
    }
    let nominal = oracles::truth(&represented(-input.center, budget)?, budget)?;
    let center = represented(output.center, budget)?;
    // Independent rational midpoint/tie search, not native float arithmetic.
    let exact_input = represented(input.center, budget)?;
    let proof = authority.proof();
    let mut rounded = Rational::zero();
    for coefficient in proof.coefficients.iter().rev() {
        rounded = oracles::nearest(&rounded.mul(&exact_input, budget)?, budget)?;
        rounded = oracles::nearest(&rounded.add(&coefficient.exact(budget)?, budget)?, budget)?;
    }
    rounded = oracles::nearest(&rounded.mul(&exact_input, budget)?, budget)?;
    assert_eq!(center, rounded);
    let error = represented(output.numerical_error, budget)?;
    let radius = represented(output.total_radius, budget)?;
    check_export(authority, input, output, budget)?;
    for endpoint in &nominal {
        oracles::le(
            &oracles::absolute(&center.sub(endpoint, budget)?, budget)?,
            &error,
            budget,
        )?;
    }
    oracles::le(&error, &radius, budget)?;
    for endpoint in [&lower, &upper] {
        oracles::le(
            &oracles::absolute(&center.sub(endpoint, budget)?, budget)?,
            &radius,
            budget,
        )?;
    }
    if input.center == 0.0 {
        assert_eq!(output.numerical_error.to_bits(), 0);
    }
    Ok(())
}

/// Independent exact rounding pins the exported neighbours and the non-additive total.
/// # Panics
/// Panics if an export stage changes or numerical error is added twice.
fn check_export(
    authority: &nrz_bindings::readings::algebra::enclosure::prepared_expm1::Prepared,
    input: Expm1Input,
    output: nrz_scalar::Expm1Output,
    budget: &mut Budget,
) -> TestResult {
    let lower = represented(output.endpoints[0], budget)?;
    let upper = represented(output.endpoints[1], budget)?;
    let center = represented(output.center, budget)?;
    let error = represented(output.numerical_error, budget)?;
    if input.endpoints[0].to_bits() == input.endpoints[1].to_bits() && input.center != 0.0 {
        let a = represented(-input.center, budget)?;
        let q = power(-i64::from(authority.proof().radius_bits))?;
        let rho = oracles::neighbour(
            &oracles::nearest(&a.mul(&q, budget)?, budget)?,
            true,
            budget,
        )?;
        let expected_lower = oracles::neighbour(
            &oracles::nearest(&center.sub(&rho, budget)?, budget)?,
            false,
            budget,
        )?;
        let expected_upper = oracles::neighbour(
            &oracles::nearest(&center.add(&rho, budget)?, budget)?,
            true,
            budget,
        )?;
        assert_eq!(lower, expected_lower);
        assert_eq!(upper, expected_upper);
        assert_eq!(error, rho);
    }
    let mut required = error;
    for (endpoint, upward) in [(&upper, false), (&lower, true)] {
        let difference = center.sub(endpoint, budget)?;
        let outward = if difference.is_zero() {
            Rational::zero()
        } else {
            oracles::neighbour(&oracles::nearest(&difference, budget)?, upward, budget)?
        };
        let magnitude = oracles::absolute(&outward, budget)?;
        if magnitude.compare(&required, budget)? == std::cmp::Ordering::Greater {
            required = magnitude;
        }
    }
    assert_eq!(represented(output.total_radius, budget)?, required);
    Ok(())
}

/// Reconstruct logical ownership from the fixture's independent exact spellings.
/// The native owner stores a fixed 1,024-byte coefficient array (inside `Self`),
/// while the returned exact authority has scope/key bytes, 1,024 fixed bytes,
/// 32 bytes per coefficient, six bound rationals, two rationals per stage, and
/// 256 bytes plus two rationals per coordinate. Each rational owns a 128-byte
/// header and four times its numerator/denominator byte lengths. Decimal long
/// division computes those lengths without `component_bits` or `owned_bytes`.
fn fixture_owned(plan: &PreparedExpm1<'_>) -> TestResult<u64> {
    let authority = plan.authority();
    let proof = authority.proof();
    let mut rational_bytes = 0;
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
        rational_bytes += 128 + 4 * (decimal_bytes(numerator) + decimal_bytes(denominator));
    }
    Ok(size_of::<PreparedExpm1<'_>>() as u64
        + proof.scope.len() as u64
        + authority.statement_key().len() as u64
        + 1024
        + 32 * proof.coefficients.len() as u64
        + 256 * authority.request().original.coordinates.len() as u64
        + rational_bytes)
}

/// Exact decimal long division by 256 supplies the minimum unsigned byte length.
fn decimal_bytes(text: &str) -> u64 {
    let mut digits: Vec<_> = text
        .trim_start_matches('-')
        .bytes()
        .map(|c| u16::from(c - b'0'))
        .collect();
    let mut count = 0;
    while digits.iter().any(|d| *d != 0) {
        let mut remainder = 0;
        for digit in &mut digits {
            let value = remainder * 10 + *digit;
            *digit = value / 256;
            remainder = value % 256;
        }
        count += 1;
    }
    count
}

/// Eight distinct targets retain judgments beside one compact original scope.
/// Warm preparation retains J+64 and one returned authority; the scope is below
/// 1,024 bytes, so its typed witness records and other temporary encodings fit
/// the 65,536-byte margin. The warm policy also admits 4,096*N+8,192
/// scratch bytes at degree N before lookup. Lowering retains G+A and scratch.
/// G+A+4,096*N+8,192+65,536 is a conservative independent allowance, not a
/// measured exact peak; the fixture requires it below duplicate G+J+A.
/// # Errors
/// Returns bounded fixture construction or native preparation failures.
/// # Panics
/// Panics if the fixture loses its separating inequality or native admission.
/// ´claim:matrix:prepared-expm1-constructor-ownership´
/// ´test:integration:prepared-expm1-constructor-ownership´
#[test]
fn prepared_expm1_constructor_ownership() -> TestResult {
    let mut f = Fixture::new()?;
    let r = broad(&mut f, 40)?;
    let context = ArithmeticContext::new()?;
    // Distinct targets deliberately reach preparation in unsorted order.
    for bits in [40, 33, 39, 34, 38, 35, 37, 36] {
        let mut candidate = r.clone();
        candidate.bits = bits;
        drop(PreparedExpm1::new(
            &mut f.cas,
            &candidate,
            [-2.0, 0.0],
            Policy::default(),
            &context,
            &mut ledger()?,
            &mut Counts::default(),
        )?);
    }
    let plan = PreparedExpm1::new(
        &mut f.cas,
        &r,
        [-2.0, 0.0],
        Policy::default(),
        &context,
        &mut ledger()?,
        &mut Counts::default(),
    )?;
    assert!(plan.authority().proof().scope.len() < 1024);
    let owned = fixture_owned(&plan)?;
    let degree = u64::from(plan.authority().proof().degree);
    drop(plan);
    let graph = f.cas.counts().payload_bytes;
    let judgments = f.cas.retained_judgment_bytes();
    let ceiling = graph + owned + 4096 * degree + 8192 + 65_536;
    assert!(ceiling < graph + judgments + owned);
    let mut limits = ledger()?.limits().clone();
    limits.staged_payload_bytes = ceiling;
    let mut budget = Budget::new(limits)?;
    budget.work(
        7,
        nrz_bindings::readings::algebra::Location::new("caller prefix"),
    )?;
    let plan = PreparedExpm1::new(
        &mut f.cas,
        &r,
        [-2.0, 0.0],
        Policy::default(),
        &context,
        &mut budget,
        &mut Counts::default(),
    )?;
    assert_eq!(
        plan.evaluate(point(0.0), &context)?
            .reading()
            .endpoints
            .map(f64::to_bits),
        [0, 0]
    );
    assert!(budget.usage().work > 7);
    Ok(())
}

/// Warm graph, fresh and reused preparation ledgers, with a retained caller prefix.
/// The exact live boundary is G + A + input + output, plus 64 only when this
/// graph is registered. A comes from `fixture_owned`, never `owned_bytes`.
/// A caller prefix P is additive. Leaving S+64 for registered B (S=G+A+I+O)
/// admits exactly; S+63 refuses. Both successful and failed calls retain Work.
/// # Errors
/// Returns preparation, exact fixture inspection or accounting failures.
/// # Panics
/// Panics if duplicate ownership is charged or a live owner is omitted.
/// ´claim:matrix:prepared-expm1-ledger-ownership´
/// ´test:integration:prepared-expm1-ledger-ownership´
#[test]
fn prepared_expm1_ledger_ownership() -> TestResult {
    let mut f = Fixture::new()?;
    let r = broad(&mut f, 40)?;
    let context = ArithmeticContext::new()?;
    drop(PreparedExpm1::new(
        &mut f.cas,
        &r,
        [-2.0, 0.0],
        Policy::default(),
        &context,
        &mut ledger()?,
        &mut Counts::default(),
    )?);
    let graph = f.cas.counts().payload_bytes;
    let judgments = f.cas.retained_judgment_bytes();
    let inputs = [point(-0.25), point(0.0), point(-1.0)];
    let cells = std::mem::size_of_val(&inputs) as u64
        + 3 * size_of::<Result<nrz_matrix::Expm1Evaluation<'_>, Expm1Failure>>() as u64;
    for reused in [false, true] {
        for short in [0, 1] {
            let mut preparation = ledger()?;
            let plan = PreparedExpm1::new(
                &mut f.cas,
                &r,
                [-2.0, 0.0],
                Policy::default(),
                &context,
                &mut preparation,
                &mut Counts::default(),
            )?;
            let owned = fixture_owned(&plan)?;
            let peak = graph + owned + cells + u64::from(reused) * 64;
            let mut fresh = ledger()?;
            let budget = if reused { &mut preparation } else { &mut fresh };
            budget.work(
                7,
                nrz_bindings::readings::algebra::Location::new("caller prefix"),
            )?;
            let before = budget.usage().work;
            // Leave the exact boundary from a generous ledger while retaining the
            // caller's remaining bytes throughout evaluation, including refusal.
            let prefix = budget.limits().staged_payload_bytes - peak + short;
            let result = budget.with_retained_payload(
                prefix,
                nrz_bindings::readings::algebra::Location::new("caller prefix"),
                |b| plan.evaluate_batch(&inputs, &context, b),
            );
            if short == 0 {
                assert_eq!(result?.len(), inputs.len());
                assert_eq!(budget.usage().work, before + 3 + u64::from(reused));
            } else {
                // The outer graph reservation has already reserved G-J when B
                // owns J, or G on fresh ledgers. The inner owner/cell admission
                // still sees B's J+64 as additive registered payload.
                let graph_additional = if reused { graph - judgments } else { graph };
                let needed = peak - graph_additional;
                assert_eq!(
                    result.expect_err("one below once-counted footprint").kind,
                    FaultKind::Exhausted {
                        reason: nrz_bindings::readings::algebra::Exhaustion::Limit,
                        budget: BudgetKind::StagedPayloadBytes,
                        limit: needed - 1,
                        needed,
                    }
                );
                assert_eq!(budget.usage().work, before + u64::from(reused));
            }
        }
    }
    Ok(())
}
