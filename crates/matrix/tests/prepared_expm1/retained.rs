// SPDX-License-Identifier: AGPL-3.0-only
//! Retained-owner readings use the independent rational and validated golden routes.

use super::{
    ArithmeticContext, Counts, Expm1Failure, Fixture, Policy, TestResult, broad, check_reading,
    ledger, oracles, point, represented,
};
use nrz_bindings::readings::algebra::{
    Rational,
    limits::{Budget, Limits},
};
use nrz_matrix::{ExecutionAccount, Heap, HeapLimits};

/// No second production evaluation supplies truth or endpoint slack expectations.
#[test]
fn retained_expm1_native_oracles() -> TestResult {
    let mut fixture = Fixture::new()?;
    let request = broad(&mut fixture, 40)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = ExecutionAccount::new(ledger()?)?;
    let context = ArithmeticContext::new()?;
    let mut counts = Counts::default();
    let law = heap
        .prepare_retained_expm1(
            fixture.cas,
            &request,
            [-2.0, 0.0],
            Policy::default(),
            &context,
            &mut counts,
            &mut account,
        )
        .map_err(|(error, _)| error)?;
    let other = heap.retain_retained_expm1(&law, &mut account)?;
    drop(law);
    let work = account.usage().work;
    let preparation = counts.clone();
    let mut oracle = Budget::new(Limits {
        work: 40_000_000_000,
        arithmetic_steps: 100_000_000,
        component_bits: 32_768,
        intermediate_bits: 131_072,
        integer_exponent: 16_384,
        ..Limits::default()
    })?;
    for input in [
        -2.0,
        (-2.0_f64).next_up(),
        -1.0,
        -0.25,
        -2.0_f64.powi(-40),
        -2.0_f64.powi(-10),
        -f64::MIN_POSITIVE,
        -f64::MIN_POSITIVE.next_up(),
        0.0,
    ]
    .map(point)
    {
        let evaluated = other.evaluate(input, &context)?;
        check_reading(
            evaluated.authority(),
            input,
            evaluated.reading(),
            &mut oracle,
        )?;
    }
    assert_eq!(
        other
            .evaluate(point(-f64::MIN_POSITIVE.next_down()), &context)
            .expect_err("normal threshold"),
        Expm1Failure::BelowNormalOperand
    );
    assert_eq!(counts, preparation);
    assert_eq!(account.usage().work, work);
    Ok(())
}
/// Validated structured fixture bounds independently bracket every retained-owner reading.
#[test]
fn retained_expm1_validated_fixture() -> TestResult {
    let payload = nrz_docgen::golden::read(
        include_bytes!("../../../../data/golden/prepared-binary64.json").as_slice(),
        nrz_docgen::golden::Limits::default(),
        &mut ledger()?,
    )?;
    let mut fixture = Fixture::new()?;
    let request = broad(&mut fixture, 40)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = ExecutionAccount::new(ledger()?)?;
    let context = ArithmeticContext::new()?;
    let law = heap
        .prepare_retained_expm1(
            fixture.cas,
            &request,
            [-2.0, 0.0],
            Policy::default(),
            &context,
            &mut Counts::default(),
            &mut account,
        )
        .map_err(|(error, _)| error)?;
    let mut identities = Vec::new();
    for row in payload.value()["entries"].as_array().ok_or("entries")? {
        if row["operation"] != "expm1" {
            continue;
        }
        let id = row["id"].as_str().ok_or("id")?;
        let spelling = row["input"]["point"].as_str().ok_or("point")?;
        let expected = match id {
            "M1" => "-1/1024",
            "M2" => "-1/1099511627776",
            "M0" => "0",
            _ => return Err("unexpected expm1 row".into()),
        };
        assert_eq!(spelling, expected);
        assert_eq!(row["target"]["kind"], "relative");
        assert_eq!(row["target"]["bits"], 40);
        let (n, d) = spelling.split_once('/').unwrap_or((spelling, "1"));
        let x = n.parse::<f64>()? / d.parse::<f64>()?;
        assert_eq!(
            represented(x, &mut ledger()?)?,
            Rational::parse(spelling, &mut ledger()?)?
        );
        let output = law.evaluate(point(x), &context)?.reading();
        let golden = row["golden"]["point"].as_array().ok_or("golden")?;
        let lower = Rational::parse(golden[0].as_str().ok_or("lower")?, &mut ledger()?)?;
        let upper = Rational::parse(golden[1].as_str().ok_or("upper")?, &mut ledger()?)?;
        oracles::le(
            &represented(output.endpoints[0], &mut ledger()?)?,
            &lower,
            &mut ledger()?,
        )?;
        oracles::le(
            &upper,
            &represented(output.endpoints[1], &mut ledger()?)?,
            &mut ledger()?,
        )?;
        identities.push(id);
    }
    assert_eq!(identities, ["M1", "M2", "M0"]);
    Ok(())
}
/// Represented subdomains remain paired with their exact owner's program after heap destruction.
#[test]
fn retained_expm1_narrow_domain() -> TestResult {
    let mut fixture = Fixture::new()?;
    let request = broad(&mut fixture, 40)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = ExecutionAccount::new(ledger()?)?;
    let context = ArithmeticContext::new()?;
    let law = heap
        .prepare_retained_expm1(
            fixture.cas,
            &request,
            [-0.75, -0.5],
            Policy::default(),
            &context,
            &mut Counts::default(),
            &mut account,
        )
        .map_err(|(error, _)| error)?;
    drop(heap);
    assert_eq!(
        law.domain()?.map(f64::to_bits),
        [-0.75_f64, -0.5].map(f64::to_bits)
    );
    assert_eq!(
        law.evaluate(point(-0.25), &context)
            .expect_err("outside admitted subdomain"),
        Expm1Failure::OutsideDomain
    );
    check_reading(
        law.authority()?,
        point(-0.5),
        law.evaluate(point(-0.5), &context)?.reading(),
        &mut ledger()?,
    )
}
