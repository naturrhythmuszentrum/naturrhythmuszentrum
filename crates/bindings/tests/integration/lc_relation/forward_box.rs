// SPDX-License-Identifier: AGPL-3.0-only
//! Independent rational four-corner oracle for conditional signed-time coverage.

use super::Q;
use super::support::{TestResult, input, ledger, number};
use nrz_bindings::{Budget, LcInput, LcRelation, Limits};
use nrz_cas::Rational;

fn rational(text: &str) -> TestResult<Rational> {
    Ok(Rational::parse(text, &mut ledger()?)?)
}

fn interval(quantity: Q, lower: &str, upper: &str) -> TestResult<LcInput> {
    input(quantity, Some(number(lower)?), Some(number(upper)?))
}

fn box_relation() -> TestResult<LcRelation> {
    Ok(LcRelation::new(
        "signed box epoch".into(),
        0,
        vec![
            interval(Q::Inductance, "1", "4")?,
            interval(Q::Capacitance, "1", "2")?,
            interval(Q::InitialPhase, "1", "2")?,
            interval(Q::Inductance, "2", "3")?,
        ],
        &mut ledger()?,
    )?)
}

/// The expected corners come from `[1/3,1] * [-3,-1]`, independently of the builder.
#[test]
fn lc_box_forward_signed_four_products() -> TestResult {
    let relation = box_relation()?;
    let time = interval(Q::Time(0), "-3", "-1")?;
    let cover =
        relation.box_forward_cover(&time, [rational("1")?, rational("3")?], &mut ledger()?)?;
    assert!(std::ptr::eq(cover.relation(), &raw const relation));
    assert!(std::ptr::eq(cover.time(), &raw const time));
    assert_eq!(cover.rate(), &[rational("1/3")?, rational("1")?]);
    assert_eq!(cover.lifted(), &[rational("-2")?, rational("5/3")?]);
    assert_eq!(cover.bracket(), &[rational("1")?, rational("3")?]);
    Ok(())
}

/// Zero crossing uses the crossed corners `1*(-2)` and `1*3`.
#[test]
fn lc_box_forward_crossing_four_products() -> TestResult {
    let relation = box_relation()?;
    let time = interval(Q::Time(0), "-2", "3")?;
    let cover =
        relation.box_forward_cover(&time, [rational("1")?, rational("3")?], &mut ledger()?)?;
    assert_eq!(cover.lifted(), &[rational("-1")?, rational("5")?]);
    Ok(())
}

/// A positive time interval retains the ordinary first/last corner order.
#[test]
fn lc_box_forward_positive_four_products() -> TestResult {
    let relation = box_relation()?;
    let time = interval(Q::Time(0), "1", "2")?;
    let cover =
        relation.box_forward_cover(&time, [rational("1")?, rational("3")?], &mut ledger()?)?;
    assert_eq!(cover.lifted(), &[rational("4/3")?, rational("4")?]);
    Ok(())
}

/// A bracket must contain the repeated-position product range `[2,6]`.
#[test]
fn lc_box_forward_bracket_refusals() -> TestResult {
    let relation = box_relation()?;
    let time = interval(Q::Time(0), "-3", "1")?;
    for bracket in [
        ["3/2", "3"],
        ["1", "2"],
        ["0", "3"],
        ["-1", "3"],
        ["3", "1"],
    ] {
        assert!(
            relation
                .box_forward_cover(
                    &time,
                    [rational(bracket[0])?, rational(bracket[1])?],
                    &mut ledger()?
                )
                .is_err()
        );
    }
    Ok(())
}

/// Unsupported physical constraints remain original and cause a scoped refusal.
#[test]
fn lc_box_forward_original_constraint_refusals() -> TestResult {
    let base = box_relation()?;
    let time = interval(Q::Time(0), "-3", "1")?;
    let bracket = [rational("1")?, rational("3")?];
    for extra in [
        interval(Q::Energy, "1", "2")?,
        interval(Q::Rate, "1/3", "1")?,
    ] {
        let mut inputs = base.inputs().to_vec();
        inputs.push(extra);
        let relation = LcRelation::new(base.epoch().into(), 0, inputs, &mut ledger()?)?;
        assert!(
            relation
                .box_forward_cover(&time, bracket.clone(), &mut ledger()?)
                .is_err()
        );
    }
    let unbounded = input(Q::Capacitance, None, Some(number("2")?))?;
    let mut inputs = base.inputs().to_vec();
    inputs.push(unbounded);
    let relation = LcRelation::new(base.epoch().into(), 0, inputs, &mut ledger()?)?;
    assert!(
        relation
            .box_forward_cover(&time, bracket, &mut ledger()?)
            .is_err()
    );
    Ok(())
}

/// Exact and one-below original ledgers include all four temporary products.
#[test]
fn lc_box_forward_resource_boundaries() -> TestResult {
    let relation = box_relation()?;
    let time = interval(Q::Time(0), "-2", "3")?;
    let bracket = [rational("1")?, rational("3")?];
    let mut measured = ledger()?;
    let cover = relation.box_forward_cover(&time, bracket.clone(), &mut measured)?;
    let work = measured.usage().work;
    let bytes = cover.retained_payload_bytes();
    assert!(work > 0 && bytes > 0);
    let mut exact = Budget::new(Limits {
        work,
        output_bytes: bytes,
        ..Limits::default()
    })?;
    relation.box_forward_cover(&time, bracket.clone(), &mut exact)?;
    assert_eq!(exact.usage().work, work);
    let mut short_work = Budget::new(Limits {
        work: work - 1,
        ..Limits::default()
    })?;
    assert!(
        relation
            .box_forward_cover(&time, bracket.clone(), &mut short_work)
            .is_err()
    );
    assert!(short_work.usage().work > 0 && short_work.usage().work < work);
    let mut short_bytes = Budget::new(Limits {
        output_bytes: bytes - 1,
        ..Limits::default()
    })?;
    assert!(
        relation
            .box_forward_cover(&time, bracket, &mut short_bytes)
            .is_err()
    );
    Ok(())
}

/// A nonpositive element cannot enter the positive-oscillation product law.
#[test]
fn lc_box_forward_nonpositive_elements() -> TestResult {
    let time = interval(Q::Time(0), "1", "2")?;
    for (l_lower, l_upper, c_lower, c_upper) in [("-4", "-1", "-2", "-1"), ("1", "2", "-2", "-1")] {
        let relation = LcRelation::new(
            "nonpositive".into(),
            0,
            vec![
                interval(Q::Inductance, l_lower, l_upper)?,
                interval(Q::Capacitance, c_lower, c_upper)?,
                interval(Q::InitialPhase, "0", "1")?,
            ],
            &mut ledger()?,
        )?;
        assert!(
            relation
                .box_forward_cover(&time, [rational("1")?, rational("3")?], &mut ledger()?)
                .is_err()
        );
    }
    Ok(())
}

/// Intersections are used even when the second declaration is wider or shifted.
#[test]
fn lc_box_forward_repeated_intersections() -> TestResult {
    let time = interval(Q::Time(0), "1", "2")?;
    for (first, second) in [(("2", "3"), ("1", "4")), (("1", "3"), ("2", "4"))] {
        let relation = LcRelation::new(
            "repeated".into(),
            0,
            vec![
                interval(Q::Inductance, first.0, first.1)?,
                interval(Q::Inductance, second.0, second.1)?,
                interval(Q::Capacitance, "1", "2")?,
                interval(Q::InitialPhase, "0", "1")?,
            ],
            &mut ledger()?,
        )?;
        let cover = relation.box_forward_cover(
            &time,
            [rational("1")?, rational("5/2")?],
            &mut ledger()?,
        )?;
        assert_eq!(cover.rate(), &[rational("2/5")?, rational("1")?]);
        assert_eq!(cover.lifted(), &[rational("2/5")?, rational("3")?]);
    }
    Ok(())
}
