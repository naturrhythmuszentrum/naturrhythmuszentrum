// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed-order calculus, exact unit rows and unavailable precision retain distinct contracts.

use crate::testing::common::{TestResult, default_budget};
use crate::testing::series_proof::{defined, fixture, object, rational};
use nrz_cas::proof::{Certificate, Statement};
use nrz_cas::series::{SeriesCertificate as C, SeriesOperation as O, SeriesStatement as S};
use nrz_cas::syntax::{Constant, Operator};
use nrz_cas::{FaultKind, Handle, InvalidCode};

fn endpoints(statement: &Statement) -> TestResult<(Handle, Handle)> {
    let Statement::Series(value) = statement else {
        return Err("series".into());
    };
    match value.as_ref() {
        S::Derive { input, output } | S::Integrate { input, output, .. } => {
            Ok((input.clone(), output.clone()))
        }
        _ => Err("calculus".into()),
    }
}

/// Round trips compare coefficients only and register no source or residual identity theorem.
/// ´test:integration:series-calculus-coefficient-roundtrips´
#[test]
fn series_calculus_coefficient_roundtrips() -> TestResult {
    for kind in 7..12 {
        let mut f = fixture(kind, false)?;
        let (input, output) = endpoints(&f.expected.statement)?;
        f.base
            .cas
            .check_proof(&f.base.context, &f.steps, &mut default_budget()?)?;
        let (operation, proofs) = if kind < 9 {
            let constant = f
                .base
                .cas
                .series_object(&input, &mut crate::testing::budgets::graph_budget()?)?
                .coefficients[0]
                .clone();
            let fact = defined(&mut f.base, &constant)?;
            let proof = f
                .base
                .cas
                .check_proof(&f.base.context, &[fact], &mut default_budget()?)?;
            (
                O::Integrate {
                    input: output,
                    constant,
                },
                vec![proof.proof().clone()],
            )
        } else {
            (O::Derive { input: output }, Vec::new())
        };
        let witness = f
            .base
            .cas
            .compute_series(&operation, &proofs, &mut default_budget()?)?;
        let (_, recovered) = endpoints(&witness.judgment().statement)?;
        assert_eq!(
            f.base
                .cas
                .series_object(&input, &mut crate::testing::budgets::graph_budget()?)?
                .coefficients,
            f.base
                .cas
                .series_object(&recovered, &mut crate::testing::budgets::graph_budget()?)?
                .coefficients
        );
        assert_eq!(
            f.base
                .cas
                .series_object(&input, &mut crate::testing::budgets::graph_budget()?)?
                .order,
            f.base
                .cas
                .series_object(&recovered, &mut crate::testing::budgets::graph_budget()?)?
                .order
        );
    }
    Ok(())
}

/// Complete ascending rows cannot omit zeros, change integer units or reuse a neighboring row.
/// ´test:integration:series-calculus-row-mutations´
#[test]
fn series_calculus_row_mutations() -> TestResult {
    for kind in [7, 9, 11] {
        for change in 0..9 {
            let mut f = fixture(kind, false)?;
            let original =
                f.base
                    .cas
                    .check_proof(&f.base.context, &f.steps, &mut default_budget()?)?;
            let bytes = f
                .base
                .cas
                .export_witness(&original, &mut default_budget()?)?;
            let Certificate::Series(certificate) =
                &mut f.steps.last_mut().ok_or("root")?.certificate
            else {
                return Err("certificate".into());
            };
            match certificate.as_mut() {
                C::Derive { rows, .. } => match change {
                    0 => {
                        rows.pop();
                    }
                    1 => rows.push(rows[0].clone()),
                    2 => rows.swap(0, 1),
                    3 => rows[0].index = 1,
                    4 => rows[0].input = rational("9")?,
                    5 | 7 | 8 => rows[0].multiplier = rational("9")?,
                    _ => rows[0].result = rational("9")?,
                },
                C::Integrate { rows, .. } => match change {
                    0 => {
                        rows.pop();
                    }
                    1 => rows.push(rows[0].clone()),
                    2 => rows.swap(0, 1),
                    3 => rows[0].index = 1,
                    4 => rows[0].input = rational("9")?,
                    5 => rows[0].integer = rational("2")?,
                    6 => rows[0].result = rational("9")?,
                    7 => rows[0].inverse = rational("0")?,
                    _ => rows[0].inverse = rational("2")?,
                },
                _ => return Err("calculus certificate".into()),
            }
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            assert_eq!(
                f.base
                    .cas
                    .check_proof(&f.base.context, &f.steps, &mut default_budget()?)
                    .expect_err("false row")
                    .kind,
                FaultKind::Invalid(InvalidCode::Rule)
            );
            assert_eq!(
                f.base.cas.canonical_snapshot(&mut default_budget()?)?,
                before
            );
            assert_eq!(
                f.base
                    .cas
                    .export_witness(&original, &mut default_budget()?)?,
                bytes
            );
        }
    }
    Ok(())
}

/// Rational capability and coordinate independence remain different fault categories.
/// ´test:integration:series-integral-constant-capabilities´
#[test]
fn series_integral_constant_capabilities() -> TestResult {
    for change in 0..3 {
        let mut f = crate::testing::series_proof::production(9)?;
        let constant = match change {
            0 => f.base.term(Operator::Constant(Constant::Pi), &[])?,
            1 => f.base.x.clone(),
            _ => f
                .base
                .term(Operator::Mul, &[f.base.zero.clone(), f.base.x.clone()])?,
        };
        let O::Integrate { constant: slot, .. } = &mut f.operation else {
            return Err("integral".into());
        };
        *slot = constant;
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.base
                .cas
                .compute_series(&f.operation, &f.proofs, &mut default_budget()?)
                .expect_err("unavailable constant")
                .kind,
            FaultKind::Invalid(if change == 0 {
                InvalidCode::SeriesUnsupported
            } else {
                InvalidCode::Scope
            })
        );
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
    }
    Ok(())
}

/// Precision changes are exact; explicit restriction precedes calculus and extraction stops at its endpoint.
/// ´test:integration:series-calculus-precision-boundaries´
#[test]
fn series_calculus_precision_boundaries() -> TestResult {
    let mut f = fixture(7, false)?;
    let (input, _) = endpoints(&f.expected.statement)?;
    let restricted = f.base.cas.compute_series(
        &O::Truncate { input, order: 2 },
        &[],
        &mut default_budget()?,
    )?;
    let Statement::Series(value) = &restricted.judgment().statement else {
        return Err("series".into());
    };
    let S::Truncate { output, .. } = value.as_ref() else {
        return Err("restriction".into());
    };
    let derivative = f.base.cas.compute_series(
        &O::Derive {
            input: output.clone(),
        },
        &[],
        &mut default_budget()?,
    )?;
    let (_, output) = endpoints(&derivative.judgment().statement)?;
    let expected = object(&mut f.base, &f.field, &["2"])?;
    assert_eq!(output, expected);
    f.base.cas.compute_series(
        &O::Coefficient {
            input: output.clone(),
            index: 0,
        },
        &[],
        &mut default_budget()?,
    )?;
    for operation in [
        O::Coefficient {
            input: output.clone(),
            index: 1,
        },
        O::Derive { input: output },
    ] {
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.base
                .cas
                .compute_series(&operation, &[], &mut default_budget()?)
                .expect_err("unavailable precision")
                .kind,
            FaultKind::Invalid(InvalidCode::SeriesUnsupported)
        );
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
    }
    Ok(())
}

/// Supplied order-one derivative claims refuse in checking and fresh portable replay.
/// ´test:integration:series-calculus-order-one-supplied-certificate´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Direct checking and both fresh replay orders retain asserted wire offsets and rollback together"
)]
fn series_calculus_order_one_supplied_certificate() -> TestResult {
    use crate::testing::answer::{index, records};
    use nrz_cas::proof::{ApplicationDraft, Primitive};
    use nrz_cas::series::SeriesDerivativeRow;

    let mut source = fixture(8, false)?;
    let positive =
        source
            .base
            .cas
            .check_proof(&source.base.context, &source.steps, &mut default_budget()?)?;
    let bytes = source
        .base
        .cas
        .export_witness(&positive, &mut default_budget()?)?;
    let (_, one) = endpoints(&source.expected.statement)?;
    assert_eq!(
        source
            .base
            .cas
            .series_object(&one, &mut crate::testing::budgets::graph_budget()?)?
            .order,
        1
    );
    let supplied = ApplicationDraft {
        rule: Primitive::SeriesDerive,
        context: source.base.context.clone(),
        conclusion: Statement::Series(Box::new(S::Derive {
            input: one.clone(),
            output: one,
        })),
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Series(Box::new(C::Derive {
            form: 0,
            rows: vec![SeriesDerivativeRow {
                index: 0,
                input: rational("0")?,
                multiplier: rational("1")?,
                result: rational("0")?,
            }],
        })),
    };
    let before = source.base.cas.canonical_snapshot(&mut default_budget()?)?;
    assert_eq!(
        source
            .base
            .cas
            .check_proof(&source.base.context, &[supplied], &mut default_budget()?)
            .expect_err("unavailable checker precision")
            .kind,
        FaultKind::Invalid(InvalidCode::SeriesUnsupported)
    );
    assert_eq!(
        source.base.cas.canonical_snapshot(&mut default_budget()?)?,
        before
    );
    assert_eq!(
        source
            .base
            .cas
            .export_witness(&positive, &mut default_budget()?)?,
        bytes
    );

    let inventory = records(&bytes)?;
    let (_, statement) = inventory
        .iter()
        .find(|(tag, _)| *tag == 39)
        .ok_or("statement")?;
    let mut cursor = statement.start + 10;
    assert_eq!(index(&bytes, &mut cursor)?, 13);
    assert_eq!(&bytes[cursor..cursor + 13], b"series.derive");
    cursor += 13;
    assert_eq!(index(&bytes, &mut cursor)?, 1);
    index(&bytes, &mut cursor)?;
    assert_eq!(&bytes[cursor..cursor + 2], &[8, 0]);
    let input_at = cursor + 2;
    let input = index(&bytes, &mut (cursor + 2))?;
    assert_eq!(bytes[cursor + 6], 0);
    let output = index(&bytes, &mut (cursor + 7))?;
    assert_ne!(input, output);
    assert_eq!(inventory[usize::try_from(output)?].0, 38);
    let mut changed = bytes.clone();
    changed[input_at..input_at + 4].copy_from_slice(&output.to_le_bytes());
    for reordered in [false, true] {
        let mut receiver = fixture(8, reordered)?;
        let (_, one) = endpoints(&receiver.expected.statement)?;
        let mut expected = receiver.expected.clone();
        expected.statement = Statement::Series(Box::new(S::Derive {
            input: one.clone(),
            output: one,
        }));
        let before = receiver
            .base
            .cas
            .canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            receiver
                .base
                .cas
                .replay(&changed, &expected, &mut default_budget()?)
                .expect_err("unavailable replay precision")
                .kind,
            FaultKind::Invalid(InvalidCode::SeriesUnsupported)
        );
        assert_eq!(
            receiver
                .base
                .cas
                .canonical_snapshot(&mut default_budget()?)?,
            before
        );
        receiver
            .base
            .cas
            .replay(&bytes, &receiver.expected, &mut default_budget()?)?;
    }
    Ok(())
}

/// Extraction reaches exactly the last changed-order coefficient and never an inferred tail.
/// ´test:integration:series-calculus-output-extraction-endpoints´
#[test]
fn series_calculus_output_extraction_endpoints() -> TestResult {
    for kind in [7, 9, 10] {
        let mut f = fixture(kind, false)?;
        let witness = f
            .base
            .cas
            .check_proof(&f.base.context, &f.steps, &mut default_budget()?)?;
        let (_, output) = endpoints(&witness.judgment().statement)?;
        let object = f
            .base
            .cas
            .series_object(&output, &mut crate::testing::budgets::graph_budget()?)?;
        let order = object.order;
        let expected = object
            .coefficients
            .last()
            .ok_or("last coefficient")?
            .clone();
        let last = f.base.cas.compute_series(
            &O::Coefficient {
                input: output.clone(),
                index: order - 1,
            },
            &[],
            &mut default_budget()?,
        )?;
        let Statement::Series(statement) = &last.judgment().statement else {
            return Err("coefficient judgment".into());
        };
        let S::Coefficient { value, .. } = statement.as_ref() else {
            return Err("coefficient result".into());
        };
        assert_eq!(value, &expected);
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.base
                .cas
                .compute_series(
                    &O::Coefficient {
                        input: output,
                        index: order
                    },
                    &[],
                    &mut default_budget()?
                )
                .expect_err("unknown tail")
                .kind,
            FaultKind::Invalid(InvalidCode::SeriesUnsupported)
        );
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
        f.base
            .cas
            .export_witness(&witness, &mut default_budget()?)?;
    }
    Ok(())
}

/// A valid proof of the wrong kind and surplus ordinary slots cannot become constant authority.
/// ´test:integration:series-integral-typed-premise-slots´
#[test]
fn series_integral_typed_premise_slots() -> TestResult {
    let mut f = crate::testing::series_proof::production(9)?;
    let O::Integrate { input, .. } = &f.operation else {
        return Err("integral".into());
    };
    let wrong = f.base.cas.compute_series(
        &O::Truncate {
            input: input.clone(),
            order: 1,
        },
        &[],
        &mut default_budget()?,
    )?;
    for proofs in [
        Vec::new(),
        vec![wrong.proof().clone()],
        vec![f.proofs[0].clone(), f.proofs[0].clone()],
    ] {
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.base
                .cas
                .compute_series(&f.operation, &proofs, &mut default_budget()?)
                .expect_err("wrong slots")
                .kind,
            FaultKind::Invalid(InvalidCode::Rule)
        );
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
    }
    Ok(())
}

/// Definedness on a structurally different region cannot stand in for the required input region.
/// ´test:integration:series-integral-region-premise-join´
#[test]
fn series_integral_region_premise_join() -> TestResult {
    use nrz_cas::Reference;
    use nrz_cas::domain::{AmbientDomain, RegionDraft, RegionExpr};
    use nrz_cas::syntax::Draft;
    let mut f = crate::testing::series_proof::production(9)?;
    let O::Integrate { constant, .. } = &f.operation else {
        return Err("integral".into());
    };
    let constant = constant.clone();
    let original = f.base.domain.clone();
    f.base.domain = f
        .base
        .cas
        .admit(
            &[Draft::Region(RegionDraft {
                header: Reference::Existing(f.base.header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Real("x".into()),
                expression: RegionExpr::Meet(vec![Reference::Existing(original.clone())]),
            })],
            &mut default_budget()?,
        )?
        .pop()
        .ok_or("region twin")?;
    assert_ne!(f.base.domain, original);
    let fact = defined(&mut f.base, &constant)?;
    let checked = f
        .base
        .cas
        .check_proof(&f.base.context, &[fact], &mut default_budget()?)?;
    assert!(checked.is_discharged());
    f.base.domain = original;
    let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
    assert_eq!(
        f.base
            .cas
            .compute_series(
                &f.operation,
                std::slice::from_ref(checked.proof()),
                &mut default_budget()?
            )
            .expect_err("wrong required region")
            .kind,
        FaultKind::Invalid(InvalidCode::Rule)
    );
    assert_eq!(
        f.base.cas.canonical_snapshot(&mut default_budget()?)?,
        before
    );
    f.base
        .cas
        .export_witness(&checked, &mut default_budget()?)?;
    f.base
        .cas
        .compute_series(&f.operation, &f.proofs, &mut default_budget()?)?;
    Ok(())
}

/// Every semantic axis survives calculus; unsupported fields and malformed scopes refuse at admission.
/// ´test:integration:series-calculus-metadata-twins´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Each complete mutation retains its setup, precise fault and rollback evidence together"
)]
fn series_calculus_metadata_twins() -> TestResult {
    use nrz_cas::Reference;
    use nrz_cas::domain::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
    use nrz_cas::series::SeriesObject;
    use nrz_cas::solving::{CoefficientDomainData, SolveDraft};
    use nrz_cas::syntax::{AtomicSort, Draft, Field};
    for kind in [7, 9] {
        for change in 0..5 {
            let mut f = fixture(kind, false)?;
            let (_, output) = endpoints(&f.expected.statement)?;
            let original = f
                .base
                .cas
                .series_object(&output, &mut crate::testing::budgets::graph_budget()?)?
                .clone();
            let mut value = SeriesObject {
                field: Reference::Existing(original.field),
                centre: Reference::Existing(original.centre),
                variable: Reference::Existing(original.variable),
                scope: original.scope,
                context: Reference::Existing(original.context),
                order: original.order,
                coefficients: original
                    .coefficients
                    .into_iter()
                    .map(Reference::Existing)
                    .collect(),
            };
            match change {
                0 => value.centre = Reference::Existing(f.base.one.clone()),
                1 => {
                    let nodes = f.base.cas.admit(
                        &[
                            Draft::Region(RegionDraft {
                                header: Reference::Existing(f.base.header.clone()),
                                scope: Vec::new(),
                                ambient: AmbientDomain::Real("x".into()),
                                expression: RegionExpr::Meet(vec![Reference::Existing(
                                    f.base.domain.clone(),
                                )]),
                            }),
                            Draft::Context(ContextDraft::Root {
                                header: Reference::Existing(f.base.header.clone()),
                                admissible: Reference::Local(0),
                                assumptions: Vec::new(),
                            }),
                        ],
                        &mut default_budget()?,
                    )?;
                    value.context = Reference::Existing(nodes[1].clone());
                }
                2 => {
                    value.order += 1;
                    value
                        .coefficients
                        .push(Reference::Existing(f.base.zero.clone()));
                }
                3 => {
                    let field = f
                        .base
                        .cas
                        .admit(
                            &[Draft::Solve(SolveDraft::CoefficientDomain(
                                CoefficientDomainData::Gaussian,
                            ))],
                            &mut default_budget()?,
                        )?
                        .pop()
                        .ok_or("field")?;
                    value.field = Reference::Existing(field);
                }
                _ => value.scope = vec![vec![AtomicSort::Scalar(Field::Real)]],
            }
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            let admitted = f
                .base
                .cas
                .admit(&[Draft::SeriesObject(value)], &mut default_budget()?);
            if change >= 3 {
                assert_eq!(
                    admitted.expect_err("unsupported field or scope").kind,
                    FaultKind::Invalid(if change == 3 {
                        InvalidCode::SeriesUnsupported
                    } else {
                        InvalidCode::Scope
                    })
                );
                assert_eq!(
                    f.base.cas.canonical_snapshot(&mut default_budget()?)?,
                    before
                );
                continue;
            }
            let output = admitted?.pop().ok_or("twin")?;
            let Statement::Series(statement) = &mut f.steps.last_mut().ok_or("root")?.conclusion
            else {
                return Err("series".into());
            };
            match statement.as_mut() {
                S::Derive { output: slot, .. } | S::Integrate { output: slot, .. } => {
                    *slot = output;
                }
                _ => return Err("calculus".into()),
            }
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            assert_eq!(
                f.base
                    .cas
                    .check_proof(&f.base.context, &f.steps, &mut default_budget()?)
                    .expect_err("metadata twin")
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
