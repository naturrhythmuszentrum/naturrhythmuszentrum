// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`compact_golden_associations`] | docgen | Exact binary64 inputs, row identities, targets and relative golden widths remain associated. Each row passes the unchanged bounded reader before its stronger coverage assertions. Every positive operand with endpoint evidence has a sigmoid upper endpoint above 1/2 or a tanh lower endpoint below the operand, excluding nonzero identity singletons. |
//! | [`compact_golden_cas_route`] | docgen | Every compact endpoint retains its independently replayed exact CAS interval. The generator's precision and schedule are provenance, not a native target promise. |
use nrz_cas::domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
use nrz_cas::enclosure::{EnclosureGoal, EnclosureRequest};
use nrz_cas::limits::{Budget, Limits};
use nrz_cas::syntax::{
    AtomicSort, Declaration, Draft, Environment, ExpressionDraft, Field, Operator, Sort,
};
use nrz_cas::{Cas, Fault, Handle, Rational, Reference};
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
fn ledger() -> Result<Budget, Fault> {
    Budget::new(Limits {
        work: 2_000_000_000,
        literal_bytes: 4096,
        integer_exponent: 4096,
        arithmetic_steps: 50_000_000,
        name_bytes: 131_072,
        source_bytes: 4_194_304,
        output_bytes: 4_194_304,
        witness_bytes: 4_194_304,
        ..Limits::default()
    })
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
                literal_bytes: 4096,
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

fn payload() -> TestResult<serde_json::Value> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/golden/prepared-binary64.json");
    Ok(nrz_docgen::golden::read(
        std::fs::File::open(path)?,
        nrz_docgen::golden::Limits::default(),
        &mut ledger()?,
    )?
    .value()
    .clone())
}

fn binary64(bits: u64) -> TestResult<String> {
    let fraction = bits & ((1_u64 << 52) - 1);
    let exponent = (bits >> 52) & 2047;
    let significand = if exponent == 0 {
        fraction
    } else {
        fraction + (1_u64 << 52)
    };
    let power = if exponent == 0 {
        -1074
    } else {
        i64::try_from(exponent)? - 1075
    };
    let mut b = ledger()?;
    let value = Rational::from_ratio(i64::try_from(significand)?, 1, &mut b)?.mul(
        &Rational::from_ratio(2, 1, &mut b)?.pow(power, &mut b)?,
        &mut b,
    )?;
    Ok(value.exact_string(&mut b)?)
}

fn selected(row: &serde_json::Value) -> bool {
    row["id"].as_str().is_some_and(|id| {
        (id.starts_with("sigma-") && id != "sigma-ln-two-box") || id.starts_with("tanh-")
    })
}

/// Exact binary64 inputs, row identities, targets and relative golden widths remain associated.
/// Each row passes the unchanged bounded reader before its stronger coverage assertions.
/// Every positive operand with endpoint evidence has a sigmoid upper endpoint above 1/2
/// or a tanh lower endpoint below the operand, excluding nonzero identity singletons.
/// ´claim:docgen:compact-golden-associations´
/// ´test:integration:compact-golden-associations´
#[test]
fn compact_golden_associations() -> TestResult {
    use serde_json::json;
    let mut all = payload()?;
    let rows = all["entries"].as_array().ok_or("entries")?.clone();
    let mut expected = std::collections::BTreeMap::new();
    for (op, upper) in [("sigma", 2.0_f64), ("tanh", 1.0)] {
        let cases = [
            ("zero", vec![0], false),
            ("upper", vec![upper.to_bits()], false),
            ("upper-predecessor", vec![upper.to_bits() - 1], false),
            ("zero-broad", vec![0, upper.to_bits()], false),
            ("zero-narrow", vec![0, 983_u64 << 52], false),
            ("zero-normal-box", vec![0, 1_u64 << 52], false),
            ("near-zero", vec![963_u64 << 52], false),
            ("underflow-scale", vec![423_u64 << 52], false),
            ("normal-minimum", vec![1_u64 << 52], false),
            ("normal-successor", vec![(1_u64 << 52) + 1], false),
            ("subnormal-maximum", vec![(1_u64 << 52) - 1], true),
            ("subnormal-minimum", vec![1], true),
        ];
        for (name, bits, refusal) in cases {
            let values = bits
                .into_iter()
                .map(binary64)
                .collect::<TestResult<Vec<_>>>()?;
            let input = if values.len() == 1 {
                json!({"point":values[0]})
            } else {
                json!({"box":values})
            };
            expected.insert(
                format!("{op}-{name}"),
                (op, input, 40, refusal.then_some("below-normal-operand")),
            );
        }
        expected.insert(
            format!("{op}-outside-upper"),
            (
                op,
                json!({"point":binary64(upper.to_bits()+1)?}),
                40,
                Some("outside-domain"),
            ),
        );
        expected.insert(
            format!("{op}-target-spacing"),
            (
                op,
                json!({"point":binary64(upper.to_bits())?}),
                128,
                Some("target-not-certifiable"),
            ),
        );
    }
    // Authored order reaches the checker unchanged; only the expectation lookup is sorted.
    for row in rows.iter().filter(|row| selected(row)) {
        let id = row["id"].as_str().ok_or("id")?;
        let (op, input, bits, refusal) = expected.remove(id).ok_or("unexpected row identity")?;
        assert_eq!(row["operation"], op, "operation association: {id}");
        assert_eq!(row["input"], input, "input association: {id}");
        assert_eq!(
            row["target"],
            json!({"kind":if op=="sigma" {"absolute"} else {"relative"},"bits":bits}),
            "target association: {id}"
        );
        assert_eq!(
            row["expected"],
            refusal.map_or_else(|| json!("enclosure"), |reason| json!({"refusal":reason})),
            "outcome association: {id}"
        );
        all["entries"] = json!([row]);
        let bytes = serde_json::to_vec(&all)?;
        let mut budget = Budget::new(Limits {
            work: 7_889_442,
            literal_bytes: 618,
            ..Limits::default()
        })?;
        let checked = nrz_docgen::golden::read(
            bytes.as_slice(),
            nrz_docgen::golden::Limits::default(),
            &mut budget,
        )?;
        assert_eq!(checked.value()["entries"][0], *row);
        relative_width(row, op, id)?;
    }
    assert!(expected.is_empty(), "missing row identities");
    spacing_obstruction(&rows)?;
    analytic_ranges(&rows)?;
    Ok(())
}

fn analytic_ranges(rows: &[serde_json::Value]) -> TestResult {
    for row in rows {
        let op = row["operation"].as_str().ok_or("operation")?;
        if !matches!(op, "sigma" | "tanh") {
            continue;
        }
        for evidence in row["evidence"].as_array().ok_or("evidence")? {
            let mut b = ledger()?;
            let x = Rational::parse(evidence["input"].as_str().ok_or("input")?, &mut b)?;
            if x.compare(&Rational::zero(), &mut b)? != std::cmp::Ordering::Greater {
                continue;
            }
            let name = evidence["endpoint"].as_str().ok_or("endpoint")?;
            let pair = row["golden"][name].as_array().ok_or("interval")?;
            // For x > 0, tanh(x) < x and sigma(x) > 1/2. Finite golden
            // intervals may straddle these bounds, but cannot miss the strict range.
            let (endpoint, bound, order) = if op == "tanh" {
                (0, x, std::cmp::Ordering::Less)
            } else {
                (
                    1,
                    Rational::from_ratio(1, 2, &mut b)?,
                    std::cmp::Ordering::Greater,
                )
            };
            let value = Rational::parse(pair[endpoint].as_str().ok_or("endpoint")?, &mut b)?;
            assert_eq!(
                value.compare(&bound, &mut b)?,
                order,
                "strict analytic range: {} {name}",
                row["id"]
            );
        }
    }
    Ok(())
}

fn relative_width(row: &serde_json::Value, op: &str, id: &str) -> TestResult {
    for evidence in row["evidence"].as_array().ok_or("evidence")? {
        let mut b = ledger()?;
        let x = Rational::parse(evidence["input"].as_str().ok_or("input")?, &mut b)?;
        let name = evidence["endpoint"].as_str().ok_or("endpoint")?;
        let pair = row["golden"][name].as_array().ok_or("interval")?;
        let lo = Rational::parse(pair[0].as_str().ok_or("lower")?, &mut b)?;
        let hi = Rational::parse(pair[1].as_str().ok_or("upper")?, &mut b)?;
        if op == "tanh" && !x.is_zero() {
            let bound = x.mul(
                &Rational::from_ratio(2, 1, &mut b)?.pow(-120, &mut b)?,
                &mut b,
            )?;
            assert!(
                hi.sub(&lo, &mut b)?.compare(&bound, &mut b)? != std::cmp::Ordering::Greater,
                "relative golden width: {id}"
            );
        }
    }
    Ok(())
}

fn spacing_obstruction(rows: &[serde_json::Value]) -> TestResult {
    // These adjacent binary64 values bracket the whole independent golden interval.
    // Every possible binary64 endpoint is at least as far away. Both images are
    // below one, so the absolute obstruction also implies tanh's relative one.
    for (op, below) in [
        ("sigma", 0x3fec_2f7d_5a8a_79ca),
        ("tanh", 0x3fe8_5efa_b514_f394),
    ] {
        let row = rows
            .iter()
            .find(|r| r["id"] == format!("{op}-upper"))
            .ok_or("upper")?;
        let mut b = ledger()?;
        let lo = Rational::parse(row["golden"]["point"][0].as_str().ok_or("lower")?, &mut b)?;
        let hi = Rational::parse(row["golden"]["point"][1].as_str().ok_or("upper")?, &mut b)?;
        let down = Rational::parse(&binary64(below)?, &mut b)?;
        let up = Rational::parse(&binary64(below + 1)?, &mut b)?;
        let ceiling = Rational::from_ratio(2, 1, &mut b)?.pow(-128, &mut b)?;
        assert_eq!(
            hi.compare(&Rational::one(), &mut b)?,
            std::cmp::Ordering::Less
        );
        assert_eq!(
            lo.sub(&down, &mut b)?.compare(&ceiling, &mut b)?,
            std::cmp::Ordering::Greater,
            "lower spacing: {op}"
        );
        assert_eq!(
            up.sub(&hi, &mut b)?.compare(&ceiling, &mut b)?,
            std::cmp::Ordering::Greater,
            "upper spacing: {op}"
        );
    }
    Ok(())
}

/// Every compact endpoint retains its independently replayed exact CAS interval.
/// The generator's precision and schedule are provenance, not a native target promise.
/// ´claim:docgen:compact-golden-cas-route´
/// ´test:integration:compact-golden-cas-route´
#[test]
fn compact_golden_cas_route() -> TestResult {
    let all = payload()?;
    let mut seen = std::collections::BTreeMap::new();
    for row in all["entries"]
        .as_array()
        .ok_or("entries")?
        .iter()
        .filter(|row| selected(row))
    {
        let op = row["operation"].as_str().ok_or("operation")?;
        for evidence in row["evidence"].as_array().ok_or("evidence")? {
            let text = evidence["input"].as_str().ok_or("input")?;
            let key = format!("{op}:{text}");
            if !seen.contains_key(&key) {
                let mut f = Fixture::new()?;
                let x = f.term(
                    Operator::Rational(Rational::parse(text, &mut ledger()?)?),
                    &[],
                )?;
                let target = f.term(
                    if op == "sigma" {
                        Operator::Sigmoid
                    } else {
                        Operator::Tanh
                    },
                    &[x],
                )?;
                let policy = nrz_cas::enclosure::RefinementPolicy {
                    version: 1,
                    precision: 160,
                    grid_ceiling: 256,
                    rounds: 1,
                    reduction_ceiling: 4,
                    term_ceiling: 128,
                    checking_work: 0,
                };
                let report =
                    f.cas
                        .enclose(&f.request(target), policy, &mut ledger()?, &mut |_| false)?;
                let checked = report.checked.ok_or("CAS enclosure")?;
                let mut fresh = Fixture::new()?;
                let x = fresh.term(
                    Operator::Rational(Rational::parse(text, &mut ledger()?)?),
                    &[],
                )?;
                let target = fresh.term(
                    if op == "sigma" {
                        Operator::Sigmoid
                    } else {
                        Operator::Tanh
                    },
                    &[x],
                )?;
                let replay = fresh.cas.check_enclosure(
                    &fresh.request(target),
                    policy,
                    checked.certificate(),
                    &mut ledger()?,
                    &mut |_| false,
                )?;
                assert_eq!(
                    replay.checked.ok_or("CAS replay")?.interval(),
                    checked.interval()
                );
                seen.insert(
                    key.clone(),
                    serde_json::json!([
                        checked.interval().lower().exact_string(&mut ledger()?)?,
                        checked.interval().upper().exact_string(&mut ledger()?)?
                    ]),
                );
            }
            assert_eq!(
                evidence["routes"][0]["interval"], seen[&key],
                "CAS route: {}",
                row["id"]
            );
        }
    }
    assert_eq!(seen.len(), 16, "distinct compact endpoints");
    Ok(())
}
