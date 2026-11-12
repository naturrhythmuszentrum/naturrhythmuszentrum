// SPDX-License-Identifier: AGPL-3.0-only
//! Occurrences and closed numeric syntax remain independent of algebraic expansion.

use crate::testing::common::{TestResult, default_budget, real};
use crate::testing::defined_total::Fixture;
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::syntax::{AtomicSort, Branch, Field, Operator, PredicateKind, Sort};
use nrz_cas::{FaultKind, InvalidCode, Rational};

/// A shared child occurs twice; a positive power only inspects its fixed signed exponent.
#[test]
fn defined_total_repeated_occurrences_and_unexpanded_power() -> TestResult {
    for (operator, visits) in [(Operator::Add, 3), (Operator::IntegerPower(i64::MAX), 2)] {
        let mut f = Fixture::new(Branch::StrictReal, false, 4, false)?;
        let x = f.term(Operator::Free("x".into()), &[])?;
        let operands = if operator == Operator::Add {
            vec![x.clone(), x]
        } else {
            vec![x]
        };
        let function = f.term(operator, &operands)?;
        let draft = f.draft(&function)?;
        for supplied in [false, true] {
            for accepted in [false, true] {
                let limits = Limits {
                    condition_steps: visits + u64::from(accepted),
                    arithmetic_steps: 0,
                    integer_exponent: 0,
                    polynomial_degree: 0,
                    polynomial_variables: 0,
                    polynomial_terms: 0,
                    region_cells: 0,
                    jet_order: 0,
                    jet_entries: 0,
                    coefficient_ops: 0,
                    ..Limits::default()
                };
                let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
                let mut budget = Budget::new(limits.clone())?;
                let result = if supplied {
                    f.cas
                        .check_proof(&f.context, std::slice::from_ref(&draft), &mut budget)
                } else {
                    f.cas.defined_total(&function, &f.context, &mut budget)
                };
                assert_eq!(result.is_ok(), accepted);
                assert_eq!(budget.limits(), &limits);
                if let Err(fault) = result {
                    assert_eq!(
                        fault.kind,
                        FaultKind::Exhausted {
                            reason: nrz_cas::Exhaustion::Limit,
                            budget: BudgetKind::ConditionSteps,
                            limit: visits,
                            needed: visits + 1,
                        }
                    );
                    assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
                }
            }
        }
    }
    Ok(())
}

/// Canonical rational magnitude encoding still prepays its existing arithmetic charge.
#[test]
fn defined_total_literal_encoding_uses_arithmetic_ledger() -> TestResult {
    for supplied in [false, true] {
        let (mut f, function) = crate::testing::defined_total_minimal::fixture(false)?;
        let draft = f.draft(&function)?;
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        let counts = f.cas.counts();
        let limits = Limits {
            arithmetic_steps: 0,
            ..Limits::default()
        };
        let mut budget = Budget::new(limits.clone())?;
        let result = if supplied {
            f.cas.check_proof(&f.context, &[draft], &mut budget)
        } else {
            f.cas.defined_total(&function, &f.context, &mut budget)
        };
        assert_eq!(
            result.err().ok_or("unfunded literal encoding")?.kind,
            FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::Limit,
                budget: BudgetKind::ArithmeticSteps,
                limit: 0,
                needed: 1,
            }
        );
        assert_eq!(budget.usage().arithmetic_steps, 0);
        assert_eq!(budget.limits(), &limits);
        assert_eq!(f.cas.counts(), counts);
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
    }
    Ok(())
}

/// Admission supplies well-typed syntax, but none of these values belongs to the closed fragment.
#[test]
fn defined_total_nonvalue_and_root_operators() -> TestResult {
    use nrz_cas::syntax::{Extent, MatrixOperation, RootSelection};
    let mut f = Fixture::new(Branch::StrictReal, false, 0, false)?;
    let x = f.term(Operator::Free("x".into()), &[])?;
    let function = f.node(
        Operator::Free("opaque".into()),
        Sort::Function {
            inputs: vec![AtomicSort::Scalar(Field::Real)],
            output: AtomicSort::Scalar(Field::Real),
        },
        &[],
    )?;
    let predicate = f.node(
        Operator::Predicate(PredicateKind::NonZero),
        Sort::Value(AtomicSort::Boolean),
        std::slice::from_ref(&x),
    )?;
    let matrix_sort = Sort::Value(AtomicSort::Matrix {
        field: Field::Real,
        rows: Extent::Literal(1),
        columns: Extent::Literal(1),
    });
    let matrix = f.node(
        Operator::Matrix {
            rows: 1,
            columns: 1,
        },
        matrix_sort.clone(),
        std::slice::from_ref(&x),
    )?;
    let transpose = f.node(
        Operator::Transpose,
        matrix_sort,
        std::slice::from_ref(&matrix),
    )?;
    let determinant = f.node(
        Operator::MatrixMap(MatrixOperation::Determinant),
        real(),
        std::slice::from_ref(&matrix),
    )?;
    let root = f.term(
        Operator::AlgebraicRoot {
            variable: "x".into(),
            field: Field::Real,
            multiplicity: Some(1),
            selection: RootSelection::Ordered(0),
        },
        &[x],
    )?;
    for function in [function, predicate, matrix, transpose, determinant, root] {
        let draft = f.draft(&function)?;
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.cas
                .check_proof(&f.context, &[draft], &mut default_budget()?)
                .err()
                .ok_or("excluded operator")?
                .kind,
            FaultKind::Invalid(InvalidCode::UnsupportedFragment)
        );
        assert_eq!(
            f.cas
                .defined_total(&function, &f.context, &mut default_budget()?)
                .err()
                .ok_or("excluded operator")?
                .kind,
            FaultKind::Invalid(InvalidCode::UnsupportedFragment)
        );
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
    }
    Ok(())
}

#[test]
fn defined_total_excluded_header_conventions() -> TestResult {
    use crate::testing::series_residual::admit;
    use nrz_cas::Reference;
    use nrz_cas::domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
    use nrz_cas::syntax::{Draft, Environment};
    for (field, branch) in [
        (Field::Named("uninterpreted".into()), Branch::StrictReal),
        (Field::PositiveCharacteristic(5), Branch::StrictReal),
        (Field::Real, Branch::Declared("uninterpreted".into())),
    ] {
        let (mut f, _) = crate::testing::defined_total_minimal::fixture(false)?;
        f.header = admit(
            &mut f.cas,
            Draft::Header(HeaderDraft::Core(Environment {
                field,
                branch,
                declarations: Vec::new(),
            })),
        )?;
        f.region = admit(
            &mut f.cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(f.header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Assignments,
                expression: RegionExpr::Ambient,
            }),
        )?;
        f.context = admit(
            &mut f.cas,
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(f.header.clone()),
                admissible: Reference::Existing(f.region.clone()),
                assumptions: Vec::new(),
            }),
        )?;
        let function = f.term(Operator::Rational(Rational::one()), &[])?;
        let draft = f.draft(&function)?;
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.cas
                .check_proof(&f.context, &[draft], &mut default_budget()?)
                .err()
                .ok_or("excluded convention")?
                .kind,
            FaultKind::Invalid(InvalidCode::UnsupportedFragment)
        );
        assert_eq!(
            f.cas
                .defined_total(&function, &f.context, &mut default_budget()?)
                .err()
                .ok_or("excluded convention")?
                .kind,
            FaultKind::Invalid(InvalidCode::UnsupportedFragment)
        );
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
    }
    Ok(())
}

/// A separately retained nonzero guard does not extend the primitive's strict-power grammar.
#[test]
fn defined_total_guard_does_not_authorize_nonpositive_powers() -> TestResult {
    use crate::testing::series_residual::admit;
    use nrz_cas::Reference;
    use nrz_cas::domain::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
    use nrz_cas::syntax::Draft;
    let mut f = Fixture::new(Branch::StrictReal, false, 1, false)?;
    let x = f.term(Operator::Free("x".into()), &[])?;
    let condition = admit(
        &mut f.cas,
        Draft::Condition {
            header: Reference::Existing(f.header.clone()),
            scope: Vec::new(),
            kind: PredicateKind::NonZero,
            arguments: vec![Reference::Existing(x.clone())],
        },
    )?;
    f.region = admit(
        &mut f.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(f.header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Real("x".into()),
            expression: RegionExpr::GuardTrue {
                base: Reference::Existing(f.region.clone()),
                predicate: Reference::Existing(condition),
            },
        }),
    )?;
    f.context = admit(
        &mut f.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(f.header.clone()),
            admissible: Reference::Existing(f.region.clone()),
            assumptions: Vec::new(),
        }),
    )?;
    for exponent in [0, -1] {
        let function = f.term(Operator::IntegerPower(exponent), std::slice::from_ref(&x))?;
        let draft = f.draft(&function)?;
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.cas
                .check_proof(&f.context, &[draft], &mut default_budget()?)
                .err()
                .ok_or("strict power")?
                .kind,
            FaultKind::Invalid(InvalidCode::UnsupportedFragment)
        );
        assert_eq!(
            f.cas
                .defined_total(&function, &f.context, &mut default_budget()?)
                .err()
                .ok_or("strict power")?
                .kind,
            FaultKind::Invalid(InvalidCode::UnsupportedFragment)
        );
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
    }
    Ok(())
}
