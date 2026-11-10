// SPDX-License-Identifier: AGPL-3.0-only
//! Independent small integer oracles and exact precision/source separation.

use crate::testing::common::{TestResult, default_budget};
use crate::testing::series_proof::{fixture, object, rational, step};
use nrz_cas::proof::Statement;
use nrz_cas::series::{
    SeriesBinaryRow, SeriesCertificate as C, SeriesObject, SeriesOperation as O,
    SeriesStatement as S,
};
use nrz_cas::syntax::{Draft, Operator};
use nrz_cas::{FaultKind, InvalidCode, Rational, Reference};

/// Closed integer formulas supply independent low-order oracles over positive, zero and negative inputs.
/// ´test:integration:series-small-polynomial-oracles´
#[test]
fn series_small_polynomial_oracles() -> TestResult {
    for a in -2..=2 {
        for b in -2..=2 {
            let mut f = fixture(0, false)?;
            let left = [a, 1 - a, a + 2].map(|v| v.to_string());
            let right = [b, b - 1, 0].map(|v| v.to_string());
            let input = object(&mut f.base, &f.field, &left.each_ref().map(String::as_str))?;
            let other = object(&mut f.base, &f.field, &right.each_ref().map(String::as_str))?;
            let values = [
                a * b,
                a * (b - 1) + (1 - a) * b,
                (1 - a) * (b - 1) + (a + 2) * b,
            ]
            .map(|v| v.to_string());
            let expected = object(
                &mut f.base,
                &f.field,
                &values.each_ref().map(String::as_str),
            )?;
            let witness = f.base.cas.compute_series(
                &O::Mul {
                    left: input,
                    right: other,
                    order: None,
                },
                &[],
                &mut default_budget()?,
            )?;
            let Statement::Series(value) = &witness.judgment().statement else {
                return Err("series".into());
            };
            let S::Mul {
                output,
                order,
                default_order,
                ..
            } = value.as_ref()
            else {
                return Err("product".into());
            };
            assert_eq!(output, &expected);
            assert_eq!(*order, 3);
            assert!(*default_order);
        }
    }
    Ok(())
}

/// Every result position is independently changed while all valid original arithmetic rows remain intact.
/// ´test:integration:series-every-output-position-refused´
#[test]
fn series_every_output_position_refused() -> TestResult {
    for kind in 0..16 {
        let count = match kind {
            0 | 5 | 6 | 8 | 12 | 15 => 1,
            4 | 7 | 14 => 2,
            10 | 13 => 4,
            _ => 3,
        };
        for index in 0..count {
            let mut f = fixture(kind, false)?;
            let root = f.steps.last().ok_or("root")?;
            let Statement::Series(value) = &root.conclusion else {
                return Err("series".into());
            };
            let target = match value.as_ref() {
                S::Source(_) | S::EvaluatedResidual { .. } => return Err("object fixture".into()),
                S::Reciprocal { output, .. }
                | S::Truncate { output, .. }
                | S::Add { output, .. }
                | S::Scale { output, .. }
                | S::Derive { output, .. }
                | S::Integrate { output, .. }
                | S::Compose { output, .. }
                | S::Mul { output, .. } => output,
                S::Coefficient { value, .. } => value,
            }
            .clone();
            let replacement = if matches!(kind, 5 | 6) {
                let (Operator::Rational(value), _) = f.base.cas.expression(&target)? else {
                    return Err("rational".into());
                };
                f.base.term(
                    Operator::Rational(value.add(&Rational::one(), &mut default_budget()?)?),
                    &[],
                )?
            } else {
                let object = f.base.cas.series_object(&target)?.clone();
                let (Operator::Rational(value), _) =
                    f.base.cas.expression(&object.coefficients[index])?
                else {
                    return Err("rational".into());
                };
                let changed = f.base.term(
                    Operator::Rational(value.add(&Rational::one(), &mut default_budget()?)?),
                    &[],
                )?;
                let mut coefficients = object
                    .coefficients
                    .into_iter()
                    .map(Reference::Existing)
                    .collect::<Vec<_>>();
                coefficients[index] = Reference::Existing(changed);
                f.base
                    .cas
                    .admit(
                        &[Draft::SeriesObject(SeriesObject {
                            field: Reference::Existing(object.field),
                            centre: Reference::Existing(object.centre),
                            variable: Reference::Existing(object.variable),
                            scope: object.scope,
                            context: Reference::Existing(object.context),
                            order: object.order,
                            coefficients,
                        })],
                        &mut default_budget()?,
                    )?
                    .pop()
                    .ok_or("changed output")?
            };
            let Statement::Series(value) = &mut f.steps.last_mut().ok_or("root")?.conclusion else {
                return Err("series".into());
            };
            match value.as_mut() {
                S::Source(_) | S::EvaluatedResidual { .. } => return Err("object fixture".into()),
                S::Reciprocal { output, .. }
                | S::Truncate { output, .. }
                | S::Add { output, .. }
                | S::Scale { output, .. }
                | S::Derive { output, .. }
                | S::Integrate { output, .. }
                | S::Compose { output, .. }
                | S::Mul { output, .. } => *output = replacement,
                S::Coefficient { value, .. } => *value = replacement,
            }
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            assert_eq!(
                f.base
                    .cas
                    .check_proof(&f.base.context, &f.steps, &mut default_budget()?)
                    .expect_err("altered output coefficient")
                    .kind,
                FaultKind::Invalid(InvalidCode::Rule)
            );
            assert_eq!(
                f.base.cas.canonical_snapshot(&mut default_budget()?)?,
                before
            );
        }
    }
    Ok(())
}

/// Explicit smaller order consumes both truncations in original operand order.
/// ´test:integration:series-explicit-common-truncation´
#[test]
fn series_explicit_common_truncation() -> TestResult {
    for change in 0..3 {
        let mut f = fixture(1, false)?;
        let Statement::Series(value) = &f.steps[0].conclusion else {
            return Err("series".into());
        };
        let S::Add { left, right, .. } = value.as_ref() else {
            return Err("add".into());
        };
        let (left, right) = (left.clone(), right.clone());
        let l = object(&mut f.base, &f.field, &["1", "2"])?;
        let r = object(&mut f.base, &f.field, &["4", "5"])?;
        let output = object(&mut f.base, &f.field, &["5", "7"])?;
        let mut steps = vec![
            step(
                &f.base,
                S::Truncate {
                    input: left.clone(),
                    output: l,
                },
                C::Truncate {
                    form: 0,
                    coefficients: vec![rational("1")?, rational("2")?],
                },
                vec![],
            ),
            step(
                &f.base,
                S::Truncate {
                    input: right.clone(),
                    output: r,
                },
                C::Truncate {
                    form: 0,
                    coefficients: vec![rational("4")?, rational("5")?],
                },
                vec![],
            ),
        ];
        let rows = [("1", "4", "5"), ("2", "5", "7")]
            .into_iter()
            .enumerate()
            .map(|(index, (a, b, c))| {
                Ok(SeriesBinaryRow {
                    index: u32::try_from(index)?,
                    left: rational(a)?,
                    right: rational(b)?,
                    result: rational(c)?,
                })
            })
            .collect::<TestResult<Vec<_>>>()?;
        steps.push(step(
            &f.base,
            S::Add {
                left,
                right,
                output,
                order: 2,
                default_order: false,
            },
            C::Add {
                form: 0,
                order: 2,
                default_order: false,
                rows,
            },
            if change == 1 { vec![1, 0] } else { vec![0, 1] },
        ));
        if change == 2 {
            steps[2].premises = vec![0];
            steps.remove(1);
        }
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        let result = f
            .base
            .cas
            .check_proof(&f.base.context, &steps, &mut default_budget()?);
        if change == 0 {
            assert!(result?.is_discharged());
        } else {
            assert!(result.is_err());
            assert_eq!(
                f.base.cas.canonical_snapshot(&mut default_budget()?)?,
                before
            );
        }
    }
    Ok(())
}

/// A valid analytic side-series theorem supplies neither truncation nor constant-definedness evidence.
/// ´test:integration:series-germ-premise-refused´
#[test]
fn series_germ_premise_refused() -> TestResult {
    for kind in [4, 7, 9] {
        let mut f = crate::testing::series_proof::production(kind)?;
        let at = crate::testing::germs::at(&mut f.base, "0", None)?;
        let request = f.base.request(
            crate::testing::germs::task(
                &at,
                nrz_cas::proof::GermMode::Series {
                    order: 1,
                    analytic: true,
                },
            ),
            &[f.base.x.clone()],
        )?;
        let nrz_cas::request::Outcome::Proved { result, .. } = f.base.cas.run(&request) else {
            return Err("checked germ".into());
        };
        assert!(matches!(
            result.witness.judgment().statement,
            Statement::Germ(_)
        ));
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.base
                .cas
                .compute_series(
                    &f.operation,
                    std::slice::from_ref(result.witness.proof()),
                    &mut default_budget()?
                )
                .err()
                .ok_or("germ accepted as a finite series premise")?
                .kind,
            FaultKind::Invalid(InvalidCode::Rule)
        );
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
        let bytes = f
            .base
            .cas
            .export_witness(&result.witness, &mut default_budget()?)?;
        assert!(matches!(
            f.base
                .cas
                .inspect(&bytes, &mut default_budget()?)?
                .judgment()
                .statement,
            Statement::Germ(_)
        ));
    }
    Ok(())
}
