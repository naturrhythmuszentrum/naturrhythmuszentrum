// SPDX-License-Identifier: AGPL-3.0-only
//! Exact-rational fixture construction and adversarial solve expectations.

use std::cmp::Ordering;

use nrz_matrix::{DenseSystem, certified_solve};
use nrz_scalar::{Interval, frobenius_upper};
use serde_json::Value;

/// A reduced rational used only to construct fixture inputs.
#[derive(Clone, Copy)]
struct Rational {
    numerator: i128,
    denominator: i128,
}

impl Rational {
    /// Reduce a rational with positive denominator.
    const fn new(numerator: i128, denominator: i128) -> Self {
        let mut a = numerator.abs();
        let mut b = denominator;
        while b != 0 {
            let next = a % b;
            a = b;
            b = next;
        }
        Self {
            numerator: numerator / a,
            denominator: denominator / a,
        }
    }
    /// Exact rational addition.
    const fn add(self, rhs: Self) -> Self {
        let mut a = self.denominator;
        let mut b = rhs.denominator;
        while b != 0 {
            let next = a % b;
            a = b;
            b = next;
        }
        let left = rhs.denominator / a;
        let right = self.denominator / a;
        Self::new(
            self.numerator * left + rhs.numerator * right,
            self.denominator * left,
        )
    }
    /// Exact rational multiplication.
    const fn mul(self, rhs: Self) -> Self {
        Self::new(
            self.numerator * rhs.numerator,
            self.denominator * rhs.denominator,
        )
    }
}

/// Round the fixture rational to binary64 using integer long division.
///
/// TODO ´todo:test:retain-rational-import-representation-radii-as´: retain rational-import representation radii as computational uncertainty with their own descriptors under rule:vision:uncertainty-unit-algebra.
fn nearest(value: Rational) -> (f64, f64) {
    if value.numerator == 0 {
        return (0.0, 0.0);
    }
    let mut scaled_num = value.numerator.abs();
    let mut scaled_den = value.denominator;
    let mut exponent = 0_i32;
    while scaled_num < scaled_den {
        scaled_num *= 2;
        exponent -= 1;
    }
    while scaled_num >= scaled_den * 2 {
        scaled_den *= 2;
        exponent += 1;
    }
    let mut remainder = scaled_num - scaled_den;
    let mut mantissa = 1_u64 << 52;
    for bit in (0..52).rev() {
        remainder *= 2;
        if remainder >= scaled_den {
            remainder -= scaled_den;
            mantissa |= 1_u64 << bit;
        }
    }
    let exact = remainder == 0;
    let twice = remainder * 2;
    if twice > scaled_den || (twice == scaled_den && mantissa & 1 != 0) {
        mantissa += 1;
    }
    if mantissa == 1_u64 << 53 {
        mantissa >>= 1;
        exponent += 1;
    }
    let biased = u64::try_from(exponent + 1023).expect("normal fixture value");
    let bits = (biased << 52) | (mantissa - (1_u64 << 52));
    let positive = f64::from_bits(bits);
    let center = if value.numerator < 0 {
        -positive
    } else {
        positive
    };
    let radius = if exact {
        0.0
    } else {
        ((center.next_up() - center).max(center - center.next_down())) / 2.0
    };
    (center, radius)
}

/// Import exact fixture entries with zero or half-ulp representation radii.
///
/// TODO ´todo:test:construct-quantity-carrying-fixtures-with-declared´: construct quantity-carrying fixtures with declared leaf measures, block quotients and intrinsic input and computational companions under rule:vision:heterogeneous-assembly and def:vision:intrinsic-uncertainty-companions.
///
/// TODO ´todo:test:import-fixture-entries-with-logical-axes´: import fixture entries with logical axes and shaped representation bounds under def:nrz:matrix-shape and def:nrz:numerical-error-shapes.
///
/// TODO ´todo:test:construct-equivalent-checked-arena-layouts-for´: construct equivalent checked arena layouts for each exact fixture under req:nrz:computation-implementation-evidence.
fn system(entries: &[Rational], n: usize) -> DenseSystem {
    let mut a = Vec::with_capacity(entries.len());
    let mut a_errors = Vec::with_capacity(entries.len());
    let mut b = Vec::with_capacity(n);
    let mut b_errors = Vec::with_capacity(n);
    for row in 0..n {
        let mut exact_sum = Rational::new(0, 1);
        for col in 0..n {
            let entry = entries[row * n + col];
            exact_sum = exact_sum.add(entry);
            let (center, radius) = nearest(entry);
            a.push(center);
            a_errors.push(radius);
        }
        let (center, radius) = nearest(exact_sum);
        b.push(center);
        b_errors.push(radius);
    }
    let a_radius = frobenius_upper(&a_errors).expect("finite matrix import");
    let b_radius = frobenius_upper(&b_errors).expect("finite RHS import");
    DenseSystem::new(n, a, a_radius, b, b_radius).expect("fixture shape")
}

/// Hilbert entries are exact unit fractions.
///
/// TODO ´todo:test:declare-hilbert-entries-as-numerical-ratio´: declare Hilbert entries as numerical ratio quantities and order as structural metadata in typed fixtures under rule:vision:semantic-kinds-and-roles and rule:vision:counts-and-metadata.
///
/// TODO ´todo:test:declare-hilbert-symmetry-with-explicit-center´: declare Hilbert symmetry with explicit center and perturbation scope under def:nrz:shape-structure.
fn hilbert(n: usize) -> DenseSystem {
    let entries = (0..n)
        .flat_map(|row| {
            (0..n).map(move |col| {
                Rational::new(1, i128::try_from(row + col + 1).expect("small order"))
            })
        })
        .collect::<Vec<_>>();
    system(&entries, n)
}

/// A Kahan upper triangle has rational powers of s.
///
/// TODO ´todo:test:check-kahan-coefficient-and-angular-kinds´: check Kahan coefficient and angular kinds separately from order metadata in quantity-carrying cases under rule:vision:semantic-kinds-and-roles and rule:vision:counts-and-metadata.
///
/// TODO ´todo:test:declare-kahan-triangular-zeros-with-checked´: declare Kahan triangular zeros with checked entry descriptors and uncertainty scope under rule:nrz:shape-entry-contract and def:nrz:shape-structure.
fn kahan(n: usize, c: Rational, s: Rational) -> DenseSystem {
    let mut entries = Vec::with_capacity(n * n);
    let mut scale = Rational::new(1, 1);
    for row in 0..n {
        for col in 0..n {
            let entry = match col.cmp(&row) {
                Ordering::Less => Rational::new(0, 1),
                Ordering::Equal => scale,
                Ordering::Greater => scale.mul(Rational::new(-c.numerator, c.denominator)),
            };
            entries.push(entry);
        }
        scale = scale.mul(s);
    }
    system(&entries, n)
}

/// Wilkinson's partial-pivoting growth matrix has only integer entries.
///
/// TODO ´todo:test:keep-integer-valued-wilkinson-entries-distinct´: keep integer-valued Wilkinson entries distinct from physical counts in typed fixture cases under def:vision:counts and rule:vision:semantic-kinds-and-roles.
///
/// TODO ´todo:test:retain-wilkinson-structural-zeros-without-inferring´: retain Wilkinson structural zeros without inferring absent uncertainty from stored centers under def:nrz:shape-structure.
fn wilkinson(n: usize) -> DenseSystem {
    let entries = (0..n)
        .flat_map(|row| {
            (0..n).map(move |col| {
                let value = if col + 1 == n || col == row {
                    1
                } else if row > col {
                    -1
                } else {
                    0
                };
                Rational::new(value, 1)
            })
        })
        .collect::<Vec<_>>();
    system(&entries, n)
}

/// Admit a metadata order or coherent ratio before applying a numerical fixture recipe.
fn parameter(entry: &Value, variant: &Value, name: &str) -> Rational {
    let original = entry["parameters"]
        .as_array()
        .expect("parameters")
        .iter()
        .find(|value| value["name"].as_str() == Some(name))
        .expect("parameter declaration");
    let text = variant["overrides"][name]
        .as_str()
        .unwrap_or_else(|| original["value"].as_str().expect("parameter value"));
    let value = if let Some((a, b)) = text.split_once('/') {
        Rational::new(
            a.parse().expect("numerator"),
            b.parse().expect("denominator"),
        )
    } else {
        Rational::new(text.parse().expect("integer"), 1)
    };
    assert_eq!(original["uncertainty"]["kind"], "none");
    if matches!(name, "n" | "n_kahan" | "n_wilkinson") {
        assert_eq!(original["role"], "metadata");
        assert!(original.get("descriptor").is_none());
        assert_eq!(original["metadata"]["kind"], "integer_range");
        assert_eq!(value.denominator, 1);
        let minimum = original["metadata"]["minimum"].as_i64().expect("minimum");
        assert!(value.numerator >= i128::from(minimum));
        if let Some(maximum) = original["metadata"]["maximum"].as_i64() {
            assert!(value.numerator <= i128::from(maximum));
        }
    } else {
        assert_eq!(original["role"], "quantity");
        let descriptor = &original["descriptor"];
        assert_eq!(
            descriptor["dimensions"],
            serde_json::json!(["0", "0", "0", "0", "0", "0", "0"])
        );
        assert_eq!(descriptor["affine_role"], "difference");
        assert_eq!(descriptor["measure"]["scale"], "1");
        assert_eq!(descriptor["measure"]["offset"], "0");
        let kind = entry["kinds"]
            .as_array()
            .expect("kinds")
            .iter()
            .find(|kind| kind["name"] == descriptor["kind"])
            .expect("declared kind");
        assert_eq!(kind["type"], "ratio");
        assert_eq!(kind["dimensions"], descriptor["dimensions"]);
    }
    value
}

/// Construct the system named by a fixture variant.
///
/// TODO ´todo:test:validate-variant-selectors-as-metadata-and´: validate variant selectors as metadata and preserve quantity descriptors through fixture dispatch under rule:vision:counts-and-metadata and req:vision:algebraic-unit-checks.
///
/// TODO ´todo:test:dispatch-fixture-variants-with-declared-axes´: dispatch fixture variants with declared axes and independently shaped companions under def:nrz:matrix-shape and def:nrz:companion-shape.
///
/// TODO ´todo:test:bind-fixture-structure-changes-only-through´: bind fixture structure changes only through declared plan parameters or variants under rule:nrz:batch-variation.
fn variant_system(entry: &Value, variant: &Value) -> DenseSystem {
    let name = variant["name"].as_str().expect("variant name");
    if name.starts_with("order") {
        hilbert(usize::try_from(parameter(entry, variant, "n").numerator).expect("order"))
    } else if name.starts_with("kahan") {
        kahan(
            usize::try_from(parameter(entry, variant, "n_kahan").numerator).expect("order"),
            parameter(entry, variant, "c"),
            parameter(entry, variant, "s"),
        )
    } else {
        wilkinson(
            usize::try_from(parameter(entry, variant, "n_wilkinson").numerator).expect("order"),
        )
    }
}

/// Every success encloses the all-ones solution and every variant meets its expectation.
///
/// TODO ´todo:test:verify-adversarial-outcomes-with-independently-typed´: verify adversarial outcomes with independently typed radii, residuals, inverse bounds and ratio budgets after measure conversion under def:vision:certified-solve-contract and rule:vision:uncertainty-unit-algebra.
///
/// TODO ´todo:test:test-descriptor-preserving-fixture-solves-and´: test descriptor preserving fixture solves and packed structure norms against independent enclosures under rule:nrz:structured-companion-coordinates and rule:nrz:shape-certified-solve.
///
/// TODO ´todo:test:verify-mixed-batch-successes-and-refusals´: verify mixed batch successes and refusals retain original point order under rule:nrz:batch-outcomes.
///
/// ´test:integration:adversarial-matrix-fixtures´
#[test]
fn adversarial_matrix_fixtures() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/datasets/problems.json");
    let data: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("dataset")).expect("JSON");
    for entry in data["entries"].as_array().expect("entries") {
        let slug = entry["problem"]["slug"].as_str().expect("slug");
        if slug != "hilbert-matrices" && slug != "kahan-and-wilkinson-matrices" {
            continue;
        }
        for variant in entry["variants"].as_array().expect("variants") {
            let name = variant["name"].as_str().expect("name");
            if !name.ends_with("-success") && !name.ends_with("-refusal") {
                continue;
            }
            let input = variant_system(entry, variant);
            let budget_rational = parameter(entry, variant, "relative_budget");
            let budget = nearest(budget_rational).0;
            let result = certified_solve(&input, budget);
            if let Ok(outcome) = &result {
                let errors = outcome
                    .candidate
                    .iter()
                    .map(|&value| {
                        let bound = Interval::point(value)
                            .expect("candidate")
                            .checked_sub(Interval::point(1.0).expect("one"))
                            .expect("difference");
                        bound.lo().abs().max(bound.hi().abs())
                    })
                    .collect::<Vec<_>>();
                let distance = frobenius_upper(&errors).expect("distance");
                assert!(distance <= outcome.radius, "false success for {name}");
            }
            if name.ends_with("-success") {
                assert!(result.is_ok(), "expected success for {name}: {result:?}");
            } else {
                assert!(result.is_err(), "expected refusal for {name}");
            }
        }
    }
}
