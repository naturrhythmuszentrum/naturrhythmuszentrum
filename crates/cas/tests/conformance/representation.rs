// SPDX-License-Identifier: AGPL-3.0-only
//! Public-contract checks for representation.

use nrz_cas::Rational;
use nrz_cas::syntax::{
    Approach, AtomicSort, Binding, Branch, Constant, Extent, Field, MatrixOperation, Operator,
    PredicateKind, RootSelection, ScalarOperation, Sort, SpectralKind,
};

use crate::testing::common::{TestResult, real};
use crate::testing::representation::Fixture;

/// Arithmetic, declared calls, derivatives and selected algebraic roots retain their exact tags.
/// ´test:integration:scalar-vocabulary´
#[test]
fn scalar_vocabulary() -> TestResult {
    let mut fixture = Fixture::new()?;
    let x = fixture.node(Operator::Free("x".into()), real(), Vec::new(), None, &[])?;
    let one = fixture.one(Vec::new())?;
    for operator in [Operator::Add, Operator::Sub, Operator::Mul, Operator::Div] {
        fixture.node(
            operator,
            real(),
            Vec::new(),
            None,
            &[x.clone(), one.clone()],
        )?;
    }
    for operator in [
        Operator::Neg,
        Operator::IntegerPower(-2),
        Operator::Call("f".into()),
        Operator::Derivative("x".into()),
    ] {
        let node = fixture.node(
            operator.clone(),
            real(),
            Vec::new(),
            None,
            std::slice::from_ref(&x),
        )?;
        assert_eq!(fixture.cas.expression(&node)?.0, &operator);
    }
    for constant in [Constant::Pi, Constant::E] {
        fixture.node(Operator::Constant(constant), real(), Vec::new(), None, &[])?;
    }
    fixture.node(
        Operator::GeneralPower(Branch::StrictReal),
        real(),
        Vec::new(),
        None,
        &[x.clone(), one],
    )?;
    let integer = fixture.node(
        Operator::Rational(Rational::one()),
        Sort::Value(AtomicSort::Integer),
        Vec::new(),
        None,
        &[],
    )?;
    fixture.node(Operator::IntegerEmbed, real(), Vec::new(), None, &[integer])?;
    for selection in [RootSelection::All, RootSelection::Ordered(0)] {
        let sort = if selection == RootSelection::All {
            Sort::Set(AtomicSort::Scalar(Field::Real))
        } else {
            real()
        };
        fixture.node(
            Operator::AlgebraicRoot {
                variable: "x".into(),
                field: Field::Real,
                multiplicity: Some(2),
                selection,
            },
            sort,
            Vec::new(),
            None,
            std::slice::from_ref(&x),
        )?;
    }
    Ok(())
}

/// Calculus and solution-set bodies enter explicit frames while endpoints remain outside.
/// ´test:integration:calculus-vocabulary´
#[test]
fn calculus_vocabulary() -> TestResult {
    let mut fixture = Fixture::new()?;
    let scalar = AtomicSort::Scalar(Field::Real);
    let scope = vec![vec![scalar.clone()]];
    let frame = Binding {
        sorts: vec![scalar.clone()],
        body_operands: vec![0],
    };
    let body = fixture.node(
        Operator::Bound { depth: 0, slot: 0 },
        real(),
        scope.clone(),
        None,
        &[],
    )?;
    let endpoint = fixture.one(Vec::new())?;
    for operator in [
        Operator::Integral { definite: false },
        Operator::Limit(Approach::PositiveInfinity),
    ] {
        fixture.node(
            operator,
            real(),
            Vec::new(),
            Some(frame.clone()),
            std::slice::from_ref(&body),
        )?;
    }
    fixture.node(
        Operator::Integral { definite: true },
        real(),
        Vec::new(),
        Some(frame.clone()),
        &[body.clone(), endpoint.clone(), endpoint.clone()],
    )?;
    for operator in [
        Operator::Series {
            order: 4,
            analytic: false,
        },
        Operator::Limit(Approach::Both),
    ] {
        fixture.node(
            operator,
            real(),
            Vec::new(),
            Some(frame.clone()),
            &[body.clone(), endpoint.clone()],
        )?;
    }
    let predicate = fixture.node(
        Operator::Predicate(PredicateKind::NonZero),
        Sort::Value(AtomicSort::Boolean),
        scope,
        None,
        &[body],
    )?;
    fixture.node(
        Operator::Set { complete: true },
        Sort::Set(scalar),
        Vec::new(),
        Some(frame.clone()),
        std::slice::from_ref(&predicate),
    )?;
    fixture.node(
        Operator::Predicate(PredicateKind::ForAll),
        Sort::Value(AtomicSort::Boolean),
        Vec::new(),
        Some(frame),
        &[predicate],
    )?;
    Ok(())
}

/// Matrix shape, order, coefficient field and spectral request type survive construction.
/// ´test:integration:matrix-vocabulary´
#[test]
fn matrix_vocabulary() -> TestResult {
    let mut fixture = Fixture::new()?;
    let matrix_sort = Sort::Value(AtomicSort::Matrix {
        field: Field::Real,
        rows: Extent::Literal(1),
        columns: Extent::Literal(1),
    });
    let one = fixture.one(Vec::new())?;
    let matrix = fixture.node(
        Operator::Matrix {
            rows: 1,
            columns: 1,
        },
        matrix_sort.clone(),
        Vec::new(),
        None,
        std::slice::from_ref(&one),
    )?;
    fixture.node(
        Operator::Transpose,
        matrix_sort.clone(),
        Vec::new(),
        None,
        std::slice::from_ref(&matrix),
    )?;
    for operator in [
        Operator::MatrixMultiply,
        Operator::Entrywise(ScalarOperation::Multiply),
        Operator::Entrywise(ScalarOperation::Add),
        Operator::Entrywise(ScalarOperation::Divide),
    ] {
        fixture.node(
            operator,
            matrix_sort.clone(),
            Vec::new(),
            None,
            &[matrix.clone(), matrix.clone()],
        )?;
    }
    for operation in [MatrixOperation::Determinant, MatrixOperation::Trace] {
        fixture.node(
            Operator::MatrixMap(operation),
            real(),
            Vec::new(),
            None,
            std::slice::from_ref(&matrix),
        )?;
    }
    for operation in [MatrixOperation::Adjugate, MatrixOperation::Inverse] {
        fixture.node(
            Operator::MatrixMap(operation),
            matrix_sort.clone(),
            Vec::new(),
            None,
            std::slice::from_ref(&matrix),
        )?;
    }
    for selection in [SpectralKind::Least, SpectralKind::Greatest] {
        fixture.node(
            Operator::Spectral(selection),
            real(),
            Vec::new(),
            None,
            std::slice::from_ref(&matrix),
        )?;
    }
    fixture.node(
        Operator::Spectral(SpectralKind::AlgebraicMultiplicity),
        Sort::Value(AtomicSort::Integer),
        Vec::new(),
        None,
        &[matrix, one],
    )?;
    let body = fixture.one(vec![vec![AtomicSort::Integer, AtomicSort::Integer]])?;
    fixture.node(
        Operator::Comprehension {
            rows: Extent::Literal(1),
            columns: Extent::Literal(1),
        },
        matrix_sort,
        Vec::new(),
        Some(Binding {
            sorts: vec![AtomicSort::Integer, AtomicSort::Integer],
            body_operands: vec![0],
        }),
        &[body],
    )?;
    Ok(())
}

/// Typed deferred requests still reject nonpolynomials, undeclared extents and wrong eigenvalues.
/// ´test:integration:deferred-payload-refusals´
#[test]
fn deferred_payload_refusals() -> TestResult {
    let mut fixture = Fixture::new()?;
    let one = fixture.one(Vec::new())?;
    let call = fixture.node(Operator::Call("f".into()), real(), Vec::new(), None, &[one])?;
    assert!(
        fixture
            .node(
                Operator::AlgebraicRoot {
                    variable: "x".into(),
                    field: Field::Real,
                    multiplicity: None,
                    selection: RootSelection::All,
                },
                Sort::Set(AtomicSort::Scalar(Field::Real)),
                Vec::new(),
                None,
                &[call],
            )
            .is_err()
    );
    let body = fixture.one(vec![vec![AtomicSort::Integer, AtomicSort::Integer]])?;
    assert!(
        fixture
            .node(
                Operator::Comprehension {
                    rows: Extent::Parameter("missing".into()),
                    columns: Extent::Literal(1)
                },
                Sort::Value(AtomicSort::Matrix {
                    field: Field::Real,
                    rows: Extent::Parameter("missing".into()),
                    columns: Extent::Literal(1),
                }),
                Vec::new(),
                Some(Binding {
                    sorts: vec![AtomicSort::Integer, AtomicSort::Integer],
                    body_operands: vec![0]
                }),
                &[body],
            )
            .is_err()
    );
    let matrix = fixture.node(
        Operator::Matrix {
            rows: 0,
            columns: 0,
        },
        Sort::Value(AtomicSort::Matrix {
            field: Field::Real,
            rows: Extent::Literal(0),
            columns: Extent::Literal(0),
        }),
        Vec::new(),
        None,
        &[],
    )?;
    assert!(
        fixture
            .node(
                Operator::Spectral(SpectralKind::Eigenvectors),
                Sort::Set(AtomicSort::Matrix {
                    field: Field::Real,
                    rows: Extent::Literal(0),
                    columns: Extent::Literal(1),
                }),
                Vec::new(),
                None,
                &[matrix.clone(), matrix],
            )
            .is_err()
    );
    Ok(())
}
