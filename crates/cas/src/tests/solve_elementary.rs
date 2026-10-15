// SPDX-License-Identifier: AGPL-3.0-only
//! Scoped interaction checks for independently replayed elementary rules.

use crate::testing::common::TestResult;

use crate::budget::{Budget, Limits};
use crate::graph::{Draft, Reference};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::proof_model::{ApplicationDraft, Certificate, DomainContract, Statement};
use crate::solving::{
    CertificateMembership, SolveCertificate, SolveDraft, SolveSchema, SolveStatement,
    SolveSubstitution, SolveView, StatementMember, TupleData,
};
use crate::testing::solve_graph::{Fixture, admit, expression};

/// A checked membership keeps an unrelated conditional premise's full scoped residual.
/// ´test:crate:elementary-preserves-residuals´
#[test]
fn elementary_preserves_residuals() -> TestResult {
    let mut fixture = Fixture::new()?;
    let (cancel_step, one) = cancellation(&mut fixture)?;
    let region = fixture.cas.admissible_region(&fixture.context)?;
    let member_step = membership(&mut fixture, one, region)?;
    let checked = fixture.cas.check_proof(
        &fixture.context,
        &[cancel_step, member_step],
        &mut Budget::new(Limits::default())?,
    )?;
    assert_eq!(checked.judgment().residuals.len(), 1);
    assert_eq!(checked.hypotheses(), []);
    Ok(())
}

/// A scalar guard is created by the existing independently replayed cancellation rule.
fn cancellation(
    fixture: &mut crate::testing::solve_graph::Fixture,
) -> TestResult<(ApplicationDraft, crate::graph::Handle)> {
    let scalar = Sort::Value(AtomicSort::Scalar(Field::Real));
    let x = admit(
        &mut fixture.cas,
        &expression(
            Reference::Existing(fixture.header.clone()),
            Operator::Free("x".into()),
            scalar.clone(),
            Vec::new(),
        ),
    )?;
    let one = admit(
        &mut fixture.cas,
        &expression(
            Reference::Existing(fixture.header.clone()),
            Operator::Rational(crate::number::Rational::one()),
            scalar.clone(),
            Vec::new(),
        ),
    )?;
    let quotient = admit(
        &mut fixture.cas,
        &expression(
            Reference::Existing(fixture.header.clone()),
            Operator::Div,
            scalar,
            vec![Reference::Existing(x.clone()), Reference::Existing(x)],
        ),
    )?;
    let region = fixture.cas.admissible_region(&fixture.context)?;
    let cancellation = ApplicationDraft {
        rule: crate::rule::Primitive::Cancel,
        context: fixture.context.clone(),
        conclusion: Statement::Equality {
            left: quotient,
            right: one.clone(),
            domain: region,
            domain_contract: DomainContract::DefinedAndEqual,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    };
    Ok((cancellation, one))
}

/// Membership in the identity predicate depends on the scalar premise without dropping its guard.
fn membership(
    fixture: &mut crate::testing::solve_graph::Fixture,
    one: crate::graph::Handle,
    region: crate::graph::Handle,
) -> TestResult<ApplicationDraft> {
    let SolveView::SolveProblem(problem) = fixture.cas.solve_view(&fixture.problem)? else {
        return Err("missing problem".into());
    };
    let coordinates = problem.unknowns.clone();
    let tuple = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::Tuple(TupleData {
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            coordinates: Reference::Existing(coordinates),
            entries: vec![Reference::Existing(one.clone())],
        })),
    )?;
    let membership = ApplicationDraft {
        rule: crate::rule::Primitive::Solve(SolveSchema::Membership),
        context: fixture.context.clone(),
        conclusion: Statement::Solve(Box::new(SolveStatement::Member(StatementMember {
            problem: fixture.problem.clone(),
            tuple: tuple.clone(),
            region: region.clone(),
        }))),
        premises: vec![0],
        substitutions: Vec::new(),
        certificate: Certificate::Solve(Box::new(SolveCertificate::Membership(
            CertificateMembership {
                problem: fixture.problem.clone(),
                tuple,
                region,
                substitutions: vec![SolveSubstitution {
                    key: "x".into(),
                    expression: one,
                }],
                definedness: Vec::new(),
            },
        ))),
    };
    Ok(membership)
}

/// Translation is computed for degree zero, one, two and five, with every output class explicit.
/// ´test:crate:elementary-generic-degree-recipes´
#[test]
fn elementary_generic_degree_recipes() -> TestResult {
    use crate::solving::coefficients::arithmetic::Engine;
    use crate::solving::coefficients::model::EvaluationScope;
    use crate::solving::elementary::recipes::{BinomialRecipe, Recipe, derive};
    use crate::solving::towers::dense::Dense;
    use crate::testing::solve_elementary::RecipeFixture;
    let cases: &[(&[i64], u32, i64, i64, i64)] = &[
        (&[-6, 2], 1, -3, 2, 0),
        (&[-3, 2, 1], 2, 1, 1, 4),
        (&[-2, 0, 0, 0, 0, 1], 5, 0, 1, 2),
    ];
    for (coefficients, expected_degree, expected_shift, expected_scale, expected_radicand) in cases
    {
        let fixture = RecipeFixture::ground(coefficients)?;
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let engine = Engine {
            state: &fixture.cas.state,
            domain: &fixture.domain,
        };
        let polynomial = Dense::read(&fixture.cas.state, &fixture.polynomial, &mut budget)?;
        let scope = EvaluationScope {
            context: &fixture.context,
            region: &fixture.region,
            premises: &fixture.premises,
            localization: &[],
        };
        let Recipe::Binomial(recipe) = derive(&engine, &polynomial, &scope, &mut budget)? else {
            return Err("missing translated binomial".into());
        };
        let BinomialRecipe {
            degree,
            shift,
            scale,
            radicand,
        } = *recipe;
        assert_eq!(degree, *expected_degree);
        for (value, expected) in [
            (&shift, expected_shift),
            (&scale, expected_scale),
            (&radicand, expected_radicand),
        ] {
            let rational = crate::number::Rational::from_ratio(*expected, 1, &mut budget)?;
            assert!(engine.formal_equal(
                value,
                &engine.rational(&rational, &mut budget)?,
                &mut budget
            )?);
        }
    }
    for (coefficients, expected) in [(Vec::new(), 0), (vec![3], 1), (vec![1, 1, 0, 1], 2)] {
        let fixture = RecipeFixture::ground(&coefficients)?;
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let engine = Engine {
            state: &fixture.cas.state,
            domain: &fixture.domain,
        };
        let polynomial = Dense::read(&fixture.cas.state, &fixture.polynomial, &mut budget)?;
        let scope = EvaluationScope {
            context: &fixture.context,
            region: &fixture.region,
            premises: &[],
            localization: &[],
        };
        match derive(&engine, &polynomial, &scope, &mut budget)? {
            Recipe::Zero => assert_eq!(expected, 0),
            Recipe::Constant(value) => {
                assert_eq!(expected, 1);
                assert!(!engine.formal_zero(&value, &mut budget)?);
            }
            Recipe::General(recipe) => {
                assert_eq!(expected, 2);
                assert!(engine.formal_zero(&recipe.shift, &mut budget)?);
                assert_eq!(recipe.coefficients.coefficients.len(), 4);
            }
            Recipe::Binomial(_) => return Err("wrong recipe class".into()),
        }
    }
    Ok(())
}

/// Independent symbolic coefficients derive the general quadratic; removing the leading guard stops inversion.
/// ´test:crate:elementary-symbolic-quadratic-translation´
#[test]
fn elementary_symbolic_quadratic_translation() -> TestResult {
    use crate::solving::coefficients::arithmetic::Engine;
    use crate::solving::coefficients::model::EvaluationScope;
    use crate::solving::elementary::recipes::{BinomialRecipe, Recipe, derive};
    use crate::solving::towers::dense::Dense;
    let fixture = crate::testing::solve_elementary::RecipeFixture::symbolic()?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let engine = Engine {
        state: &fixture.cas.state,
        domain: &fixture.domain,
    };
    let polynomial = Dense::read(&fixture.cas.state, &fixture.polynomial, &mut budget)?;
    let scope = EvaluationScope {
        context: &fixture.context,
        region: &fixture.region,
        premises: &fixture.premises,
        localization: &[],
    };
    let Recipe::Binomial(recipe) = derive(&engine, &polynomial, &scope, &mut budget)? else {
        return Err("general quadratic was not derived".into());
    };
    let BinomialRecipe {
        degree,
        shift,
        scale,
        radicand,
    } = *recipe;
    assert_eq!(degree, 2);
    for (value, expected) in [&shift, &scale, &radicand]
        .into_iter()
        .zip(&fixture.expected)
    {
        assert!(engine.formal_equal(value, &engine.read(expected, &mut budget)?, &mut budget)?);
    }
    let unguarded = EvaluationScope {
        premises: &[],
        ..scope
    };
    assert!(derive(&engine, &polynomial, &unguarded, &mut budget).is_err());
    Ok(())
}
