// SPDX-License-Identifier: AGPL-3.0-only
//! Independent point controls for the conditional dyadic-power numerical leaf.
use super::*;
use nrz_scalar::{
    ArithmeticFailure, DyadicPowerFailure, DyadicPowerLimits, DyadicPowerOutput, DyadicPowerProgram,
};
use std::cmp::Ordering;
use std::io::Write as _;

/// Capacities admitting every selected replay and observation schedule.
const LIMITS: DyadicPowerLimits = DyadicPowerLimits {
    exponent_bits: 63,
    root_steps: 63,
    product_steps: 124,
};

/// Decode a stored exact binary64 spelling without rounding it.
fn exact_input(text: &str) -> TestResult<f64> {
    let mut b = ledger()?;
    let q = Rational::parse(text, &mut b)?;
    let magnitude = oracles::absolute(&q, &mut b)?;
    let bits = oracles::nearest_positive_bits(&magnitude, &mut b)?
        .ok_or("golden input exceeds finite binary64")?;
    assert_eq!(oracles::value(bits, &mut b)?, magnitude, "{text}");
    Ok(f64::from_bits(
        bits | if q.is_negative() { 1_u64 << 63 } else { 0 },
    ))
}

/// Exact stored ratio; classification remains integer-only in the native constructor.
fn exponent(text: &str) -> TestResult<(i64, u64)> {
    let (top, bottom) = text.split_once('/').unwrap_or((text, "1"));
    Ok((top.parse()?, bottom.parse()?))
}

/// Fixed corpus identities bind each test input, exponent and expected outcome.
fn expected_row(id: &str) -> Option<(i64, u64, [u64; 2], bool, &'static str)> {
    let normal = f64::MIN_POSITIVE.to_bits();
    let half = 0.5_f64.to_bits();
    let one = 1.0_f64.to_bits();
    let two = 2.0_f64.to_bits();
    let quarter_anchor = (1.0_f64 / 16.0).to_bits();
    let eighth_anchor = (1.0_f64 / 256.0).to_bits();
    let row = match id {
        "power-upper" => (1, 8, [two; 2], false, "enclosure"),
        "power-zero-exponent" => (0, 1, [one; 2], false, "outside-domain"),
        "power-one" => (1, 8, [one; 2], false, "enclosure"),
        "power-normal-near-one" => (255, 256, [normal; 2], false, "enclosure"),
        "power-quarter" => (1, 4, [half; 2], false, "enclosure"),
        "power-outside-upper" => (
            1,
            4,
            [2.0_f64.next_up().to_bits(); 2],
            false,
            "outside-domain",
        ),
        "power-rounding-box" => (
            1,
            4,
            [
                (1.0_f64 / 16.0).next_down().to_bits(),
                (1.0_f64 / 16.0).next_up().to_bits(),
            ],
            true,
            "enclosure",
        ),
        "power-negative-exponent" => (-1, 4, [one; 2], false, "outside-domain"),
        "power-identity" => (1, 1, [two; 2], false, "enclosure"),
        "power-normal-next-near-one" => (
            255,
            256,
            [f64::MIN_POSITIVE.next_up().to_bits(); 2],
            false,
            "enclosure",
        ),
        "power-three-quarters" => (3, 4, [half; 2], false, "enclosure"),
        "power-subnormal-max" => (1, 4, [(normal - 1); 2], false, "below-normal-operand"),
        "power-quarter-exact" => (1, 4, [quarter_anchor; 2], false, "enclosure"),
        "power-upper-predecessor" => (1, 8, [2.0_f64.next_down().to_bits(); 2], false, "enclosure"),
        "power-above-one-exponent" => (5, 4, [one; 2], false, "outside-domain"),
        "power-one-neighbours" => (
            1,
            8,
            [1.0_f64.next_down().to_bits(), 1.0_f64.next_up().to_bits()],
            true,
            "enclosure",
        ),
        "power-eighth-exact" => (1, 8, [eighth_anchor; 2], false, "enclosure"),
        "power-subnormal-min" => (1, 4, [1; 2], false, "below-normal-operand"),
        "power-seven-eighths" => (7, 8, [two; 2], false, "enclosure"),
        "power-zero-base" => (1, 4, [0; 2], false, "outside-domain"),
        "power-non-dyadic" => (1, 3, [one; 2], false, "non-dyadic-exponent"),
        "power-normal-identity" => (1, 1, [normal; 2], false, "enclosure"),
        "power-negative-base" => (1, 4, [(-1.0_f64).to_bits(); 2], false, "outside-domain"),
        "power-three-quarters-exact" => (3, 4, [quarter_anchor; 2], false, "enclosure"),
        _ => return None,
    };
    Some(row)
}

/// Whole golden pairs at each original point, with phase and cause for every refusal.
/// # Panics
/// Panics if a stored row loses identity, containment, phase or typed cause.
/// ´claim:matrix:scalar-dyadic-power-golden-values´
/// ´test:integration:scalar-dyadic-power-golden-values´
#[expect(
    clippy::too_many_lines,
    reason = "one selected test binds all stored identities and whole endpoint pairs"
)]
#[test]
fn scalar_dyadic_power_golden_values() -> TestResult {
    let payload = nrz_docgen::golden::read(
        include_bytes!("../../../../data/golden/dyadic-power.json").as_slice(),
        nrz_docgen::golden::Limits::default(),
        &mut ledger()?,
    )?;
    let context = ArithmeticContext::new()?;
    let mut successes = 0;
    let mut endpoint_records = 0;
    let mut refusals = 0;
    for row in payload.value()["entries"].as_array().ok_or("entries")? {
        assert_eq!(row["operation"], "power");
        assert_eq!(row["target"]["kind"], "outward");
        let id = row["id"].as_str().ok_or("id")?;
        let (numerator, denominator) = exponent(row["exponent"].as_str().ok_or("exponent")?)?;
        let (fixed_num, fixed_den, fixed_bits, fixed_box, fixed_outcome) =
            expected_row(id).ok_or("unknown power row")?;
        assert_eq!((numerator, denominator), (fixed_num, fixed_den), "{id}");
        let texts = if let Some(point) = row["input"]["point"].as_str() {
            assert!(!fixed_box, "{id}");
            [point, point]
        } else {
            assert!(fixed_box, "{id}");
            [
                row["input"]["box"][0].as_str().ok_or("box lower")?,
                row["input"]["box"][1].as_str().ok_or("box upper")?,
            ]
        };
        assert_eq!(
            [
                exact_input(texts[0])?.to_bits(),
                exact_input(texts[1])?.to_bits()
            ],
            fixed_bits,
            "{id}"
        );
        let outcome = row["expected"]["refusal"].as_str().unwrap_or("enclosure");
        assert_eq!(outcome, fixed_outcome, "{id}");
        let program = DyadicPowerProgram::new(
            [f64::MIN_POSITIVE, 2.0],
            numerator,
            denominator,
            LIMITS,
            &context,
        );
        if let Some(reason) = row["expected"]["refusal"].as_str() {
            let expected = match reason {
                "non-dyadic-exponent" => DyadicPowerFailure::NonDyadicExponent,
                "outside-domain" => DyadicPowerFailure::OutsideDomain,
                "below-normal-operand" => DyadicPowerFailure::BelowNormalOperand,
                _ => return Err(format!("{id}: unknown refusal {reason}").into()),
            };
            if id.contains("exponent") || id == "power-non-dyadic" {
                assert_eq!(program.expect_err("construction refusal"), expected, "{id}");
            } else {
                let program = program?;
                let text = row["input"]["point"].as_str().ok_or("refusal point")?;
                assert_eq!(
                    program
                        .evaluate(exact_input(text)?, &context)
                        .expect_err("point refusal"),
                    expected,
                    "{id}"
                );
            }
            refusals += 1;
            continue;
        }
        assert_eq!(row["expected"], "enclosure", "{id}");
        let program = program?;
        let points = if let Some(text) = row["input"]["point"].as_str() {
            vec![("point", text)]
        } else {
            vec![
                (
                    "lower_endpoint",
                    row["input"]["box"][0].as_str().ok_or("box lower")?,
                ),
                (
                    "upper_endpoint",
                    row["input"]["box"][1].as_str().ok_or("box upper")?,
                ),
            ]
        };
        for (name, text) in points {
            let x = exact_input(text)?;
            let output = program.evaluate(x, &context)?;
            let pair = row["golden"][name].as_array().ok_or("golden pair")?;
            let mut b = ledger()?;
            let low = Rational::parse(pair[0].as_str().ok_or("golden lower")?, &mut b)?;
            let high = Rational::parse(pair[1].as_str().ok_or("golden upper")?, &mut b)?;
            let native_low = represented(output.endpoints[0], &mut b)?;
            let native_high = represented(output.endpoints[1], &mut b)?;
            assert_ne!(
                native_low.compare(&low, &mut b)?,
                Ordering::Greater,
                "{id}/{name}: stored {pair:?}, native {:?}",
                output.endpoints
            );
            assert_ne!(
                high.compare(&native_high, &mut b)?,
                Ordering::Greater,
                "{id}/{name}: stored {pair:?}, native {:?}",
                output.endpoints
            );
            endpoint_records += 1;
        }
        successes += 1;
    }
    assert_eq!((successes, endpoint_records, refusals), (15, 17, 9));
    Ok(())
}

/// Nearest binary64 square root by exact squares and exact midpoint cells.
fn nearest_root_bits(q: &Rational, b: &mut Budget) -> TestResult<u64> {
    let mut low = 0_u64;
    let mut high = 2.0_f64.to_bits();
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        let value = oracles::value(middle, b)?;
        let square = value.mul(&value, b)?;
        if square.compare(q, b)? == Ordering::Greater {
            high = middle;
        } else {
            low = middle;
        }
    }
    let left = oracles::value(low, b)?;
    let right = oracles::value(high, b)?;
    let midpoint = left.add(&right, b)?.div(&number(2, 1)?, b)?;
    let threshold = midpoint.mul(&midpoint, b)?;
    Ok(
        if q.compare(&threshold, b)? == Ordering::Greater
            || (q.compare(&threshold, b)? == Ordering::Equal && low & 1 != 0)
        {
            high
        } else {
            low
        },
    )
}

/// Apply the primitive's zero/unit identities and independent nearest-cell search.
#[expect(
    clippy::float_cmp,
    reason = "zero and unit are exact primitive identities"
)]
fn rounded_product(left: f64, right: f64, b: &mut Budget) -> TestResult<Option<[f64; 2]>> {
    if left == 0.0 || right == 0.0 {
        return Ok(Some([0.0, 0.0]));
    }
    if left == 1.0 || right == 1.0 {
        return Ok(Some([left * right; 2]));
    }
    let q = represented(left, b)?.mul(&represented(right, b)?, b)?;
    let Some(bits) = oracles::nearest_positive_bits(&q, b)? else {
        return Ok(None);
    };
    if bits == 0x7fef_ffff_ffff_ffff {
        return Ok(None);
    }
    Ok(Some([
        if bits == 0 {
            -f64::from_bits(1)
        } else {
            f64::from_bits(bits - 1)
        },
        f64::from_bits(bits + 1),
    ]))
}

/// Outward root endpoints with the same proved zero/unit identities.
#[expect(
    clippy::float_cmp,
    reason = "zero and unit roots are exact primitive identities"
)]
fn rounded_root(value: f64, b: &mut Budget) -> TestResult<[f64; 2]> {
    if value == 0.0 || value == 1.0 {
        return Ok([value; 2]);
    }
    let bits = nearest_root_bits(&represented(value, b)?, b)?;
    Ok([
        if bits == 0 {
            -f64::from_bits(1)
        } else {
            f64::from_bits(bits - 1)
        },
        f64::from_bits(bits + 1),
    ])
}

/// Replay the two outward root endpoints independently.
fn oracle_root(image: [f64; 2], b: &mut Budget) -> TestResult<[f64; 2]> {
    Ok([rounded_root(image[0], b)?[0], rounded_root(image[1], b)?[1]])
}

/// Replay the square hull with a nonnegative lower endpoint.
fn oracle_square(image: [f64; 2], b: &mut Budget) -> TestResult<Option<[f64; 2]>> {
    let magnitude = if image[0] >= 0.0 {
        image
    } else if image[1] <= 0.0 {
        [-image[1], -image[0]]
    } else {
        [0.0, (-image[0]).max(image[1])]
    };
    let Some(lower) = rounded_product(magnitude[0], magnitude[0], b)? else {
        return Ok(None);
    };
    let Some(upper) = rounded_product(magnitude[1], magnitude[1], b)? else {
        return Ok(None);
    };
    Ok(Some([lower[0].max(0.0), upper[1]]))
}

/// Replay all four product corners and their nonnegative hull.
fn oracle_mul(left: [f64; 2], right: [f64; 2], b: &mut Budget) -> TestResult<Option<[f64; 2]>> {
    let mut lower = f64::INFINITY;
    let mut upper = f64::NEG_INFINITY;
    for (a, c) in [
        (left[0], right[0]),
        (left[0], right[1]),
        (left[1], right[0]),
        (left[1], right[1]),
    ] {
        let Some(pair) = rounded_product(a, c, b)? else {
            return Ok(None);
        };
        lower = lower.min(pair[0]);
        upper = upper.max(pair[1]);
    }
    Ok(Some([lower.max(0.0), upper]))
}

/// The final three rounded midpoint stages and two outward displacement subtractions.
/// # Panics
/// Panics if the reconstructed centre or displacement is not exactly binary64.
fn oracle_reading(image: [f64; 2], b: &mut Budget) -> TestResult<DyadicPowerOutput> {
    let lower = represented(image[0], b)?;
    let upper = represented(image[1], b)?;
    let difference = oracles::nearest(&upper.sub(&lower, b)?, b)?;
    let half = oracles::nearest(&difference.div(&number(2, 1)?, b)?, b)?;
    let center = oracles::nearest(&lower.add(&half, b)?, b)?;
    let mut displacement = Rational::zero();
    for endpoint in [&lower, &upper] {
        let difference = center.sub(endpoint, b)?;
        let outward = if difference.is_zero() {
            Rational::zero()
        } else {
            let nearest = oracles::nearest(&difference, b)?;
            let direction = difference.compare(&Rational::zero(), b)? != Ordering::Less;
            oracles::neighbour(&nearest, direction, b)?
        };
        let magnitude = oracles::absolute(&outward, b)?;
        if magnitude.compare(&displacement, b)? == Ordering::Greater {
            displacement = magnitude;
        }
    }
    let center_bits = oracles::nearest_positive_bits(&oracles::absolute(&center, b)?, b)?
        .ok_or("oracle center overflow")?;
    let error_bits =
        oracles::nearest_positive_bits(&displacement, b)?.ok_or("oracle displacement overflow")?;
    assert_eq!(oracles::value(center_bits, b)?, center);
    assert_eq!(oracles::value(error_bits, b)?, displacement);
    Ok(DyadicPowerOutput {
        endpoints: image,
        center: f64::from_bits(center_bits),
        total_radius: f64::from_bits(error_bits),
        numerical_error: f64::from_bits(error_bits),
    })
}

/// Replay one transition at a time; a failure records the first unfinished call.
#[expect(
    clippy::float_cmp,
    reason = "represented one is the exact validated shortcut"
)]
fn replay(
    x: f64,
    numerator: u64,
    roots: u32,
) -> TestResult<(Option<DyadicPowerOutput>, [usize; 3])> {
    if x == 1.0 {
        return Ok((
            Some(DyadicPowerOutput {
                endpoints: [1.0, 1.0],
                center: 1.0,
                total_radius: 0.0,
                numerical_error: 0.0,
            }),
            [0, 0, 0],
        ));
    }
    let mut b = ledger()?;
    let mut image = [x, x];
    let mut counts = [0, 0, 0];
    for _ in 0..roots {
        image = oracle_root(image, &mut b)?;
        counts[0] += 1;
    }
    let root = image;
    let bits = numerator.bit_width();
    for shift in (0..bits - 1).rev() {
        let Some(squared) = oracle_square(image, &mut b)? else {
            return Ok((None, counts));
        };
        image = squared;
        counts[1] += 1;
        if numerator >> shift & 1 != 0 {
            let Some(product) = oracle_mul(image, root, &mut b)? else {
                return Ok((None, counts));
            };
            image = product;
            counts[2] += 1;
        }
    }
    Ok((Some(oracle_reading(image, &mut b)?), counts))
}

/// Require bitwise equality of every native and reconstructed output field.
fn same_reading(native: DyadicPowerOutput, oracle: DyadicPowerOutput) {
    assert_eq!(
        native.endpoints.map(f64::to_bits),
        oracle.endpoints.map(f64::to_bits)
    );
    assert_eq!(native.center.to_bits(), oracle.center.to_bits());
    assert_eq!(native.total_radius.to_bits(), oracle.total_radius.to_bits());
    assert_eq!(
        native.numerical_error.to_bits(),
        oracle.numerical_error.to_bits()
    );
}

/// Exact rational replay fixes every root, product, midpoint and displacement stage,
/// with oracle end fixtures for maximal, overflowing and subnormal values.
/// # Panics
/// Panics if a successful replay differs bit for bit from native execution,
/// the underflow witness fails, the replay's first finite certification refusal
/// leaves `[63, 58, 58]`, the native refusal changes its name, or a rounding-cell
/// or end fixture fails. The observation test
/// (´test:integration:scalar-dyadic-power-observation´) pins the native prefix.
/// ´claim:matrix:scalar-dyadic-power-reconstruction´
/// ´test:integration:scalar-dyadic-power-reconstruction´
#[expect(
    clippy::too_many_lines,
    reason = "one selected replay test includes both finite-boundary oracle controls"
)]
#[test]
fn scalar_dyadic_power_reconstruction() -> TestResult {
    let context = ArithmeticContext::new()?;
    let cases = [
        (f64::MIN_POSITIVE, 1, 0),
        (2.0, 1, 0),
        (1.0, 3, 2),
        (0.25, 1, 1),
        (1.0 / 16.0, 1, 2),
        (1.0 / 256.0, 1, 3),
        (0.5, 3, 2),
        (0.5, 3, 4),
        (2.0, 7, 3),
        (f64::MIN_POSITIVE, 255, 8),
        (f64::MIN_POSITIVE.next_up(), 255, 8),
        (1.0_f64.next_down(), 1, 3),
        (1.0_f64.next_up(), 1, 3),
        (2.0_f64.next_down(), 1, 3),
        (f64::MIN_POSITIVE, (1_u64 << 52) - 1, 52),
        (2.0, i64::MAX as u64, 63),
    ];
    for (x, numerator, roots) in cases {
        let denominator = 1_u64 << roots;
        let limits = if roots == 52 {
            DyadicPowerLimits {
                exponent_bits: 52,
                root_steps: 52,
                product_steps: 102,
            }
        } else {
            LIMITS
        };
        let program = DyadicPowerProgram::new(
            [f64::MIN_POSITIVE, 2.0],
            i64::try_from(numerator)?,
            denominator,
            limits,
            &context,
        )?;
        let (expected, counts) = replay(x, numerator, roots)?;
        let actual = program.evaluate(x, &context);
        if let Some(oracle) = expected {
            let native = actual?;
            same_reading(native, oracle);
            if roots == 52 {
                assert!(native.endpoints[0] >= 0.0);
                assert!(native.endpoints[0] < f64::MIN_POSITIVE);
                assert_eq!(counts, [52, 51, 51]);
            }
        } else {
            assert_eq!(roots, 63);
            assert_eq!(counts, [63, 58, 58]);
            assert_eq!(
                actual.expect_err("finite upper certification refusal"),
                DyadicPowerFailure::TargetNotCertifiable {
                    failure: ArithmeticFailure::Overflow
                }
            );
        }
        writeln!(
            std::io::stdout(),
            "DYADIC-REPLAY x={x:?} m={numerator} k={roots} counts={counts:?} success={}",
            expected.is_some()
        )?;
    }
    let mut b = ledger()?;
    let one = Rational::one();
    for bits in [1.0_f64.to_bits(), 1.0_f64.to_bits() + 1] {
        let left = oracles::value(bits, &mut b)?;
        let right = oracles::value(bits + 1, &mut b)?;
        let midpoint = left.add(&right, &mut b)?.div(&number(2, 1)?, &mut b)?;
        let chosen = oracles::nearest_positive_bits(&midpoint, &mut b)?.ok_or("tie")?;
        assert_eq!(chosen, if bits & 1 == 0 { bits } else { bits + 1 });
        let square = midpoint.mul(&midpoint, &mut b)?;
        assert_eq!(nearest_root_bits(&square, &mut b)?, chosen);
    }
    assert_eq!(nearest_root_bits(&one, &mut b)?, 1.0_f64.to_bits());
    let maximum_bits = 0x7fef_ffff_ffff_ffff_u64;
    let maximum = oracles::value(maximum_bits, &mut b)?;
    assert_eq!(
        oracles::nearest_positive_bits(&maximum, &mut b)?,
        Some(maximum_bits)
    );
    let below_threshold = maximum.add(&power(969)?, &mut b)?;
    assert_eq!(
        oracles::nearest_positive_bits(&below_threshold, &mut b)?,
        Some(maximum_bits)
    );
    let threshold = power(1024)?.sub(&power(970)?, &mut b)?;
    assert_eq!(oracles::nearest_positive_bits(&threshold, &mut b)?, None);
    assert_eq!(
        oracles::nearest_positive_bits(&power(-1075)?, &mut b)?,
        Some(0)
    );
    let inward = oracles::value(maximum_bits - 1, &mut b)?;
    assert_eq!(oracles::neighbour(&maximum, false, &mut b)?, inward);
    assert!(oracles::neighbour(&maximum, true, &mut b).is_err());
    let negative_maximum = maximum.negated(&mut b)?;
    let negative_inward = inward.negated(&mut b)?;
    assert_eq!(
        oracles::neighbour(&negative_maximum, true, &mut b)?,
        negative_inward
    );
    assert!(oracles::neighbour(&negative_maximum, false, &mut b).is_err());
    Ok(())
}

/// Collect span names, fields and exact Work events for numerical observation.
#[derive(Clone, Default)]
struct PowerObserver {
    /// Next identifier assigned to a visited span.
    next: std::sync::Arc<std::sync::atomic::AtomicU64>,
    /// Span names keyed by their assigned identifiers.
    names: std::sync::Arc<std::sync::Mutex<std::collections::BTreeMap<u64, String>>>,
    /// Recorded span fields with their identifiers and names.
    fields: std::sync::Arc<std::sync::Mutex<Vec<(u64, String, String)>>>,
    /// Number of exact Work events seen by this subscriber.
    exact_events: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

/// Record debug field values for one visited span.
struct PowerFields {
    /// Identifier of the span being visited.
    id: u64,
    /// Shared ordered buffer of visited field values.
    fields: std::sync::Arc<std::sync::Mutex<Vec<(u64, String, String)>>>,
}

impl tracing::field::Visit for PowerFields {
    /// Save a debug field value with its span identifier.
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields.lock().expect("field visitor lock").push((
            self.id,
            field.name().to_owned(),
            format!("{value:?}"),
        ));
    }
}

impl tracing::Subscriber for PowerObserver {
    /// Enable every span and event during this local observation.
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    /// Assign an identifier and record a new span's initial fields.
    fn new_span(&self, span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        use std::sync::atomic::Ordering as AtomicOrdering;
        let id = self.next.fetch_add(1, AtomicOrdering::Relaxed) + 1;
        self.names
            .lock()
            .expect("span names lock")
            .insert(id, span.metadata().name().to_owned());
        span.record(&mut PowerFields {
            id,
            fields: self.fields.clone(),
        });
        tracing::span::Id::from_u64(id)
    }
    /// Record updated fields for an existing span.
    fn record(&self, span: &tracing::span::Id, values: &tracing::span::Record<'_>) {
        values.record(&mut PowerFields {
            id: span.into_u64(),
            fields: self.fields.clone(),
        });
    }
    /// Ignore causal links because these assertions inspect span fields.
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    /// Count events on the exact Work tracing target.
    fn event(&self, event: &tracing::Event<'_>) {
        use std::sync::atomic::Ordering as AtomicOrdering;
        if event.metadata().target() == "nrz::work" {
            self.exact_events.fetch_add(1, AtomicOrdering::Relaxed);
        }
    }
    /// Ignore span entry because this test reads recorded fields.
    fn enter(&self, _: &tracing::span::Id) {}
    /// Ignore span exit because this test reads recorded fields.
    fn exit(&self, _: &tracing::span::Id) {}
}

/// Return the latest recorded value of one field on one span.
fn field<'a>(fields: &'a [(u64, String, String)], id: u64, name: &str) -> Option<&'a str> {
    fields
        .iter()
        .rev()
        .find(|(span, key, _)| *span == id && key == name)
        .map(|(_, _, value)| value.as_str())
}

/// A visiting subscriber sees one input and outcome per call, actual bounded
/// prefix counts, and no exact Work in the numerical leaf.
/// # Panics
/// Panics if a span omits its original point, result or completed stage counts.
/// ´claim:matrix:scalar-dyadic-power-observation´
/// ´test:integration:scalar-dyadic-power-observation´
#[expect(
    clippy::too_many_lines,
    reason = "one selected observer checks lowering, validation and kernel fields"
)]
#[test]
fn scalar_dyadic_power_observation() -> TestResult {
    use std::sync::atomic::Ordering as AtomicOrdering;
    static DISPATCHERS: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = DISPATCHERS.lock().map_err(|_| "subscriber guard")?;
    let observer = PowerObserver::default();
    observer_dispatch::with_observer(observer.clone(), || -> TestResult {
        let context = ArithmeticContext::new()?;
        let ordinary = DyadicPowerProgram::new([f64::MIN_POSITIVE, 2.0], 3, 4, LIMITS, &context)?;
        let costly = DyadicPowerProgram::new(
            [f64::MIN_POSITIVE, 2.0],
            i64::MAX,
            1_u64 << 63,
            LIMITS,
            &context,
        )?;
        ordinary.evaluate(1.0, &context)?;
        ordinary.evaluate(0.5, &context)?;
        assert_eq!(
            costly.evaluate(2.0, &context).expect_err("refused prefix"),
            DyadicPowerFailure::TargetNotCertifiable {
                failure: ArithmeticFailure::Overflow
            }
        );
        Ok(())
    })?;
    let names = observer.names.lock().map_err(|_| "span names")?;
    let kernels: Vec<_> = names
        .iter()
        .filter(|(_, name)| name.as_str() == "nrz.kernel")
        .map(|(id, _)| *id)
        .collect();
    let lowerings: Vec<_> = names
        .iter()
        .filter(|(_, name)| name.as_str() == "nrz.lower")
        .map(|(id, _)| *id)
        .collect();
    let validations: Vec<_> = names
        .iter()
        .filter(|(_, name)| name.as_str() == "nrz.validation")
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(kernels.len(), 3);
    assert_eq!(lowerings.len(), 2);
    assert_eq!(validations.len(), 3);
    drop(names);
    let fields = observer.fields.lock().map_err(|_| "span fields")?;
    for (id, numerator, roots) in [
        (lowerings[0], "3", "2"),
        (lowerings[1], "9223372036854775807", "63"),
    ] {
        assert_eq!(field(&fields, id, "canonical_numerator"), Some(numerator));
        assert_eq!(field(&fields, id, "canonical_root_steps"), Some(roots));
        assert!(
            field(&fields, id, "result")
                .ok_or("lowering result")?
                .starts_with("Ok(")
        );
    }
    for (id, x) in validations.into_iter().zip(["1.0", "0.5", "2.0"]) {
        assert_eq!(field(&fields, id, "x"), Some(x));
        assert_eq!(field(&fields, id, "result"), Some("Ok(())"));
    }
    for (index, id) in kernels.into_iter().enumerate() {
        let expected_x = ["1.0", "0.5", "2.0"][index];
        assert_eq!(field(&fields, id, "x"), Some(expected_x));
        assert_eq!(
            fields
                .iter()
                .filter(|(span, name, _)| *span == id && name == "x")
                .count(),
            1
        );
        let outcome = field(&fields, id, "result").ok_or("kernel result")?;
        assert_eq!(
            fields
                .iter()
                .filter(|(span, name, _)| *span == id && name == "result")
                .count(),
            1
        );
        assert!(outcome.starts_with(if index == 2 { "Err(" } else { "Ok(" }));
        let mut counts = [0_usize; 5];
        for (slot, name) in counts.iter_mut().zip([
            "roots",
            "squares",
            "multiplies",
            "midpoint_steps",
            "displacement_steps",
        ]) {
            *slot = field(&fields, id, name).ok_or("stage count")?.parse()?;
        }
        match index {
            0 => assert_eq!(counts, [0, 0, 0, 0, 0]),
            1 => assert_eq!(counts, [2, 1, 1, 3, 2]),
            _ => assert_eq!(counts, [63, 58, 58, 0, 0]),
        }
        writeln!(
            std::io::stdout(),
            "DYADIC-OBSERVE x={expected_x} counts={counts:?} result={outcome}"
        )?;
    }
    assert_eq!(observer.exact_events.load(AtomicOrdering::Relaxed), 0);
    assert!(fields.iter().all(|(_, name, _)| !name.contains("owner")));
    drop(fields);
    Ok(())
}
