// SPDX-License-Identifier: AGPL-3.0-only
//! Independent complete claims, fresh replay and the Product residual consumer.

use crate::testing::common::{TestResult, default_budget};
use crate::testing::defined_total::Fixture;
use nrz_cas::syntax::{Branch, Operator};
use nrz_cas::{FaultKind, InvalidCode};

#[test]
fn defined_total_ambient_scope_and_replay() -> TestResult {
    for ambient in 0..5 {
        for scoped in [false, true] {
            let mut source = Fixture::new(Branch::StrictReal, scoped, ambient, false)?;
            let function = source.polynomial()?;
            let witness =
                source
                    .cas
                    .defined_total(&function, &source.context, &mut default_budget()?)?;
            let expected = source.expected(&function)?;
            assert_eq!(witness.judgment(), &expected);
            assert!(witness.is_unconditional());
            let draft = source.draft(&function)?;
            let supplied =
                source
                    .cas
                    .check_proof(&source.context, &[draft], &mut default_budget()?)?;
            assert_eq!(supplied.judgment(), &expected);
            let bytes = source
                .cas
                .export_witness(&witness, &mut default_budget()?)?;
            let mut receiver = Fixture::new(Branch::StrictReal, scoped, ambient, true)?;
            let original = receiver.polynomial()?;
            let expected = receiver.expected(&original)?;
            let other = receiver.term(Operator::Free("y".into()), &[])?;
            let changed = receiver.expected(&other)?;
            let before = receiver.cas.canonical_snapshot(&mut default_budget()?)?;
            assert_eq!(
                receiver
                    .cas
                    .replay(&bytes, &changed, &mut default_budget()?)
                    .err()
                    .ok_or("different claim")?
                    .kind,
                FaultKind::Invalid(InvalidCode::EndpointMismatch)
            );
            assert_eq!(
                receiver.cas.canonical_snapshot(&mut default_budget()?)?,
                before
            );
            let replayed = receiver
                .cas
                .replay(&bytes, &expected, &mut default_budget()?)?;
            assert_eq!(replayed.judgment(), &expected);
            assert!(replayed.is_unconditional());
            assert_ne!(replayed.proof(), witness.proof());
        }
    }
    Ok(())
}

#[test]
fn defined_total_langevin_product_residual() -> TestResult {
    use crate::testing::series_residual::{Fixture as ResidualFixture, Term, binary, rational};
    use Term::{Q, X, Y};
    use nrz_cas::proof::Statement;
    use nrz_cas::series::SeriesStatement;
    let source = binary(
        Operator::Sub,
        binary(Operator::Mul, Q("1/3"), X),
        binary(Operator::Mul, Q("1/45"), Term::Power(Box::new(X), 3)),
    );
    let differential = binary(
        Operator::Sub,
        binary(
            Operator::Add,
            binary(
                Operator::Add,
                binary(Operator::Mul, X, Y(1)),
                binary(Operator::Mul, Q("2"), Y(0)),
            ),
            binary(Operator::Mul, X, Term::Power(Box::new(Y(0)), 2)),
        ),
        X,
    );
    for reordered in [false, true] {
        let mut f = ResidualFixture::totality(reordered)?;
        let subject = f.total_source(&source)?;
        assert!(subject.is_unconditional());
        let Statement::Series(statement) = &subject.judgment().statement else {
            return Err("source statement".into());
        };
        let SeriesStatement::Source(source) = statement.as_ref() else {
            return Err("source".into());
        };
        let expected = ["0", "1/3", "0", "-1/45", "0"]
            .map(rational)
            .into_iter()
            .collect::<TestResult<Vec<_>>>()?;
        let expected_object = f.object(&expected)?;
        assert_eq!(source.output, expected_object);
        let (_, full_rows) = f.term(&differential, 8)?;
        let full_expected = ["0", "0", "0", "0", "0", "-2/135", "0", "1/2025"]
            .map(rational)
            .into_iter()
            .collect::<TestResult<Vec<_>>>()?;
        assert_eq!(
            full_rows.last().ok_or("full residual")?.coefficients,
            full_expected
        );
        let (expression, rows) = f.term(&differential, 4)?;
        assert_eq!(
            rows.last().ok_or("residual root")?.coefficients,
            vec![nrz_cas::Rational::zero(); 4]
        );
        let (residual, _) = f.cas.check_evaluated_residual(
            &subject,
            &expression,
            &f.y,
            &rows,
            None,
            &mut default_budget()?,
        )?;
        assert!(residual.is_unconditional());
        let Statement::Series(statement) = &residual.judgment().statement else {
            return Err("residual statement".into());
        };
        let SeriesStatement::EvaluatedResidual {
            derivative_order,
            order,
            output,
            ..
        } = statement.as_ref()
        else {
            return Err("residual".into());
        };
        assert_eq!((*derivative_order, *order), (1, 4));
        let zeros = f.object(&vec![nrz_cas::Rational::zero(); 4])?;
        assert_eq!(*output, zeros);
        let bytes = f.cas.export_witness(&residual, &mut default_budget()?)?;
        assert!(
            bytes
                .windows(b"defined.total".len())
                .any(|value| value == b"defined.total")
        );
    }
    Ok(())
}
