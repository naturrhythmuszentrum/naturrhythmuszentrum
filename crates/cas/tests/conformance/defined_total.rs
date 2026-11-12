// SPDX-License-Identifier: AGPL-3.0-only
//! Closed grammar, strict original syntax and exact application boundaries.

use crate::testing::common::{TestResult, default_budget};
use crate::testing::defined_total::Fixture;
use nrz_cas::proof::{Certificate, Primitive};
use nrz_cas::syntax::{AtomicSort, Branch, Constant, Field, Operator, Sort};
use nrz_cas::{FaultKind, InvalidCode, Rational};

#[test]
fn defined_total_numeric_grammar() -> TestResult {
    for branch in [Branch::StrictReal, Branch::PrincipalComplex] {
        for sort in [
            AtomicSort::Integer,
            AtomicSort::Scalar(Field::Rational),
            AtomicSort::Scalar(Field::Real),
            AtomicSort::Scalar(Field::Complex),
        ] {
            let mut f = Fixture::new(branch.clone(), false, 0, false)?;
            let sort = Sort::Value(sort);
            let one = f.node(Operator::Rational(Rational::one()), sort.clone(), &[])?;
            let neg = f.node(Operator::Neg, sort.clone(), std::slice::from_ref(&one))?;
            let sum = f.node(Operator::Add, sort.clone(), &[one.clone(), neg])?;
            let difference = f.node(Operator::Sub, sort.clone(), &[sum, one.clone()])?;
            let product = f.node(Operator::Mul, sort.clone(), &[difference, one])?;
            let power = f.node(Operator::IntegerPower(256), sort.clone(), &[product])?;
            let mut values = vec![power];
            if sort != Sort::Value(AtomicSort::Integer) {
                let integer = f.node(
                    Operator::Free("i".into()),
                    Sort::Value(AtomicSort::Integer),
                    &[],
                )?;
                let embedded = f.node(Operator::IntegerEmbed, sort.clone(), &[integer])?;
                let divisor = f.node(
                    Operator::Rational(Rational::parse("-2/3", &mut default_budget()?)?),
                    sort.clone(),
                    &[],
                )?;
                values.push(f.node(Operator::Div, sort.clone(), &[embedded, divisor])?);
            }
            if matches!(
                sort,
                Sort::Value(AtomicSort::Scalar(Field::Real | Field::Complex))
            ) {
                for constant in [Constant::Pi, Constant::E] {
                    values.push(f.node(Operator::Constant(constant), sort.clone(), &[])?);
                }
            }
            for function in values {
                let witness = f
                    .cas
                    .defined_total(&function, &f.context, &mut default_budget()?)?;
                assert_eq!(witness.judgment(), &f.expected(&function)?);
                assert!(witness.is_unconditional());
            }
        }
    }
    Ok(())
}

fn refused(f: &mut Fixture, function: &nrz_cas::Handle, code: InvalidCode) -> TestResult {
    let draft = f.draft(function)?;
    let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
    for supplied in [false, true] {
        let result = if supplied {
            f.cas.check_proof(
                &f.context,
                std::slice::from_ref(&draft),
                &mut default_budget()?,
            )
        } else {
            f.cas
                .defined_total(function, &f.context, &mut default_budget()?)
        };
        assert_eq!(
            result.err().ok_or("unsupported syntax accepted")?.kind,
            FaultKind::Invalid(code)
        );
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
    }
    Ok(())
}

#[test]
fn defined_total_original_partial_operands() -> TestResult {
    let mut f = Fixture::new(Branch::StrictReal, false, 4, false)?;
    let x = f.term(Operator::Free("x".into()), &[])?;
    let zero = f.term(Operator::Rational(Rational::zero()), &[])?;
    let one = f.term(Operator::Rational(Rational::one()), &[])?;
    let hole = f.term(Operator::Div, &[one.clone(), x.clone()])?;
    let dead = f.term(Operator::Mul, &[zero.clone(), hole.clone()])?;
    let cancel = f.term(Operator::Sub, &[hole.clone(), hole.clone()])?;
    let constant = f.term(Operator::Sub, &[x.clone(), x.clone()])?;
    let constant = f.term(Operator::Add, &[constant, one.clone()])?;
    let hidden = f.term(Operator::Add, &[dead.clone(), one])?;
    let mut excluded = vec![hole, dead, cancel];
    for denominator in [x.clone(), constant, hidden] {
        excluded.push(f.term(Operator::Div, &[x.clone(), denominator])?);
    }
    for operator in [
        Operator::IntegerPower(0),
        Operator::IntegerPower(-1),
        Operator::NaturalLog,
        Operator::Sine,
        Operator::Cosine,
        Operator::Arctangent,
        Operator::Exponential,
        Operator::Sigmoid,
        Operator::Tanh,
        Operator::Call("opaque".into()),
        Operator::Derivative("x".into()),
    ] {
        excluded.push(f.term(operator, std::slice::from_ref(&x))?);
    }
    for operator in [
        Operator::GeneralPower(Branch::StrictReal),
        Operator::PositivePower,
        Operator::ClampFloor,
    ] {
        excluded.push(f.term(operator, &[x.clone(), zero.clone()])?);
    }
    for function in excluded {
        refused(&mut f, &function, InvalidCode::UnsupportedFragment)?;
    }
    let zero_divisor = f.term(Operator::Div, &[x, zero])?;
    refused(&mut f, &zero_divisor, InvalidCode::RefutedDomain)
}

#[test]
fn defined_total_malformed_applications() -> TestResult {
    use nrz_cas::domain::{AmbientDomain, RegionDraft, RegionExpr};
    use nrz_cas::proof::Statement;
    use nrz_cas::syntax::Substitution;
    use nrz_cas::{Reference, syntax::Draft};
    let mut f = Fixture::new(Branch::StrictReal, false, 0, false)?;
    let function = f.polynomial()?;
    let original = f.draft(&function)?;
    let other_region = crate::testing::series_residual::admit(
        &mut f.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(f.header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::Empty,
        }),
    )?;
    for change in 0..5 {
        let mut draft = original.clone();
        match change {
            0 => draft.premises.push(0),
            1 => draft.certificate = Certificate::Congruence(Vec::new()),
            2 => draft.substitutions.push(Substitution {
                key: "x".into(),
                replacement: function.clone(),
            }),
            3 => {
                let Statement::PredicateFact { region, .. } = &mut draft.conclusion else {
                    return Err("predicate".into());
                };
                *region = other_region.clone();
            }
            _ => {
                draft.conclusion = Statement::Empty {
                    region: other_region.clone(),
                }
            }
        }
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.cas
                .check_proof(&f.context, &[draft], &mut default_budget()?)
                .err()
                .ok_or("malformed application")?
                .kind,
            FaultKind::Invalid(InvalidCode::Rule)
        );
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
    }
    let spec = Primitive::DefinedTotal.specification();
    assert_eq!(
        (spec.key, spec.version, spec.weight),
        ("defined.total", 1, 1)
    );
    assert_eq!(spec.premises, nrz_cas::proof::PremiseShape::Exact(0));
    assert_eq!(spec.certificate, nrz_cas::proof::CertificateSchema::None);
    Ok(())
}
