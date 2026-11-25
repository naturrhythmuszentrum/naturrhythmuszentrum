// SPDX-License-Identifier: AGPL-3.0-only
//! Infinite alias coverage is tested separately from finite numerical member materialization.

use super::super::support::{TestResult, integer, ledger, number, pi_certificate};
use super::support;
use crate::kernel_oracle::Rational as Oracle;
use nrz_bindings::readings::algebra::Rational;
use nrz_bindings::{
    Budget, LcAliasCover, LcAliasLattice, LcCertainty, LcDeterminacy, LcPairParameters,
    LcQuantity as Q, Limits, LimitsOverride,
};
use nrz_matrix::LcAliasProjection;
use nrz_scalar::{ArithmeticContext, Enclosure, LiteralLimits, Scalar};
use std::cmp::Ordering;

/// The primitive timestamp lattice retains every nonnegative alias in both numerical formats.
/// ´test:integration:lc-relation-aliases´
#[test]
fn lc_relation_aliases() -> TestResult {
    aliases::<f32>()?;
    aliases::<f64>()
}

/// Independently bounded Machin pi and exact rational arithmetic supply member endpoint oracles.
fn aliases<P: Scalar + 'static>() -> TestResult {
    let context = ArithmeticContext::new()?;
    let pi = super::super::super::pi_conversion::pi::<P>(&context, &mut ledger()?)?;
    for known in [false, true] {
        let exact = cover(known)?;
        assert_eq!(exact.certainty(), LcCertainty::Certified);
        assert_eq!(
            exact.determinacy(Q::Product)?,
            LcDeterminacy::DeterminedInfinite
        );
        assert_eq!(
            exact.determinacy(Q::Inductance)?,
            LcDeterminacy::Underdetermined
        );
        assert_eq!(exact.first_index(), number("0")?);
        assert_eq!(exact.last_index(), None);
        assert_eq!(
            exact.cycle_steps(),
            &[number("10")?, number("25")?, number("32")?]
        );
        assert_eq!(exact.rate_step(&mut ledger()?)?, number("20000")?);
        assert!(
            exact
                .relation()
                .inputs()
                .iter()
                .all(|input| !matches!(input.quantity(), Q::Rate | Q::AngularFrequency))
        );
        let native = LcAliasProjection::<P>::new(exact, pi.clone(), &mut ledger()?)?;
        assert!(native.pi().same_region(&pi));
        let endpoints = if known {
            [(15875, 8), (16125, 8)]
        } else {
            [(21500, 11), (22500, 11)]
        };
        for index in [0, 1, 17] {
            let member = native.branch(
                integer(index)?,
                LiteralLimits::default(),
                &context,
                &mut ledger()?,
            )?;
            assert_eq!(member.index(), &integer(index)?);
            let shifts = super::super::super::pi_conversion::oracle::factor(1, 1, false, 52)?;
            for (i, (top, bottom)) in endpoints.into_iter().enumerate() {
                for shift in shifts {
                    let coefficient = integer(20000 * index)?;
                    let shift = rational(shift)?.mul(&coefficient, &mut ledger()?)?;
                    let value = rational(Oracle::new(top, bottom)?)?.add(&shift, &mut ledger()?)?;
                    contains_exact(member.rate_endpoints()[i], &value)?;
                    let product =
                        Rational::one().div(&value.pow(2, &mut ledger()?)?, &mut ledger()?)?;
                    contains_exact(member.product_endpoints()[1 - i], &product)?;
                }
            }
        }
    }
    Ok(())
}

/// Arbitrary exact indices remain available beyond any machine materialization count.
/// ´test:integration:lc-relation-alias-witnesses-and-domain´
#[test]
fn lc_relation_alias_witnesses_and_domain() -> TestResult {
    let exact = cover(false)?;
    let parameters = LcPairParameters {
        rate: number("2000")?,
        initial_phase: number("0")?,
        ratio: number("50")?,
        voltage: number("5")?,
        gauge: number("0")?,
    };
    for index in [0, 1, 17] {
        let point = exact.witness(&integer(index)?, &parameters, &mut ledger()?)?;
        assert!(point.relation().shares_allocation(exact.relation()));
        for (observation, (base, step)) in
            point
                .values()
                .observations
                .iter()
                .zip([(0, 10), (0, 25), (1, 32)])
        {
            assert_eq!(
                observation.cycle.as_ref(),
                Some(&integer(base + step * index)?)
            );
        }
    }
    assert!(
        exact
            .rate_endpoints(&number("18446744073709551616")?, &mut ledger()?)
            .is_ok()
    );
    for index in ["-1", "1/2"] {
        assert!(
            exact
                .rate_endpoints(&number(index)?, &mut ledger()?)
                .is_err()
        );
    }
    let mut inputs = support::inputs(false, "1/20")?;
    inputs.retain(|input| input.quantity() != Q::AngularFrequency);
    let relation = support::relation(inputs)?;
    let bad = LcAliasLattice {
        quantum: number("1/20000")?,
        bezout: [number("3")?, number("-2")?],
    };
    assert!(
        relation
            .alias_cover(
                vec![integer(0)?, integer(0)?, integer(1)?],
                bad,
                &pi_certificate()?,
                &mut ledger()?
            )
            .is_err()
    );
    Ok(())
}

/// Omitted frequency support stays absent; the original three exact timestamp gaps define aliases.
fn cover(known: bool) -> TestResult<LcAliasCover> {
    let mut inputs = support::inputs(known, "1/20")?;
    inputs.retain(|input| input.quantity() != Q::AngularFrequency);
    let lattice = LcAliasLattice {
        quantum: number("1/10000")?,
        bezout: [number("3")?, number("-2")?],
    };
    Ok(support::relation(inputs)?.alias_cover(
        vec![integer(0)?, integer(0)?, integer(1)?],
        lattice,
        &pi_certificate()?,
        &mut ledger()?,
    )?)
}

/// The independent small rational oracle exports exact integers, never rounded decimal truth.
fn rational(value: Oracle) -> TestResult<Rational> {
    number(&format!("{}/{}", value.numerator, value.denominator))
}

/// Independent binary decoding and arbitrary-precision comparisons avoid squaring i128 bounds.
fn contains_exact<P: Scalar>(enclosure: Enclosure<P>, value: &Rational) -> TestResult {
    let lower = rational(Oracle::binary(enclosure.lower().get().to_f64())?)?;
    let upper = rational(Oracle::binary(enclosure.upper().get().to_f64())?)?;
    assert_ne!(lower.compare(value, &mut ledger()?)?, Ordering::Greater);
    assert_ne!(upper.compare(value, &mut ledger()?)?, Ordering::Less);
    Ok(())
}

/// Literal work boundaries include all base checking and exact alias transformation work.
/// Index zero uses the same map, simplifying its zero shift and unit factor exactly.
/// ´test:integration:lc-relation-alias-witness-work´
#[test]
fn lc_relation_alias_witness_work() -> TestResult {
    // Unchanged operation Work 571950/917934/918824 plus local-entry terms respectively:
    // 206+69 kinds + 241 identity + 344 vertices = 860;
    // 336+102 + 372 + 540 = 1350; 340+102 + 374 + 544 = 1360.
    const ROWS: [(i64, u64); 3] = [(0, 572_707), (1, 919_116), (17, 920_014)];
    let exact = cover(false)?;
    let parameters = LcPairParameters {
        rate: number("2000")?,
        initial_phase: number("0")?,
        ratio: number("50")?,
        voltage: number("5")?,
        gauge: number("0")?,
    };
    for (index, work) in ROWS {
        let index = integer(index)?;
        let mut measured = ledger()?;
        exact.witness(&index, &parameters, &mut measured)?;
        assert_eq!(measured.usage().work, work);
        for (limit, success) in [(work + 17, true), (work + 16, false)] {
            let limits = Limits::default().resolve(&LimitsOverride {
                work: Some(limit),
                ..LimitsOverride::default()
            })?;
            let mut budget = Budget::new(limits)?;
            budget.work(
                17,
                nrz_bindings::readings::algebra::Location::new("alias prefix"),
            )?;
            assert_eq!(
                exact.witness(&index, &parameters, &mut budget).is_ok(),
                success
            );
            assert!(budget.usage().work >= 17);
        }
    }
    Ok(())
}

/// Scaling only one element does not satisfy the retained original-equation preservation map.
/// ´test:integration:lc-relation-alias-transform-mutation-refusals´
#[test]
fn lc_relation_alias_transform_mutation_refusals() -> TestResult {
    let exact = cover(false)?;
    let parameters = LcPairParameters {
        rate: number("2000")?,
        initial_phase: number("0")?,
        ratio: number("50")?,
        voltage: number("5")?,
        gauge: number("0")?,
    };
    let point = exact.witness(&integer(1)?, &parameters, &mut ledger()?)?;
    point.check_transformed_values(point.values(), &mut ledger()?)?;
    let mut only_inductance = point.base_witness().values().clone();
    only_inductance.inductance = point.values().inductance.clone();
    assert!(
        point
            .check_transformed_values(&only_inductance, &mut ledger()?)
            .is_err()
    );
    let mut wrong_energy = point.values().clone();
    wrong_energy.energy = point.base_witness().values().energy.clone();
    assert!(
        point
            .check_transformed_values(&wrong_energy, &mut ledger()?)
            .is_err()
    );
    Ok(())
}
