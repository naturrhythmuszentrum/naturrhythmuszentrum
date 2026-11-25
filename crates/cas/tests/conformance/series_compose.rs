// SPDX-License-Identifier: AGPL-3.0-only
//! Independent object composition and complete malformed-evidence controls.

use crate::testing::common::{TestResult, default_budget};
use crate::testing::series_compose::{fixture, operation};
use crate::testing::series_proof::rational;
use nrz_cas::proof::Certificate;
use nrz_cas::series::SeriesCertificate as C;
use nrz_cas::{FaultKind, InvalidCode};

/// Identical nonzero rational centres admit increment composition; a centre twin cannot transport it.
/// ´test:integration:compose-centred-identity´
#[test]
fn compose_centred_identity() -> TestResult {
    use nrz_cas::proof::Statement;
    use nrz_cas::series::{SeriesObject, SeriesStatement as S};
    use nrz_cas::{Reference, syntax::Draft};
    for identical in [false, true] {
        let mut f = fixture(&["1", "1"], &["0", "1"], &["1", "1"], true, false)?;
        let Statement::Series(s) = &f.steps[0].conclusion else {
            return Err("series".into());
        };
        let S::Compose {
            left,
            right,
            output,
            ..
        } = s.as_ref()
        else {
            return Err("compose".into());
        };
        let originals = [left.clone(), right.clone(), output.clone()];
        let mut changed = Vec::new();
        for (index, handle) in originals.iter().enumerate() {
            let object = f
                .base
                .cas
                .series_object(handle, &mut crate::testing::budgets::graph_budget()?)?
                .clone();
            changed.push(
                f.base
                    .cas
                    .admit(
                        &[Draft::SeriesObject(SeriesObject {
                            field: Reference::Existing(object.field),
                            centre: Reference::Existing(if identical || index == 1 {
                                f.base.one.clone()
                            } else {
                                object.centre
                            }),
                            variable: Reference::Existing(object.variable),
                            scope: object.scope,
                            context: Reference::Existing(object.context),
                            order: object.order,
                            coefficients: object
                                .coefficients
                                .into_iter()
                                .map(Reference::Existing)
                                .collect(),
                        })],
                        &mut default_budget()?,
                    )?
                    .pop()
                    .ok_or("centred object")?,
            );
        }
        let Statement::Series(s) = &mut f.steps[0].conclusion else {
            return Err("series".into());
        };
        let S::Compose {
            left,
            right,
            output,
            ..
        } = s.as_mut()
        else {
            return Err("compose".into());
        };
        *left = changed[0].clone();
        *right = changed[1].clone();
        *output = changed[2].clone();
        f.expected.statement = f.steps[0].conclusion.clone();
        for production in [false, true] {
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            let result = if production {
                f.base
                    .cas
                    .compute_series(&operation(&f)?, &[], &mut default_budget()?)
            } else {
                f.base
                    .cas
                    .check_proof(&f.base.context, &f.steps, &mut default_budget()?)
            };
            if identical {
                result?;
            } else {
                assert_eq!(
                    result.expect_err("centre mismatch").kind,
                    FaultKind::Invalid(InvalidCode::Rule)
                );
                assert_eq!(
                    before,
                    f.base.cas.canonical_snapshot(&mut default_budget()?)?
                );
            }
        }
    }
    Ok(())
}

/// Hand polynomial expansions cover constants, nonunits, zero linear terms and order one.
/// ´test:integration:compose-direct-expansion´
#[test]
fn compose_direct_expansion() -> TestResult {
    for (p, q, expected) in [
        (
            &["0", "1", "1", "0"][..],
            &["0", "1", "-1", "0"][..],
            &["0", "1", "0", "-2"][..],
        ),
        (
            &["1", "2", "3", "0"][..],
            &["0", "2", "0", "0"][..],
            &["1", "4", "12", "0"][..],
        ),
        (
            &["1", "1", "1", "0", "0"][..],
            &["0", "0", "1", "0", "0"][..],
            &["1", "0", "1", "0", "1"][..],
        ),
        (&["7"][..], &["0"][..], &["7"][..]),
        (
            &["1/2", "2/3", "0"][..],
            &["0", "3/4", "0"][..],
            &["1/2", "1/2", "0"][..],
        ),
        (
            &["1", "2", "3"][..],
            &["0", "0", "0"][..],
            &["1", "0", "0"][..],
        ),
        (
            &["1", "2", "3"][..],
            &["0", "1", "0"][..],
            &["1", "2", "3"][..],
        ),
    ] {
        for reordered in [false, true] {
            let mut f = fixture(p, q, expected, true, reordered)?;
            let request = operation(&f)?;
            let checked =
                f.base
                    .cas
                    .check_proof(&f.base.context, &f.steps, &mut default_budget()?)?;
            assert_eq!(checked.judgment(), &f.expected);
            let produced = f
                .base
                .cas
                .compute_series(&request, &[], &mut default_budget()?)?;
            assert_eq!(produced.judgment(), &f.expected);
            let bytes = f
                .base
                .cas
                .export_witness(&checked, &mut default_budget()?)?;
            let mut receiver = fixture(p, q, expected, true, !reordered)?;
            let replayed =
                receiver
                    .base
                    .cas
                    .replay(&bytes, &receiver.expected, &mut default_budget()?)?;
            assert_eq!(replayed.judgment(), &receiver.expected);
        }
    }
    Ok(())
}

/// Every finite field, including a masked zero product, contributes to checked authority.
/// ´test:integration:compose-complete-row-mutations´
#[test]
fn compose_complete_row_mutations() -> TestResult {
    for mutation in 0..22 {
        let mut f = fixture(
            &["0", "1", "1", "0"],
            &["0", "1", "-1", "0"],
            &["0", "1", "0", "-2"],
            true,
            false,
        )?;
        let Certificate::Series(value) = &mut f.steps.last_mut().ok_or("root")?.certificate else {
            return Err("certificate".into());
        };
        let C::Compose {
            form,
            zero,
            seed,
            powers,
            sums,
            order,
            default_order,
        } = value.as_mut()
        else {
            return Err("compose".into());
        };
        match mutation {
            0 => *zero = rational("1")?,
            1 => seed[0] = rational("0")?,
            2 => powers[0].exponent = 2,
            3 => powers[0].row.left_index = 1,
            4 => powers[0].row.left = rational("7")?,
            5 => powers[0].row.product = rational("7")?,
            6 => powers[0].row.sum = rational("7")?,
            7 => {
                powers.pop();
            }
            8 => powers.swap(0, 1),
            9 => powers.push(powers[0].clone()),
            10 => sums[0].product = rational("7")?,
            11 => *form = 1,
            12 => *form = 2,
            13 => *order = 3,
            14 => *default_order = false,
            15 => seed.push(rational("0")?),
            16 => sums.swap(0, 1),
            17 => {
                sums.pop();
            }
            18 => powers[0].row.right = rational("7")?,
            19 => powers[0].row.degree = 1,
            20 => powers[0].row.right_index = 1,
            _ => sums[0].sum = rational("7")?,
        }
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        let fault = f
            .base
            .cas
            .check_proof(&f.base.context, &f.steps, &mut default_budget()?)
            .expect_err("false evidence");
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(if matches!(mutation, 11 | 12) {
                InvalidCode::SeriesUnsupported
            } else {
                InvalidCode::Rule
            })
        );
        assert_eq!(
            before,
            f.base.cas.canonical_snapshot(&mut default_budget()?)?
        );
    }
    Ok(())
}

/// The mathematical refusal owns the exact coefficient beyond rollback and store destruction.
/// ´test:integration:compose-refusal-owned-evidence´
#[test]
fn compose_refusal_owned_evidence() -> TestResult {
    for p in [&["0", "0"][..], &["7", "0"][..], &["1", "2"][..]] {
        for production in [false, true] {
            let mut f = fixture(p, &["2/3", "1"], &["0", "0"], true, false)?;
            let nrz_cas::series::SeriesOperation::Compose { left, right, .. } = operation(&f)?
            else {
                return Err("compose".into());
            };
            let coefficient = f
                .base
                .cas
                .series_object(&right, &mut crate::testing::budgets::graph_budget()?)?
                .coefficients[0]
                .clone();
            f.base.cas.compute_series(
                &nrz_cas::series::SeriesOperation::Truncate {
                    input: left,
                    order: 1,
                },
                &[],
                &mut default_budget()?,
            )?;
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            let mut budget = default_budget()?;
            budget.work(
                37,
                nrz_cas::Location::new("composition refusal prior spend"),
            )?;
            let fault = if production {
                f.base.cas.compute_series(&operation(&f)?, &[], &mut budget)
            } else {
                f.base
                    .cas
                    .check_proof(&f.base.context, &f.steps, &mut budget)
            }
            .expect_err("nonzero constant");
            assert_eq!(
                fault.kind,
                FaultKind::Invalid(InvalidCode::SeriesNonzeroConstant)
            );
            assert!(budget.usage().work > 37);
            assert_eq!(
                before,
                f.base.cas.canonical_snapshot(&mut default_budget()?)?
            );
            drop(f);
            let evidence = fault.series_constant().ok_or("owned evidence")?;
            assert_eq!(evidence.input.logical_id(), right.logical_id());
            assert_eq!(evidence.coefficient.logical_id(), coefficient.logical_id());
            assert!(evidence.input.same_attempt(&evidence.coefficient));
            assert_eq!(evidence.index, 0);
            assert_eq!(evidence.value, rational("2/3")?);
        }
    }
    Ok(())
}

/// Refused imported identities stay historical as later admissions get fresh serials.
/// ´test:integration:compose-import-refusal-identity-reuse´
#[test]
fn compose_import_refusal_identity_reuse() -> TestResult {
    use nrz_cas::solving::{CoefficientDomainData, SolveDraft};
    use nrz_cas::syntax::Draft;
    let bytes = crate::integration::nonzero_compose_import()?;
    let mut f = crate::testing::cases::Fixture::new()?;
    let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
    let mut budget = default_budget()?;
    budget.work(37, nrz_cas::Location::new("refused import prior work"))?;
    let fault = f
        .cas
        .inspect(&bytes, &mut budget)
        .expect_err("nonzero imported constant");
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::SeriesNonzeroConstant)
    );
    assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);
    assert!(budget.usage().work > 37);
    let saved = fault.series_constant().ok_or("import evidence")?;
    assert_eq!(saved.value, rational("2/3")?);
    assert_eq!(saved.index, 0);
    assert!(saved.input.same_attempt(&saved.coefficient));
    // The same wire records receive fresh logical IDs and a distinct refusal attempt.
    let second = f
        .cas
        .inspect(&bytes, &mut budget)
        .expect_err("second refused import");
    let second = second.series_constant().ok_or("second evidence")?;
    assert_ne!(saved.input.logical_id(), second.input.logical_id());
    assert_ne!(
        saved.coefficient.logical_id(),
        second.coefficient.logical_id()
    );
    assert_ne!(saved.input, second.input);
    assert_ne!(saved.coefficient, second.coefficient);
    assert!(!saved.input.same_attempt(&second.input));
    assert!(second.input.same_attempt(&second.coefficient));
    assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);
    let replacement_coefficient = f.number("1000")?;
    let coefficient_advanced =
        replacement_coefficient.logical_id().serial > saved.coefficient.logical_id().serial;
    assert!(coefficient_advanced);
    assert_ne!(
        replacement_coefficient.logical_id(),
        saved.coefficient.logical_id()
    );
    let field = f
        .cas
        .admit(
            &[Draft::Solve(SolveDraft::CoefficientDomain(
                CoefficientDomainData::Rational,
            ))],
            &mut default_budget()?,
        )?
        .remove(0);
    let replacement = crate::testing::series_proof::object(&mut f, &field, &["1000"])?;
    let input_advanced = replacement.logical_id().serial > saved.input.logical_id().serial;
    assert!(input_advanced);
    assert_ne!(replacement.logical_id(), saved.input.logical_id());
    let object = f.cas.series_object(&replacement, &mut budget)?;
    assert_eq!(object.order, 1);
    assert_eq!(object.coefficients, [replacement_coefficient]);
    // Only the new handle authenticates that lookup; `SeriesDiagnosticIdentity` cannot
    // be passed to it (the public compile-fail control checks this type boundary).
    let after_replacement = f.cas.canonical_snapshot(&mut default_budget()?)?;
    let later = f
        .cas
        .inspect(&bytes, &mut budget)
        .expect_err("refused import after replacement");
    let later = later.series_constant().ok_or("later evidence")?;
    assert_ne!(saved.input, later.input);
    assert_ne!(saved.coefficient, later.coefficient);
    assert!(!saved.input.same_attempt(&later.input));
    assert_eq!(
        after_replacement,
        f.cas.canonical_snapshot(&mut default_budget()?)?
    );
    drop(f);
    assert_eq!(saved.value, rational("2/3")?);
    assert_eq!(saved.input, saved.input.clone());
    Ok(())
}

/// Different original orders and repeated original operands retain ordered owned restrictions.
/// ´test:integration:compose-ordered-restrictions´
#[test]
fn compose_ordered_restrictions() -> TestResult {
    for (p, q, out, default) in [
        (
            &["1", "2", "3", "9"][..],
            &["0", "1", "0"][..],
            &["1", "2", "3"][..],
            true,
        ),
        (
            &["1", "2", "3", "9"][..],
            &["0", "1", "0"][..],
            &["1", "2"][..],
            false,
        ),
        (
            &["0", "1", "1"][..],
            &["0", "1", "1"][..],
            &["0", "1"][..],
            false,
        ),
    ] {
        let mut f = fixture(p, q, out, default, false)?;
        let request = operation(&f)?;
        let mut proofs = Vec::new();
        for step in &f.steps[..f.steps.len() - 1] {
            proofs.push(
                f.base
                    .cas
                    .check_proof(
                        &f.base.context,
                        std::slice::from_ref(step),
                        &mut default_budget()?,
                    )?
                    .proof()
                    .clone(),
            );
        }
        let actual = f
            .base
            .cas
            .compute_series(&request, &proofs, &mut default_budget()?)?;
        assert_eq!(actual.judgment(), &f.expected);
        let supplied = f
            .base
            .cas
            .check_proof(&f.base.context, &f.steps, &mut default_budget()?)?;
        assert_eq!(supplied.judgment(), &f.expected);
        for mutation in 0..3 {
            let mut wrong = proofs.clone();
            match mutation {
                0 => {
                    wrong.pop();
                }
                1 => wrong.push(proofs[0].clone()),
                _ => {
                    if p == q || proofs.len() < 2 {
                        continue;
                    }
                    wrong.swap(0, 1);
                }
            }
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            assert_eq!(
                f.base
                    .cas
                    .compute_series(&request, &wrong, &mut default_budget()?)
                    .expect_err("ordered slots")
                    .kind,
                FaultKind::Invalid(InvalidCode::Rule)
            );
            assert_eq!(
                before,
                f.base.cas.canonical_snapshot(&mut default_budget()?)?
            );
        }
    }
    Ok(())
}

/// Source-backed objects remain ordinary objects, and source proofs cannot fill restriction slots.
/// ´test:integration:compose-source-boundary´
#[test]
fn compose_source_boundary() -> TestResult {
    use nrz_cas::proof::Statement;
    use nrz_cas::series::{SeriesOperation as O, SeriesStatement as S};
    let mut f = crate::testing::series_source::setup(0, 4, false, true)?;
    let source = f
        .base
        .cas
        .expand_series_source(&f.request, &f.proofs, &mut default_budget()?)?;
    let input = crate::testing::series_source::statement(source.judgment())?
        .output
        .clone();
    let witness = f.base.cas.compute_series(
        &O::Compose {
            left: input.clone(),
            right: input.clone(),
            order: None,
        },
        &[],
        &mut default_budget()?,
    )?;
    assert!(
        matches!(&witness.judgment().statement,Statement::Series(s) if matches!(s.as_ref(),S::Compose{..}))
    );
    let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
    let fault = f
        .base
        .cas
        .compute_series(
            &O::Compose {
                left: input.clone(),
                right: input,
                order: Some(3),
            },
            &[source.proof().clone(), source.proof().clone()],
            &mut default_budget()?,
        )
        .expect_err("source is not an object restriction");
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Rule));
    assert_eq!(
        before,
        f.base.cas.canonical_snapshot(&mut default_budget()?)?
    );
    Ok(())
}

/// Precision refusals retain both available orders independently of mathematical nonzero evidence.
/// ´test:integration:compose-precision-refusals´
#[test]
fn compose_precision_refusals() -> TestResult {
    use nrz_cas::proof::Statement;
    use nrz_cas::series::{SeriesOperation as O, SeriesStatement as S};
    for requested in [0, 3] {
        for production in [false, true] {
            let mut f = fixture(&["1", "1"], &["0", "1"], &["1", "1"], true, false)?;
            let mut request = operation(&f)?;
            let O::Compose { order, .. } = &mut request else {
                return Err("compose".into());
            };
            *order = Some(requested);
            let root = f.steps.last_mut().ok_or("root")?;
            let Statement::Series(s) = &mut root.conclusion else {
                return Err("series".into());
            };
            let S::Compose {
                order,
                default_order,
                ..
            } = s.as_mut()
            else {
                return Err("compose".into());
            };
            *order = requested;
            *default_order = false;
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            let mut budget = default_budget()?;
            budget.work(
                37,
                nrz_cas::Location::new("composition precision prior work"),
            )?;
            let fault = if production {
                f.base.cas.compute_series(&request, &[], &mut budget)
            } else {
                f.base
                    .cas
                    .check_proof(&f.base.context, &f.steps, &mut budget)
            }
            .expect_err("precision");
            assert_eq!(
                fault.kind,
                FaultKind::Invalid(InvalidCode::SeriesUnsupported)
            );
            assert_eq!(fault.series_precision(), Some([2, 2, requested]));
            assert!(fault.series_constant().is_none());
            assert!(budget.usage().work > 37);
            assert_eq!(
                before,
                f.base.cas.canonical_snapshot(&mut default_budget()?)?
            );
        }
    }
    Ok(())
}

/// Returned refusal payload has its own exact byte boundary and outlives canonical rollback.
/// ´test:integration:compose-refusal-evidence-budget´
#[test]
fn compose_refusal_evidence_budget() -> TestResult {
    use nrz_cas::limits::{Budget, BudgetKind, Limits};
    use nrz_cas::proof::Statement;
    use nrz_cas::series::{SeriesOperation as O, SeriesStatement as S};
    assert!(
        std::mem::size_of::<nrz_cas::SeriesRefusalEvidence>() + 2 * std::mem::size_of::<usize>()
            <= 256
    );
    for precision in [false, true] {
        for production in [false, true] {
            let bytes = if precision { 256 } else { 273 };
            for available in [bytes - 1, bytes] {
                let mut f = fixture(
                    &["7"],
                    &[if precision { "0" } else { "2/3" }],
                    &["7"],
                    true,
                    false,
                )?;
                let mut request = operation(&f)?;
                if precision {
                    let O::Compose { order, .. } = &mut request else {
                        return Err("compose".into());
                    };
                    *order = Some(2);
                    let Statement::Series(s) = &mut f.steps[0].conclusion else {
                        return Err("series".into());
                    };
                    let S::Compose {
                        order,
                        default_order,
                        ..
                    } = s.as_mut()
                    else {
                        return Err("compose".into());
                    };
                    *order = 2;
                    *default_order = false;
                }
                let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
                let mut budget = Budget::new(Limits {
                    output_bytes: available,
                    ..Limits::default()
                })?;
                budget.work(37, nrz_cas::Location::new("refusal evidence prior work"))?;
                let fault = if production {
                    f.base.cas.compute_series(&request, &[], &mut budget)
                } else {
                    f.base
                        .cas
                        .check_proof(&f.base.context, &f.steps, &mut budget)
                }
                .expect_err("refusal evidence");
                assert_eq!(
                    before,
                    f.base.cas.canonical_snapshot(&mut default_budget()?)?
                );
                assert!(budget.usage().work > 37);
                if available < bytes {
                    assert_eq!(
                        fault.kind,
                        FaultKind::Exhausted {
                            reason: nrz_cas::Exhaustion::Limit,
                            budget: BudgetKind::OutputBytes,
                            limit: available,
                            needed: bytes
                        }
                    );
                    assert!(fault.series_evidence.is_none());
                } else if precision {
                    assert_eq!(
                        fault.kind,
                        FaultKind::Invalid(InvalidCode::SeriesUnsupported)
                    );
                    drop(f);
                    assert_eq!(fault.series_precision(), Some([1, 1, 2]));
                } else {
                    assert_eq!(
                        fault.kind,
                        FaultKind::Invalid(InvalidCode::SeriesNonzeroConstant)
                    );
                    drop(f);
                    assert_eq!(
                        fault.series_constant().ok_or("coefficient")?.value,
                        rational("2/3")?
                    );
                }
            }
        }
    }
    Ok(())
}
