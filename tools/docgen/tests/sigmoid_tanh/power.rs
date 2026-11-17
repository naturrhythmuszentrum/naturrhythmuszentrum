// SPDX-License-Identifier: AGPL-3.0-only
//! Dyadic power coverage uses the existing prepared payload and CAS fixture.

use super::{Fixture, TestResult, binary64, read_payload};
use nrz_cas::enclosure::{
    CheckedEnclosure, EnclosureCertificate, EnclosureReport, EnclosureRequest, RefinementPolicy,
};
use nrz_cas::limits::{Budget, Limits};
#[path = "profile.rs"]
pub mod profile;
use nrz_cas::syntax::Operator;
use nrz_cas::{FaultKind, InvalidCode, Rational};
use serde_json::{Value, json};
use std::cmp::Ordering;

fn payload() -> TestResult<Value> {
    read_payload("dyadic-power.json")
}

fn limits() -> Limits {
    Limits {
        work: 1_000_000_000_000,
        literal_bytes: 4096,
        integer_exponent: 4096,
        arithmetic_steps: 500_000_000,
        component_bits: 65536,
        intermediate_bits: 131_072,
        name_bytes: 131_072,
        source_bytes: 4_194_304,
        output_bytes: 4_194_304,
        witness_bytes: 4_194_304,
        ..Limits::default()
    }
}

fn ledger() -> TestResult<Budget> {
    Ok(Budget::new(limits())?)
}

const fn policy() -> RefinementPolicy {
    RefinementPolicy {
        version: 1,
        precision: 160,
        grid_ceiling: 256,
        rounds: 1,
        reduction_ceiling: 4,
        term_ceiling: 128,
        checking_work: 0,
    }
}

fn original(x: &str, q: &str) -> TestResult<(Fixture, EnclosureRequest)> {
    let mut store_limits = limits();
    // Preserve the caller profile after intersection with permanent store limits.
    store_limits.component_bits = profile::reproduction_limits().component_bits;
    let mut f = Fixture::with_names_and_limits(&["x"], store_limits)?;
    let x = f.term(Operator::Rational(Rational::parse(x, &mut ledger()?)?), &[])?;
    let q = f.term(Operator::Rational(Rational::parse(q, &mut ledger()?)?), &[])?;
    let target = f.term(Operator::PositivePower, &[x, q])?;
    let request = f.request(target);
    Ok((f, request))
}

fn row<'a>(payload: &'a Value, id: &str) -> TestResult<&'a Value> {
    payload["entries"]
        .as_array()
        .ok_or("entries")?
        .iter()
        .find(|row| row["id"] == id)
        .ok_or_else(|| format!("missing row {id}").into())
}

/// Stored rows correspond exactly to independent binary64 input reconstructions.
/// # Errors
/// Returns fixture-reading, parsing or exact-arithmetic errors.
/// # Panics
/// Panics if a stored identity, exponent, target, outcome or declared route changes.
pub fn power_golden_associations() -> TestResult {
    let all = payload()?;
    let normal = 1_u64 << 52;
    let one = 1023_u64 << 52;
    let two = 1024_u64 << 52;
    let sixteenth = 1019_u64 << 52;
    let cases: &[(&str, &[u64], &str, Option<&str>)] = &[
        ("upper", &[two], "1/8", None),
        ("zero-exponent", &[one], "0", Some("outside-domain")),
        ("one", &[one], "1/8", None),
        ("normal-near-one", &[normal], "255/256", None),
        ("quarter", &[1022_u64 << 52], "1/4", None),
        ("outside-upper", &[two + 1], "1/4", Some("outside-domain")),
        ("rounding-box", &[sixteenth - 1, sixteenth + 1], "1/4", None),
        ("negative-exponent", &[one], "-1/4", Some("outside-domain")),
        ("identity", &[two], "1", None),
        ("normal-next-near-one", &[normal + 1], "255/256", None),
        ("three-quarters", &[1022_u64 << 52], "3/4", None),
        (
            "subnormal-max",
            &[normal - 1],
            "1/4",
            Some("below-normal-operand"),
        ),
        ("quarter-exact", &[sixteenth], "1/4", None),
        ("three-quarters-exact", &[sixteenth], "3/4", None),
        ("upper-predecessor", &[two - 1], "1/8", None),
        ("above-one-exponent", &[one], "5/4", Some("outside-domain")),
        ("one-neighbours", &[one - 1, one + 1], "1/8", None),
        ("eighth-exact", &[1015_u64 << 52], "1/8", None),
        ("subnormal-min", &[1], "1/4", Some("below-normal-operand")),
        ("seven-eighths", &[two], "7/8", None),
        ("zero-base", &[0], "1/4", Some("outside-domain")),
        ("non-dyadic", &[one], "1/3", Some("non-dyadic-exponent")),
        ("normal-identity", &[normal], "1", None),
        (
            "negative-base",
            &[0xbff0_0000_0000_0000],
            "1/4",
            Some("outside-domain"),
        ),
    ];
    let rows = all["entries"].as_array().ok_or("entries")?;
    assert_eq!(rows.len(), cases.len(), "power row count");
    for (name, bits, q, reason) in cases {
        let id = format!("power-{name}");
        let found = row(&all, &id)?;
        let values = bits
            .iter()
            .map(|b| binary64(*b))
            .collect::<TestResult<Vec<_>>>()?;
        let input = match values.as_slice() {
            [x] => json!({"point":x}),
            xs => json!({"box":xs}),
        };
        assert_eq!(found["operation"], "power", "operation association: {id}");
        assert_eq!(found["input"], input, "input association: {id}");
        assert_eq!(found["exponent"], *q, "exponent association: {id}");
        assert_eq!(
            found["target"],
            json!({"kind":"outward"}),
            "target association: {id}"
        );
        assert_eq!(
            found["expected"],
            reason.map_or_else(|| json!("enclosure"), |r| json!({"refusal":r})),
            "outcome association: {id}"
        );
    }
    assert!(
        rows.windows(2)
            .any(|p| p[0]["id"].as_str() > p[1]["id"].as_str()),
        "authored rows remain unsorted"
    );
    routes(&all);
    Ok(())
}

/// Pin the declared route registry without sharing numerical helpers.
/// # Panics
/// Panics if a declared route or its precision changes.
fn routes(all: &Value) {
    for (i, id, kind, tool, revision, precision, value) in [
        (
            0,
            "nrz-cas",
            "nrz-cas-enclosure",
            "nrz-cas",
            "2add514fef00ac86399b60e113b71c9be52c91a1",
            "precision_bits",
            160,
        ),
        (
            1,
            "mpmath-iv",
            "mpmath-interval",
            "mpmath 1.3.0",
            "b5c04506ef0cd4a1f1213f8389ee21c9c3551582",
            "precision_decimal_digits",
            150,
        ),
    ] {
        let route = &all["routes"][i];
        assert_eq!(route["id"], id, "route identity");
        assert_eq!(route["kind"], kind, "route kind");
        assert_eq!(route["tool"], tool, "route tool");
        assert_eq!(route["revision"], revision, "route revision");
        assert_eq!(route[precision], value, "route precision");
    }
}

/// Every endpoint reproduces through CAS and independent fresh-store certificate replay.
/// # Errors
/// Returns fixture-reading, preparation, replay or exact-arithmetic errors.
/// # Panics
/// Panics if retained intervals, original scope or complete golden containment disagree.
pub fn power_golden_cas_route() -> TestResult {
    let all = payload()?;
    let mut count = 0;
    let mut maximum_work = 0;
    let mut maximum_steps = 0;
    for row in all["entries"].as_array().ok_or("entries")? {
        for endpoint in row["evidence"].as_array().ok_or("evidence")? {
            let observed = reproduce(row, endpoint, profile::reproduction_limits())?;
            maximum_work = maximum_work.max(observed.work);
            maximum_steps = maximum_steps.max(observed.arithmetic_steps);
            count += 1;
        }
    }
    assert_eq!(count, 17, "replayed power endpoint count");
    assert_eq!(
        maximum_work,
        profile::reproduction_minimum().work,
        "reproduction Work maximum"
    );
    assert_eq!(
        maximum_steps,
        profile::reproduction_minimum().arithmetic_steps,
        "reproduction arithmetic-step maximum"
    );
    Ok(())
}

#[derive(Debug, PartialEq)]
struct Observation {
    work: u64,
    arithmetic_steps: u64,
    phase: &'static str,
    signature: Vec<String>,
}

fn retain(
    report: EnclosureReport,
    observation: &mut Observation,
    phase: &'static str,
) -> TestResult<CheckedEnclosure> {
    if let Some(fault) = report.resource_fault {
        return Err(fault.into());
    }
    if report.usage.work > observation.work {
        observation.work = report.usage.work;
        observation.phase = phase;
    }
    observation.arithmetic_steps = observation
        .arithmetic_steps
        .max(report.usage.arithmetic_steps);
    observation.signature.push(format!("{:?}", report.outcome));
    let checked = report.checked.ok_or("missing checked enclosure")?;
    observation
        .signature
        .push(checked.interval().lower().exact_string(&mut ledger()?)?);
    observation
        .signature
        .push(checked.interval().upper().exact_string(&mut ledger()?)?);
    Ok(checked)
}

fn reproduce(row: &Value, endpoint: &Value, caps: Limits) -> TestResult<Observation> {
    let mut observation = Observation {
        work: 0,
        arithmetic_steps: 0,
        phase: "",
        signature: Vec::new(),
    };
    let x = endpoint["input"].as_str().ok_or("input")?;
    let q = row["exponent"].as_str().ok_or("exponent")?;
    let (mut source, request) = original(x, q)?;
    let coarse_policy = RefinementPolicy {
        precision: 128,
        ..policy()
    };
    let coarse_report = source.cas.enclose(
        &request,
        coarse_policy,
        &mut Budget::new(caps.clone())?,
        &mut |_| false,
    )?;
    let coarse = retain(coarse_report, &mut observation, "coarse")?;
    let fine_report = source.cas.enclose(
        &request,
        policy(),
        &mut Budget::new(caps.clone())?,
        &mut |_| false,
    )?;
    let checked = retain(fine_report, &mut observation, "fine")?;
    assert!(
        coarse
            .interval()
            .contains(checked.interval(), &mut ledger()?)?,
        "power refinement"
    );
    assert_eq!(checked.formal_schema().name, "enclosure.positive-power");
    assert_eq!(
        checked.request().target,
        request.target,
        "original power identity"
    );
    let bytes = checked.certificate().to_bytes(&mut ledger()?)?;
    let certificate = EnclosureCertificate::from_bytes(&bytes, &mut ledger()?)?;
    let (mut fresh, expected) = original(x, q)?;
    let replay_report = fresh.cas.check_enclosure(
        &expected,
        policy(),
        &certificate,
        &mut Budget::new(caps)?,
        &mut |_| false,
    )?;
    let replay = retain(replay_report, &mut observation, "replay")?;
    assert_eq!(checked.interval(), replay.interval(), "fresh power replay");
    let pair = json!([
        checked.interval().lower().exact_string(&mut ledger()?)?,
        checked.interval().upper().exact_string(&mut ledger()?)?
    ]);
    assert_eq!(
        endpoint["routes"][0]["interval"], pair,
        "CAS route: {}",
        row["id"]
    );
    let name = endpoint["endpoint"].as_str().ok_or("endpoint")?;
    let golden = &row["golden"][name];
    let lo = Rational::parse(golden[0].as_str().ok_or("lower")?, &mut ledger()?)?;
    let hi = Rational::parse(golden[1].as_str().ok_or("upper")?, &mut ledger()?)?;
    assert_ne!(
        checked.interval().lower().compare(&lo, &mut ledger()?)?,
        Ordering::Greater,
        "whole lower containment"
    );
    assert_ne!(
        checked.interval().upper().compare(&hi, &mut ledger()?)?,
        Ordering::Less,
        "whole upper containment"
    );
    Ok(observation)
}

/// A changed original exponent cannot reuse the quarter-power certificate.
/// # Errors
/// Returns fixture-reading, preparation or exact-arithmetic errors.
/// # Panics
/// Panics if the stored quarter case drifts or another exponent authenticates its proof.
pub fn power_golden_original_exponent() -> TestResult {
    let all = payload()?;
    let r = row(&all, "power-quarter")?;
    assert_eq!(r["input"], json!({"point":"1/2"}));
    assert_eq!(r["exponent"], "1/4");
    let (mut source, request) = original("1/2", "1/4")?;
    let checked = source
        .cas
        .enclose(&request, policy(), &mut ledger()?, &mut |_| false)?
        .checked
        .ok_or("enclosure")?;
    let (mut fresh, wrong) = original("1/2", "1/8")?;
    let result = fresh.cas.check_enclosure(
        &wrong,
        policy(),
        checked.certificate(),
        &mut ledger()?,
        &mut |_| false,
    );
    assert!(
        matches!(result, Err(ref fault) if fault.kind == FaultKind::Invalid(InvalidCode::EndpointMismatch)),
        "changed original exponent refusal: {result:?}"
    );
    Ok(())
}

/// Exact fourth and eighth powers and integer-decoded adjacent values pin rounding-boundary rows.
/// # Errors
/// Returns fixture-reading, parsing or exact-arithmetic errors.
/// # Panics
/// Panics if exact anchors or the whole neighbouring golden intervals cross their bounds.
pub fn power_golden_rounding_boundaries() -> TestResult {
    let all = payload()?;
    let half = 1022_u64 << 52;
    let r = row(&all, "power-rounding-box")?;
    let sixteenth = 1019_u64 << 52;
    assert_eq!(
        r["input"],
        json!({"box":[binary64(sixteenth-1)?,binary64(sixteenth+1)?]}),
        "stored rounding input"
    );
    assert_eq!(r["exponent"], "1/4", "stored rounding exponent");
    let anchor = Rational::from_ratio(1, 2, &mut ledger()?)?;
    assert_eq!(
        anchor
            .pow(4, &mut ledger()?)?
            .exact_string(&mut ledger()?)?,
        "1/16"
    );
    assert_eq!(
        anchor
            .pow(8, &mut ledger()?)?
            .exact_string(&mut ledger()?)?,
        "1/256"
    );
    for id in ["power-quarter-exact", "power-eighth-exact"] {
        assert_eq!(
            row(&all, id)?["golden"]["point"],
            json!(["1/2", "1/2"]),
            "exact algebraic anchor: {id}"
        );
    }
    for (i, name, down, up) in [
        (0, "lower_endpoint", half - 1, half),
        (1, "upper_endpoint", half, half + 1),
    ] {
        let lower = Rational::parse(&binary64(down)?, &mut ledger()?)?;
        let upper = Rational::parse(&binary64(up)?, &mut ledger()?)?;
        let x = Rational::parse(
            r["input"]["box"][i].as_str().ok_or("input")?,
            &mut ledger()?,
        )?;
        assert_eq!(
            lower.pow(4, &mut ledger()?)?.compare(&x, &mut ledger()?)?,
            Ordering::Less,
            "lower fourth power"
        );
        assert_eq!(
            upper.pow(4, &mut ledger()?)?.compare(&x, &mut ledger()?)?,
            Ordering::Greater,
            "upper fourth power"
        );
        let lo = Rational::parse(
            r["golden"][name][0].as_str().ok_or("lower")?,
            &mut ledger()?,
        )?;
        let hi = Rational::parse(
            r["golden"][name][1].as_str().ok_or("upper")?,
            &mut ledger()?,
        )?;
        assert_eq!(
            lo.compare(&lower, &mut ledger()?)?,
            Ordering::Greater,
            "whole golden above neighbour"
        );
        assert_eq!(
            hi.compare(&upper, &mut ledger()?)?,
            Ordering::Less,
            "whole golden below neighbour"
        );
    }
    assert_eq!(
        row(&all, "power-three-quarters-exact")?["golden"]["point"],
        json!(["1/8", "1/8"]),
        "non-unit exact anchor"
    );
    eighth_boundaries(&all)
}

/// Place the stored eighth-power box against adjacent represented values.
/// # Panics
/// Panics if its input, exponent or exact neighbour comparisons change.
fn eighth_boundaries(all: &Value) -> TestResult {
    let r = row(all, "power-one-neighbours")?;
    let one = 1023_u64 << 52;
    assert_eq!(
        r["input"],
        json!({"box":[binary64(one-1)?,binary64(one+1)?]})
    );
    assert_eq!(r["exponent"], "1/8");
    for (i, name, down, up) in [
        (0, "lower_endpoint", one - 1, one),
        (1, "upper_endpoint", one, one + 1),
    ] {
        let mut b = ledger()?;
        let lower = Rational::parse(&binary64(down)?, &mut b)?;
        let upper = Rational::parse(&binary64(up)?, &mut b)?;
        let x = Rational::parse(r["input"]["box"][i].as_str().ok_or("input")?, &mut b)?;
        assert_eq!(
            lower.pow(8, &mut b)?.compare(&x, &mut b)?,
            Ordering::Less,
            "lower eighth power"
        );
        assert_eq!(
            upper.pow(8, &mut b)?.compare(&x, &mut b)?,
            Ordering::Greater,
            "upper eighth power"
        );
        let lo = Rational::parse(r["golden"][name][0].as_str().ok_or("lower")?, &mut b)?;
        let hi = Rational::parse(r["golden"][name][1].as_str().ok_or("upper")?, &mut b)?;
        assert_eq!(
            lo.compare(&lower, &mut b)?,
            Ordering::Greater,
            "whole golden above eighth-power neighbour"
        );
        assert_eq!(
            hi.compare(&upper, &mut b)?,
            Ordering::Less,
            "whole golden below eighth-power neighbour"
        );
    }
    Ok(())
}

fn power_bounds(
    pair: &[Rational; 2],
    image: &Rational,
    denominator: i64,
    b: &mut Budget,
) -> TestResult<bool> {
    Ok(
        pair[0].pow(denominator, b)?.compare(image, b)? != Ordering::Greater
            && pair[1].pow(denominator, b)?.compare(image, b)? != Ordering::Less,
    )
}

/// Exact integer powers independently bound every route and golden endpoint image.
/// # Errors
/// Returns fixture-reading, parsing, budget or exact-arithmetic errors.
/// # Panics
/// Panics if an endpoint inequality, relative width or isolated comparison changes.
pub fn power_golden_exact_inequalities() -> TestResult {
    // Only exact rational arithmetic is shared with the routes: no enclosure,
    // replay, root schedule, directed library or native evaluator is called.
    let all = payload()?;
    let mut count = 0;
    let mut maximum_work = 0;
    let mut maximum_steps = 0;
    for row in all["entries"].as_array().ok_or("entries")? {
        for endpoint in row["evidence"].as_array().ok_or("evidence")? {
            let observed = compare_endpoint(row, endpoint, profile::comparison_limits())?;
            maximum_work = maximum_work.max(observed.work);
            maximum_steps = maximum_steps.max(observed.arithmetic_steps);
            count += 1;
        }
    }
    assert_eq!(count, 17, "exact power endpoint count");
    assert_eq!(
        maximum_work,
        profile::comparison_minimum().work,
        "comparison Work maximum"
    );
    assert_eq!(
        maximum_steps,
        profile::comparison_minimum().arithmetic_steps,
        "comparison arithmetic-step maximum"
    );
    let mut budget = ledger()?;
    let one = Rational::from_ratio(1, 1, &mut budget)?;
    let two = Rational::from_ratio(2, 1, &mut budget)?;
    let half = Rational::from_ratio(1, 2, &mut budget)?;
    assert!(
        !power_bounds(&[two.clone(), two], &one, 8, &mut budget)?,
        "exact lower comparison"
    );
    assert!(
        !power_bounds(&[half.clone(), half], &one, 8, &mut budget)?,
        "exact upper comparison"
    );
    Ok(())
}

fn compare_endpoint(row: &Value, endpoint: &Value, caps: Limits) -> TestResult<Observation> {
    let mut budget = Budget::new(caps)?;
    let exponent = row["exponent"].as_str().ok_or("exponent")?;
    let (numerator, denominator) = exponent.split_once('/').unwrap_or((exponent, "1"));
    let numerator: i64 = numerator.parse()?;
    let denominator: i64 = denominator.parse()?;
    assert!(
        numerator > 0 && numerator <= denominator && u64::try_from(denominator)?.is_power_of_two(),
        "exact dyadic exponent"
    );
    let input = Rational::parse(endpoint["input"].as_str().ok_or("input")?, &mut budget)?;
    let image = input.pow(numerator, &mut budget)?;
    let name = endpoint["endpoint"].as_str().ok_or("endpoint")?;
    let golden = &row["golden"][name];
    for pair in [
        &endpoint["routes"][0]["interval"],
        &endpoint["routes"][1]["interval"],
        golden,
    ] {
        let pair = [
            Rational::parse(pair[0].as_str().ok_or("lower")?, &mut budget)?,
            Rational::parse(pair[1].as_str().ok_or("upper")?, &mut budget)?,
        ];
        assert!(
            power_bounds(&pair, &image, denominator, &mut budget)?,
            "exact power inequalities: {} {name}",
            row["id"]
        );
    }
    let lo = Rational::parse(golden[0].as_str().ok_or("lower")?, &mut budget)?;
    let hi = Rational::parse(golden[1].as_str().ok_or("upper")?, &mut budget)?;
    let allowance = input.mul(
        &Rational::from_ratio(2, 1, &mut budget)?.pow(-120, &mut budget)?,
        &mut budget,
    )?;
    assert_ne!(
        hi.sub(&lo, &mut budget)?.compare(&allowance, &mut budget)?,
        Ordering::Greater,
        "relative power golden width"
    );
    Ok(Observation {
        work: budget.usage().work,
        arithmetic_steps: budget.usage().arithmetic_steps,
        phase: "comparison",
        signature: Vec::new(),
    })
}
