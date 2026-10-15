// SPDX-License-Identifier: AGPL-3.0-only
//! Independent supplied certificates for strict elementary solving.

use crate::testing::common::TestResult;
use crate::testing::solve_elementary::{Fixture, hole, translation};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::proof::Certificate;
use nrz_cas::solving::SolveCertificate;
use nrz_cas::syntax::Operator;

/// The original denominator excludes one of the two numerator roots, even after cancellation.
/// ´test:integration:member-original-denominator-hole´
#[test]
fn member_original_denominator_hole() -> TestResult {
    for (value, succeeds) in [(-1, true), (1, false), (0, false)] {
        let (mut fixture, problem) = hole()?;
        let entry = fixture.integer(value, false)?;
        let step = fixture.member(&problem, &entry)?;
        let before = fixture.cas.counts();
        let result = fixture.cas.check_proof(
            &fixture.context,
            std::slice::from_ref(&step),
            &mut Budget::new(Limits::blind_cubic())?,
        );
        if succeeds {
            let witness = result?;
            assert_eq!(witness.hypotheses(), []);
            assert_eq!(witness.judgment().residuals, []);
        } else {
            assert!(result.is_err());
            assert_eq!(fixture.cas.counts(), before);
        }
    }
    Ok(())
}

/// A numerator zero does not make division by zero defined, and substitution rows cannot lie.
/// ´test:integration:member-strict-substitution-mutations´
#[test]
fn member_strict_substitution_mutations() -> TestResult {
    let mut fixture = Fixture::new()?;
    let x = fixture.free("x")?;
    let zero = fixture.integer(0, false)?;
    let one = fixture.integer(1, false)?;
    let quotient = fixture.expression(Operator::Div, &[zero.clone(), x], false)?;
    let predicate = fixture.equation(&quotient, &zero)?;
    let problem = fixture.problem("x", &predicate, &[])?;
    let valid = fixture.member(&problem, &one)?;
    fixture.cas.check_proof(
        &fixture.context,
        std::slice::from_ref(&valid),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let hole = fixture.member(&problem, &zero)?;
    let mut changed = valid;
    let Certificate::Solve(certificate) = &mut changed.certificate else {
        return Err("wrong certificate".into());
    };
    let SolveCertificate::Membership(member) = certificate.as_mut() else {
        return Err("wrong member".into());
    };
    member
        .substitutions
        .first_mut()
        .ok_or("missing substitution")?
        .expression = zero;
    for step in [hole, changed] {
        let before = fixture.cas.counts();
        assert!(
            fixture
                .cas
                .check_proof(
                    &fixture.context,
                    std::slice::from_ref(&step),
                    &mut Budget::new(Limits::blind_cubic())?
                )
                .is_err()
        );
        assert_eq!(fixture.cas.counts(), before);
    }
    Ok(())
}

/// A degree partition retains both alternatives, and a partial split expression needs definedness.
/// ´test:integration:degree-partition-mutations´
#[test]
fn degree_partition_mutations() -> TestResult {
    let mut fixture = Fixture::new()?;
    let coefficient = fixture.free("a")?;
    let valid = fixture.split(&coefficient)?;
    let checked = fixture.cas.check_proof(
        &fixture.context,
        std::slice::from_ref(&valid),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(checked.judgment().residuals, []);
    let mut missing = valid.clone();
    let Certificate::Solve(certificate) = &mut missing.certificate else {
        return Err("wrong certificate".into());
    };
    let SolveCertificate::TotalPredicateSplit(split) = certificate.as_mut() else {
        return Err("wrong split".into());
    };
    split.children.pop();
    let mut wrong = valid;
    let Certificate::Solve(certificate) = &mut wrong.certificate else {
        return Err("wrong certificate".into());
    };
    let SolveCertificate::TotalPredicateSplit(split) = certificate.as_mut() else {
        return Err("wrong split".into());
    };
    split.expression = fixture.free("b")?;
    let one = fixture.integer(1, false)?;
    let partial = fixture.expression(Operator::Div, &[one, coefficient], false)?;
    let undefined = fixture.split(&partial)?;
    for step in [missing, wrong, undefined] {
        let before = fixture.cas.counts();
        assert!(
            fixture
                .cas
                .check_proof(
                    &fixture.context,
                    std::slice::from_ref(&step),
                    &mut Budget::new(Limits::blind_cubic())?
                )
                .is_err()
        );
        assert_eq!(fixture.cas.counts(), before);
    }
    Ok(())
}

/// Translation checks both solution directions and the actual inverse composition.
/// ´test:integration:transport-inverse-mutations´
#[test]
fn transport_inverse_mutations() -> TestResult {
    for (offset, succeeds) in [(-1, true), (0, false), (1, false)] {
        let (mut fixture, step) = translation(offset)?;
        let before = fixture.cas.counts();
        let result = fixture.cas.check_proof(
            &fixture.context,
            std::slice::from_ref(&step),
            &mut Budget::new(Limits::blind_cubic())?,
        );
        if succeeds {
            assert_eq!(result?.hypotheses(), []);
        } else {
            assert!(result.is_err());
            assert_eq!(fixture.cas.counts(), before);
        }
    }
    Ok(())
}

/// Identity maps cannot turn a punctured original problem into its whole-domain numerator equation.
/// ´test:integration:transport-denominator-removal-refused´
#[test]
fn transport_denominator_removal_refused() -> TestResult {
    use nrz_cas::solving::{
        CertificateSolutionTransport, SolveSchema, SolveStatement, SolveView, StatementEquivalent,
    };
    let (mut fixture, source) = hole()?;
    let SolveView::SolveProblem(problem) = fixture.cas.solve_view(&source)? else {
        return Err("missing problem".into());
    };
    let predicate = problem.predicate.clone();
    let target = fixture.problem("x", &predicate, &[])?;
    let forward = fixture.affine(&target, 0)?;
    let backward = fixture.affine(&source, 0)?;
    let step = fixture.step(
        SolveSchema::SolutionTransport,
        SolveStatement::Equivalent(StatementEquivalent {
            source: source.clone(),
            target: target.clone(),
            forward: forward.clone(),
            backward: backward.clone(),
            region: fixture.region.clone(),
        }),
        SolveCertificate::SolutionTransport(CertificateSolutionTransport {
            source,
            target,
            forward,
            backward,
            region: fixture.region.clone(),
            definedness: Vec::new(),
            equations: Vec::new(),
            forward_inverse: Vec::new(),
            backward_inverse: Vec::new(),
        }),
    );
    let before = fixture.cas.counts();
    assert!(
        fixture
            .cas
            .check_proof(
                &fixture.context,
                std::slice::from_ref(&step),
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// Exactly one submitted membership obligation succeeds at N and refuses atomically at N-1.
/// ´test:integration:elementary-identity-budget-boundary´
#[test]
fn elementary_identity_budget_boundary() -> TestResult {
    for (limit, succeeds) in [(1, true), (0, false)] {
        let (mut fixture, problem) = hole()?;
        let entry = fixture.integer(-1, false)?;
        let step = fixture.member(&problem, &entry)?;
        let before = fixture.cas.counts();
        let mut budget = Budget::new(Limits {
            solve_identities: limit,
            ..Limits::blind_cubic()
        })?;
        let result =
            fixture
                .cas
                .check_proof(&fixture.context, std::slice::from_ref(&step), &mut budget);
        if succeeds {
            result?;
            assert_eq!(budget.usage().solve_identities, 1);
        } else {
            let fault = result.err().ok_or("accepted zero budget")?;
            assert!(matches!(
                fault.kind,
                nrz_cas::FaultKind::Exhausted {
                    budget: BudgetKind::SolveIdentities,
                    ..
                }
            ));
            assert_eq!(fixture.cas.counts(), before);
        }
    }
    Ok(())
}

/// Exact binomial order and complete products work in more than one degree.
/// ´test:integration:binomial-complete-products´
#[test]
fn binomial_complete_products() -> TestResult {
    for order in [2, 3, 5] {
        let mut fixture = crate::testing::solve_elementary::BinomialFixture::new(order, 2)?;
        let checked = fixture.base.cas.check_proof(
            &fixture.base.context,
            &fixture.steps,
            &mut Budget::new(Limits {
                tower_degree: 32,
                ..Limits::blind_cubic()
            })?,
        )?;
        assert_eq!(checked.hypotheses(), []);
        assert_eq!(checked.judgment().residuals, []);
        assert!(
            matches!(&checked.judgment().statement, nrz_cas::proof::Statement::Solve(statement) if matches!(statement.as_ref(), nrz_cas::solving::SolveStatement::Coverage(_)))
        );
    }
    let (mut base, steps) = crate::testing::solve_elementary::constant_cover()?;
    let checked = base.cas.check_proof(
        &base.context,
        &steps,
        &mut Budget::new(Limits {
            tower_degree: 32,
            ..Limits::blind_cubic()
        })?,
    )?;
    assert_eq!(checked.judgment().residuals, []);
    Ok(())
}

/// Wrong order, prime factorization, omitted factors and changed multiplicities cannot certify coverage.
/// ´test:integration:binomial-product-mutations´
#[test]
fn binomial_product_mutations() -> TestResult {
    let mut fixture = crate::testing::solve_elementary::BinomialFixture::new(2, 2)?;
    for mutation in 0..4 {
        let mut steps = fixture.steps.clone();
        let index = if mutation < 2 { 2 } else { 6 };
        let Certificate::Solve(certificate) = &mut steps
            .get_mut(index)
            .ok_or("missing certificate")?
            .certificate
        else {
            return Err("wrong certificate".into());
        };
        match certificate.as_mut() {
            SolveCertificate::BinomialCoverage(value) => {
                if mutation == 0 {
                    value.order = 3;
                } else {
                    value.factors.first_mut().ok_or("missing prime")?.prime = 4;
                }
            }
            SolveCertificate::ProductCompleteness(value) => {
                if mutation == 2 {
                    value.factors.pop();
                } else {
                    value
                        .factors
                        .first_mut()
                        .ok_or("missing factor")?
                        .multiplicity = 2;
                }
            }
            _ => return Err("wrong variant".into()),
        }
        let before = fixture.base.cas.counts();
        assert!(
            fixture
                .base
                .cas
                .check_proof(
                    &fixture.base.context,
                    &steps,
                    &mut Budget::new(Limits::blind_cubic())?
                )
                .is_err()
        );
        assert_eq!(fixture.base.cas.counts(), before);
    }
    Ok(())
}

/// A constructive witness proves existence; the linear uniqueness subcase checks its leading coefficient separately.
/// ´test:integration:root-refinement-witness-mutations´
#[test]
fn root_refinement_witness_mutations() -> TestResult {
    for unique in [false, true] {
        for (value, succeeds) in [(2, true), (-2, false)] {
            let (mut fixture, step) =
                crate::testing::solve_elementary::root_refinement(value, unique)?;
            let before = fixture.cas.counts();
            let result = fixture.cas.check_proof(
                &fixture.context,
                std::slice::from_ref(&step),
                &mut Budget::new(Limits::blind_cubic())?,
            );
            if succeeds {
                assert_eq!(result?.hypotheses(), []);
            } else {
                assert!(result.is_err());
                assert_eq!(fixture.cas.counts(), before);
            }
        }
    }
    Ok(())
}

/// Zero radicands retain one root while translations recover the original quadratic exactly.
/// ´test:integration:quadratic-translation-and-zero-radicand´
#[test]
fn quadratic_translation_and_zero_radicand() -> TestResult {
    use crate::testing::solve_elementary::BinomialFixture;
    for mut fixture in [
        BinomialFixture::zero(2)?,
        BinomialFixture::zero(3)?,
        BinomialFixture::translated()?,
    ] {
        let checked = fixture.base.cas.check_proof(
            &fixture.base.context,
            &fixture.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        assert_eq!(checked.hypotheses(), []);
        assert_eq!(checked.judgment().residuals, []);
    }
    Ok(())
}
