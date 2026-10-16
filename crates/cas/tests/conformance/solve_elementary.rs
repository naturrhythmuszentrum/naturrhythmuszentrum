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

/// Universal recovery covers every zero-polynomial solution and refuses a changed inverse.
/// ´test:integration:identity-family-recovery-mutations´
#[test]
fn identity_family_recovery_mutations() -> TestResult {
    for (offset, succeeds) in [(0, true), (1, false), (-1, false)] {
        let (mut fixture, steps) = crate::testing::solve_elementary::identity_family(offset)?;
        let before = fixture.cas.counts();
        let result = fixture.cas.check_proof(
            &fixture.context,
            &steps,
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

/// Pure parameterization/recovery orientations admit; mixed roles and same-role edges refuse atomically.
/// ´test:integration:family-map-orientation-admission´
#[test]
fn family_map_orientation_admission() -> TestResult {
    use nrz_cas::solving::CoordinateRole::{Family, Unknown};
    for (source, target, succeeds) in [
        (vec![Family], vec![Unknown], true),
        (vec![Unknown], vec![Family], true),
        (vec![Family], vec![Family], false),
        (vec![Unknown], vec![Unknown], false),
        (vec![Family, Unknown], vec![Unknown], false),
        (vec![Family], vec![Unknown, Family], false),
    ] {
        let (mut fixture, draft) = crate::testing::solve_elementary::role_map(&source, &target)?;
        let before = fixture.cas.counts();
        let result = fixture.cas.admit(
            std::slice::from_ref(&draft),
            &mut Budget::new(Limits::blind_cubic())?,
        );
        if succeeds {
            result?;
        } else {
            assert!(result.is_err());
            assert_eq!(fixture.cas.counts(), before);
        }
    }
    Ok(())
}

/// Rank uses exact row operations, a nonzero minor and zero rows; each independent mutation fails.
/// ´test:integration:character-rank-mutations´
#[test]
fn character_rank_mutations() -> TestResult {
    let (mut fixture, valid) = crate::testing::solve_elementary::character_rank()?;
    fixture.cas.check_proof(
        &fixture.context,
        std::slice::from_ref(&valid),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    for mutation in 0..4 {
        let mut step = valid.clone();
        let Certificate::Solve(certificate) = &mut step.certificate else {
            return Err("wrong certificate".into());
        };
        let SolveCertificate::ExactLinearEquivalence(value) = certificate.as_mut() else {
            return Err("wrong linear certificate".into());
        };
        match mutation {
            0 => {
                value.operations.clear();
            }
            1 => {
                value.pivots.pop();
            }
            2 => {
                value.minor = fixture.integer(0, false)?;
            }
            _ => {
                value.pivots.reverse();
            }
        }
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

/// Standard conjugation fixes the symmetric sum, but never an arbitrary complex variable.
/// ´test:integration:conjugation-fixed-field-mutations´
#[test]
fn conjugation_fixed_field_mutations() -> TestResult {
    for symmetric in [true, false] {
        let (mut fixture, step) = crate::testing::solve_elementary::conjugate_real(symmetric)?;
        let before = fixture.cas.counts();
        let result = fixture.cas.check_proof(
            &fixture.context,
            std::slice::from_ref(&step),
            &mut Budget::new(Limits::blind_cubic())?,
        );
        if symmetric {
            assert_eq!(result?.judgment().residuals, []);
        } else {
            assert!(result.is_err());
            assert_eq!(fixture.cas.counts(), before);
        }
    }
    Ok(())
}

/// Repeated zero has multiplicity two; changing either the requested order or derivative value refuses.
/// ´test:integration:multiplicity-derivative-mutations´
#[test]
fn multiplicity_derivative_mutations() -> TestResult {
    let mut fixture = crate::testing::solve_elementary::BinomialFixture::multiplicity()?;
    fixture.base.cas.check_proof(
        &fixture.base.context,
        &fixture.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    for mutation in 0..2 {
        let mut steps = fixture.steps.clone();
        let step = steps.last_mut().ok_or("missing multiplicity")?;
        let Certificate::Solve(certificate) = &mut step.certificate else {
            return Err("wrong certificate".into());
        };
        let SolveCertificate::MultiplicityTransfer(value) = certificate.as_mut() else {
            return Err("wrong multiplicity".into());
        };
        if mutation == 0 {
            value.multiplicities = vec![1];
        } else {
            value.values.pop();
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

/// Every nested root presentation is checked, and the requested form must match exactly.
/// ´test:integration:presentation-form-mutations´
#[test]
fn presentation_form_mutations() -> TestResult {
    let mut fixture = crate::testing::solve_elementary::BinomialFixture::presentation()?;
    fixture.base.cas.check_proof(
        &fixture.base.context,
        &fixture.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let mut steps = fixture.steps.clone();
    let Certificate::Solve(certificate) =
        &mut steps.last_mut().ok_or("missing presentation")?.certificate
    else {
        return Err("wrong certificate".into());
    };
    let SolveCertificate::RootSelectionRefinement(value) = certificate.as_mut() else {
        return Err("wrong refinement".into());
    };
    value.form = Some(nrz_cas::solving::SolveForm::AlgebraicRoots);
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
    Ok(())
}

/// Real restriction retains both real roots or proves both nonreal, and rejects an omitted branch.
/// ´test:integration:real-restriction-partition-mutations´
#[test]
fn real_restriction_partition_mutations() -> TestResult {
    for nonreal in [false, true] {
        let mut fixture = crate::testing::solve_elementary::real_restriction(nonreal)?;
        fixture.base.cas.check_proof(
            &fixture.base.context,
            &fixture.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let mut steps = fixture.steps.clone();
        let Certificate::Solve(certificate) =
            &mut steps.last_mut().ok_or("missing restriction")?.certificate
        else {
            return Err("wrong certificate".into());
        };
        let SolveCertificate::RealRestriction(value) = certificate.as_mut() else {
            return Err("wrong restriction".into());
        };
        if nonreal {
            value.rejected.clear();
        } else {
            value.retained.clear();
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

/// A recovery with compatible fields but a different parameter product is refused at the proposition boundary.
/// ´test:integration:family-recovery-exact-space-pairing´
#[test]
fn family_recovery_exact_space_pairing() -> TestResult {
    use nrz_cas::proof::Statement;
    use nrz_cas::solving::SolveStatement;
    let (mut fixture, mut steps) = crate::testing::solve_elementary::identity_family(0)?;
    steps.pop();
    let step = steps.last_mut().ok_or("missing family coverage")?;
    let Statement::Solve(statement) = &mut step.conclusion else {
        return Err("wrong statement".into());
    };
    let SolveStatement::FamilyCoverage(value) = statement.as_mut() else {
        return Err("wrong family".into());
    };
    let other = fixture.affine_named(&value.problem, 0, "y")?;
    value.recovery = fixture.recovery(&other, 0)?;
    let Certificate::Solve(certificate) = &mut step.certificate else {
        return Err("wrong certificate".into());
    };
    let SolveCertificate::ParameterImageCoverage(proof) = certificate.as_mut() else {
        return Err("wrong recovery".into());
    };
    proof.recovery = Some(value.recovery.clone());
    let before = fixture.cas.counts();
    assert!(
        fixture
            .cas
            .check_proof(
                &fixture.context,
                &steps,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// An exhaustive degree partition retains the whole zero-leading family and refuses a missing branch.
/// ´test:integration:degree-drop-family-cover´
#[test]
fn degree_drop_family_cover() -> TestResult {
    let mut fixture = crate::testing::solve_elementary::degree_drop_cover()?;
    let checked = fixture.base.cas.check_proof(
        &fixture.base.context,
        &fixture.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(checked.hypotheses(), []);
    assert_eq!(checked.judgment().residuals, []);
    let mut steps = fixture.steps.clone();
    let Certificate::Solve(certificate) = &mut steps.last_mut().ok_or("missing cover")?.certificate
    else {
        return Err("wrong certificate".into());
    };
    let SolveCertificate::TotalPredicateSplit(value) = certificate.as_mut() else {
        return Err("wrong cover".into());
    };
    value.children.pop();
    value.branches.pop();
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
    Ok(())
}

/// Both consistent and contradictory systems reconstruct every row and preserve the augmented rank.
/// ´test:integration:linear-system-rank-mutations´
#[test]
fn linear_system_rank_mutations() -> TestResult {
    for inconsistent in [false, true] {
        let (mut fixture, mut step) =
            crate::testing::solve_elementary::linear_system(inconsistent)?;
        fixture.cas.check_proof(
            &fixture.context,
            std::slice::from_ref(&step),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let nrz_cas::proof::Statement::Solve(statement) = &mut step.conclusion else {
            return Err("wrong statement".into());
        };
        let nrz_cas::solving::SolveStatement::LinearSystem(value) = statement.as_mut() else {
            return Err("wrong system".into());
        };
        value.augmented_rank = if inconsistent { 1 } else { 2 };
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

/// A finite sample proves universal membership of its constant map but cannot prove whole-domain coverage.
/// ´test:integration:sampled-family-is-not-coverage´
#[test]
fn sampled_family_is_not_coverage() -> TestResult {
    let (mut fixture, steps) = crate::testing::solve_elementary::sampled_family()?;
    fixture.cas.check_proof(
        &fixture.context,
        steps.get(..1).ok_or("missing membership")?,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let before = fixture.cas.counts();
    assert!(
        fixture
            .cas
            .check_proof(
                &fixture.context,
                &steps,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// Two attempted character pivots succeed at N and refuse atomically at N-1.
/// ´test:integration:linear-pivot-budget-boundary´
#[test]
fn linear_pivot_budget_boundary() -> TestResult {
    for (limit, succeeds) in [(2, true), (1, false)] {
        let (mut fixture, step) = crate::testing::solve_elementary::character_rank()?;
        let before = fixture.cas.counts();
        let mut budget = Budget::new(Limits {
            linear_pivots: limit,
            ..Limits::blind_cubic()
        })?;
        let result =
            fixture
                .cas
                .check_proof(&fixture.context, std::slice::from_ref(&step), &mut budget);
        if succeeds {
            result?;
            assert_eq!(budget.usage().linear_pivots, 2);
        } else {
            let fault = result.err().ok_or("accepted low budget")?;
            assert!(matches!(
                fault.kind,
                nrz_cas::FaultKind::Exhausted {
                    budget: BudgetKind::LinearPivots,
                    ..
                }
            ));
            assert_eq!(fixture.cas.counts(), before);
        }
    }
    Ok(())
}

/// Both binomial coverage modes refuse to widen the original coefficient region.
/// ´test:integration:binomial-original-region-containment´
#[test]
fn binomial_original_region_containment() -> TestResult {
    for zero in [false, true] {
        for restricted in [false, true] {
            let mut fixture = crate::testing::solve_elementary::binomial_region(zero, restricted)?;
            let before = fixture.base.cas.counts();
            let result = fixture.base.cas.check_proof(
                &fixture.base.context,
                &fixture.steps,
                &mut Budget::new(Limits::blind_cubic())?,
            );
            if restricted {
                assert!(result.is_err());
                assert_eq!(fixture.base.cas.counts(), before);
            } else {
                let witness = result?;
                assert_eq!(witness.hypotheses(), []);
                assert_eq!(witness.judgment().residuals, []);
            }
        }
    }
    Ok(())
}

/// Consistent and contradictory linear systems allow restrictions and reject expansions of the original region.
/// ´test:integration:linear-original-region-containment´
#[test]
fn linear_original_region_containment() -> TestResult {
    for inconsistent in [false, true] {
        for contained in [false, true] {
            let mut fixture =
                crate::testing::solve_elementary::linear_region(inconsistent, contained)?;
            let before = fixture.base.cas.counts();
            let result = fixture.base.cas.check_proof(
                &fixture.base.context,
                &fixture.steps,
                &mut Budget::new(Limits::blind_cubic())?,
            );
            if contained {
                let witness = result?;
                assert_eq!(witness.hypotheses(), []);
                assert_eq!(witness.judgment().residuals, []);
            } else {
                assert!(result.is_err());
                assert_eq!(fixture.base.cas.counts(), before);
            }
        }
    }
    Ok(())
}

/// Existence remains valid on every stratum, but radical provenance requires the specialized leading term.
/// ´test:integration:radical-specialized-leading-coefficient´
#[test]
fn radical_specialized_leading_coefficient() -> TestResult {
    for stratum in 0..3 {
        let mut fixture = crate::testing::solve_elementary::specialized_presentation(stratum)?;
        fixture
            .base
            .cas
            .check_proof(
                &fixture.base.context,
                fixture.steps.get(..1).ok_or("missing existence")?,
                &mut Budget::new(Limits::blind_cubic())?,
            )
            .map_err(|error| format!("existence stratum {stratum}: {error}"))?;
        let before = fixture.base.cas.counts();
        let result = fixture.base.cas.check_proof(
            &fixture.base.context,
            &fixture.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        );
        if stratum == 1 {
            let witness = result?;
            assert_eq!(witness.hypotheses(), []);
            assert_eq!(witness.judgment().residuals, []);
        } else {
            assert!(result.is_err());
            assert_eq!(fixture.base.cas.counts(), before);
        }
    }
    Ok(())
}

/// A finite sample cannot claim the representation of an exact family image.
/// ´test:integration:exact-family-branch-shape´
#[test]
fn exact_family_branch_shape() -> TestResult {
    for image in [false, true] {
        let mut fixture = crate::testing::solve_elementary::family_presentation(image)?;
        fixture.base.cas.check_proof(
            &fixture.base.context,
            fixture.steps.get(..1).ok_or("missing membership")?,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let before = fixture.base.cas.counts();
        let result = fixture.base.cas.check_proof(
            &fixture.base.context,
            &fixture.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        );
        if image {
            let witness = result?;
            assert_eq!(witness.hypotheses(), []);
            assert_eq!(witness.judgment().residuals, []);
        } else {
            assert!(result.is_err());
            assert_eq!(fixture.base.cas.counts(), before);
        }
    }
    Ok(())
}

/// A general algebraic placeholder cannot discharge a recursively radical presentation request.
/// ´test:integration:general-root-is-not-radical-provenance´
#[test]
fn general_root_is_not_radical_provenance() -> TestResult {
    for radicals in [false, true] {
        let mut fixture = crate::testing::solve_elementary::general_presentation(radicals)?;
        fixture.base.cas.check_proof(
            &fixture.base.context,
            fixture.steps.get(..2).ok_or("missing existence")?,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let before = fixture.base.cas.counts();
        let result = fixture.base.cas.check_proof(
            &fixture.base.context,
            &fixture.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        );
        if radicals {
            assert!(result.is_err());
            assert_eq!(fixture.base.cas.counts(), before);
        } else {
            let witness = result?;
            assert_eq!(witness.hypotheses(), []);
            assert_eq!(witness.judgment().residuals, []);
        }
    }
    Ok(())
}

/// Existence of independent cube-root choices never supplies a branch-unsafe equality.
/// ´test:integration:independent-cube-root-equality-refused´
#[test]
fn independent_cube_root_equality_refused() -> TestResult {
    for independent in [false, true] {
        let mut fixture = crate::testing::solve_elementary::independent_cube_roots(independent)?;
        fixture.base.cas.check_proof(
            &fixture.base.context,
            fixture.steps.get(..3).ok_or("missing existence")?,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let before = fixture.base.cas.counts();
        let result = fixture.base.cas.check_proof(
            &fixture.base.context,
            &fixture.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        );
        if independent {
            assert!(result.is_err());
            assert_eq!(fixture.base.cas.counts(), before);
        } else {
            let witness = result?;
            assert_eq!(witness.hypotheses(), []);
            assert_eq!(witness.judgment().residuals, []);
        }
    }
    Ok(())
}

/// Symbolic roots, exact renamed declarations and distinct coefficient domains survive restriction.
/// ´test:integration:symbolic-real-restriction-strata´
#[test]
fn symbolic_real_restriction_strata() -> TestResult {
    for shape in 0..6 {
        let mut fixture = crate::testing::solve_elementary::symbolic_restriction(shape)?;
        let witness = fixture.base.cas.check_proof(
            &fixture.base.context,
            &fixture.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        assert_eq!(witness.hypotheses(), []);
        assert_eq!(witness.judgment().residuals, []);
    }
    Ok(())
}

/// A false coefficient map, changed region and display-only declaration pairing are refused atomically.
/// ´test:integration:symbolic-real-restriction-transport-mutations´
#[test]
fn symbolic_real_restriction_transport_mutations() -> TestResult {
    for mutation in 0..3 {
        let mut fixture = crate::testing::solve_elementary::symbolic_restriction(0)?;
        let algebra = fixture.base.algebra.clone();
        let false_map = fixture.base.scalar_map(&algebra, &[])?;
        let mut steps = fixture.steps.clone();
        let Certificate::Solve(certificate) =
            &mut steps.last_mut().ok_or("missing restriction")?.certificate
        else {
            return Err("wrong certificate".into());
        };
        let SolveCertificate::RealRestriction(value) = certificate.as_mut() else {
            return Err("wrong restriction".into());
        };
        match mutation {
            0 => value.transport.coefficient_map = false_map,
            1 => {
                let changed = value
                    .transport
                    .regions
                    .get(1)
                    .ok_or("missing stratum")?
                    .real
                    .clone();
                value
                    .transport
                    .regions
                    .first_mut()
                    .ok_or("missing overall region")?
                    .real = changed;
            }
            _ => {
                let wrong = value
                    .transport
                    .coefficients
                    .get(1)
                    .ok_or("missing coefficient")?
                    .complex
                    .clone();
                value
                    .transport
                    .coefficients
                    .first_mut()
                    .ok_or("missing coefficient")?
                    .complex = wrong;
            }
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

/// Used hypotheses become target-context evidence or explicit residuals, never foreign identities.
/// ´test:integration:symbolic-real-restriction-conditions´
#[test]
fn symbolic_real_restriction_conditions() -> TestResult {
    for discharge in [false, true] {
        let mut fixture = crate::testing::solve_elementary::assumed_restriction(discharge)?;
        let witness = fixture.base.cas.check_proof(
            &fixture.base.context,
            &fixture.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        if discharge {
            assert_eq!(witness.hypotheses().len(), 1);
            assert_eq!(
                witness
                    .hypotheses()
                    .first()
                    .ok_or("missing target hypothesis")?
                    .context,
                fixture.base.context
            );
            assert_eq!(witness.judgment().residuals, []);
        } else {
            assert_eq!(witness.hypotheses(), []);
            assert_eq!(witness.judgment().residuals.len(), 1);
            assert_eq!(
                witness
                    .judgment()
                    .residuals
                    .first()
                    .ok_or("missing residual")?
                    .home,
                fixture.base.context
            );
        }
        let mut steps = fixture.steps.clone();
        let Certificate::Solve(certificate) =
            &mut steps.last_mut().ok_or("missing restriction")?.certificate
        else {
            return Err("wrong certificate".into());
        };
        let SolveCertificate::RealRestriction(value) = certificate.as_mut() else {
            return Err("wrong restriction".into());
        };
        value.transport.conditions.clear();
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

/// Closed zero/nonzero parameter strata keep the identity family and singleton independently.
/// ´test:integration:symbolic-real-restriction-degree-cover´
#[test]
fn symbolic_real_restriction_degree_cover() -> TestResult {
    let mut fixture = crate::testing::solve_elementary::degree_restriction()?;
    let witness = fixture.base.cas.check_proof(
        &fixture.base.context,
        &fixture.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(witness.hypotheses(), []);
    assert_eq!(witness.judgment().residuals, []);
    Ok(())
}

/// The parameterized cubic retains its real linear factor and independently excludes both quadratic roots.
/// ´test:integration:symbolic-real-restriction-mixed-cubic´
#[test]
fn symbolic_real_restriction_mixed_cubic() -> TestResult {
    let mut fixture = crate::testing::solve_elementary::mixed_restriction()?;
    fixture.base.cas.check_proof(
        &fixture.base.context,
        &fixture.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let mut steps = fixture.steps.clone();
    let Certificate::Solve(certificate) =
        &mut steps.last_mut().ok_or("missing restriction")?.certificate
    else {
        return Err("wrong certificate".into());
    };
    let SolveCertificate::RealRestriction(value) = certificate.as_mut() else {
        return Err("wrong restriction".into());
    };
    let rejected = value.rejected.pop().ok_or("missing rejected root")?;
    value.retained.push(rejected);
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
    Ok(())
}

/// Constant fractions and towers discharge one; a parameter requires its actual regional guard.
/// ´test:integration:embedded-ground-units-retain-parameter-guards´
#[test]
fn embedded_ground_units_retain_parameter_guards() -> TestResult {
    let mut unguarded = crate::testing::solve_elementary::embedded_fraction(false)?;
    let rank = unguarded.steps.pop().ok_or("missing rank")?;
    unguarded.base.cas.check_proof(
        &unguarded.base.context,
        &unguarded.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    unguarded.steps.push(rank);
    let before = unguarded.base.cas.counts();
    assert!(
        unguarded
            .base
            .cas
            .check_proof(
                &unguarded.base.context,
                &unguarded.steps,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    assert_eq!(unguarded.base.cas.counts(), before);
    for mut fixture in [
        crate::testing::solve_elementary::embedded_fraction(true)?,
        crate::testing::solve_elementary::embedded_tower_one()?,
    ] {
        fixture.base.cas.check_proof(
            &fixture.base.context,
            &fixture.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
    }
    Ok(())
}

/// Exact quotient inverses support both echelon pivots and reversible scaling.
/// ´test:integration:checked-inverse-pivots´
#[test]
fn checked_inverse_pivots() -> TestResult {
    for scale in [false, true] {
        let mut fixture = crate::testing::solve_elementary::inverse_pivot(scale)?;
        let witness = fixture.base.cas.check_proof(
            &fixture.base.context,
            &fixture.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        assert_eq!(witness.hypotheses(), []);
        assert_eq!(witness.judgment().residuals, []);
    }
    Ok(())
}

/// Wrong inverses and changes to the exact map, context or region cannot certify a pivot.
/// ´test:integration:checked-inverse-pivot-mutations´
#[test]
fn checked_inverse_pivot_mutations() -> TestResult {
    for mutation in 0..4 {
        let mut fixture =
            crate::testing::solve_elementary::inverse_pivot_mutation(false, mutation)?;
        let before = fixture.base.cas.counts();
        assert!(
            fixture
                .base
                .cas
                .check_proof(
                    &fixture.base.context,
                    &fixture.steps,
                    &mut Budget::new(Limits::blind_cubic())?
                )
                .is_err()
        );
        assert_eq!(fixture.base.cas.counts(), before);
    }
    Ok(())
}

/// Two levels of total coefficient splits retain closed leaf representations and complete coverage.
/// ´test:integration:closed-strata-presentations´
#[test]
fn closed_strata_presentations() -> TestResult {
    let mut fixture = crate::testing::solve_elementary::closed_presentations()?;
    let checked = fixture.base.cas.check_proof(
        &fixture.base.context,
        &fixture.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(checked.hypotheses(), []);
    assert_eq!(checked.judgment().residuals, []);
    Ok(())
}

/// Complete child proofs cannot supply missing, widened, differently formed or foreign branches.
/// ´test:integration:closed-strata-presentation-mutations´
#[test]
fn closed_strata_presentation_mutations() -> TestResult {
    for mutation in 0..5 {
        let mut fixture = crate::testing::solve_elementary::closed_presentation_mutation(mutation)?;
        let roots = fixture
            .steps
            .last()
            .ok_or("missing final presentation")?
            .premises
            .clone();
        for root in roots {
            let prefix = fixture
                .steps
                .get(..=usize::try_from(root)?)
                .ok_or("missing premise prefix")?;
            let closure = crate::testing::solve_elementary::presentation_closure(prefix)?;
            fixture
                .base
                .cas
                .check_proof(
                    &fixture.base.context,
                    &closure,
                    &mut Budget::new(Limits::blind_cubic())?,
                )
                .map_err(|fault| format!("mutation {mutation}, premise {root}: {fault:?}"))?;
        }
        let before = fixture.base.cas.counts();
        let fault = fixture
            .base
            .cas
            .check_proof(
                &fixture.base.context,
                &fixture.steps,
                &mut Budget::new(Limits::blind_cubic())?,
            )
            .err()
            .ok_or("false presentation accepted")?;
        assert_eq!(fault.location.operation, "elementary solving");
        assert_eq!(fixture.base.cas.counts(), before);
    }
    Ok(())
}

/// Polynomial equality neither blocks a scalar inverse nor supplies a missing one.
/// ´test:integration:inverse-pivot-skips-polynomial-identities´
#[test]
fn inverse_pivot_skips_polynomial_identities() -> TestResult {
    for with_scalar in [true, false] {
        let (mut fixture, baseline) =
            crate::testing::solve_elementary::inverse_pivot_scan(with_scalar)?;
        let original = fixture.base.cas.check_proof(
            &fixture.base.context,
            &baseline,
            &mut Budget::new(Limits::blind_cubic())?,
        );
        let before = fixture.base.cas.counts();
        let result = fixture.base.cas.check_proof(
            &fixture.base.context,
            &fixture.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        );
        if with_scalar {
            assert_eq!(result?.judgment(), original?.judgment());
        } else {
            let expected = original.err().ok_or("missing inverse accepted")?;
            let actual = result
                .err()
                .ok_or("polynomial identity accepted as an inverse")?;
            assert!(matches!(
                actual.kind,
                nrz_cas::FaultKind::Invalid(nrz_cas::InvalidCode::Rule)
            ));
            assert_eq!(actual.kind, expected.kind);
            assert_eq!(actual.location, expected.location);
            assert_eq!(fixture.base.cas.counts(), before);
        }
    }
    Ok(())
}

/// Missing linkage, map/region changes, a rotated real choice and a changed radicand cannot classify roots.
/// ´test:integration:linked-cubic-realness-mutations´
#[test]
fn linked_cubic_realness_mutations() -> TestResult {
    for bombelli in [false, true] {
        for mutation in 0..6 {
            let mut fixture = crate::testing::solve_elementary::linked_cubic(bombelli, mutation)?;
            let rejected = match mutation {
                3 | 4 => fixture.steps.iter().position(|step| matches!(&step.certificate,
                    Certificate::Solve(certificate) if matches!(certificate.as_ref(), SolveCertificate::RootSelectionRefinement(_))))
                    .ok_or("missing choice")?,
                0 | 5 => fixture.steps.len().checked_sub(2).ok_or("missing conjugation")?,
                _ => fixture.steps.len().checked_sub(1).ok_or("missing classification")?,
            };
            let prefix = fixture
                .steps
                .get(..rejected)
                .ok_or("missing valid prefix")?;
            fixture.base.cas.check_proof(
                &fixture.base.context,
                prefix,
                &mut Budget::new(Limits::blind_cubic())?,
            )?;
            let checked = fixture.base.cas.check_proof(
                &fixture.base.context,
                &fixture.steps,
                &mut Budget::new(Limits::blind_cubic())?,
            );
            let Err(fault) = checked else {
                return Err("mutated realness was accepted".into());
            };
            assert_eq!(
                fault.kind,
                nrz_cas::FaultKind::Invalid(nrz_cas::InvalidCode::Rule),
                "Bombelli {bombelli}, mutation {mutation}"
            );
        }
    }
    Ok(())
}

/// Formal nonzero denominators cannot establish defined real values on an unguarded specialization.
/// ´test:integration:fraction-realness-requires-denominator´
#[test]
fn fraction_realness_requires_denominator() -> TestResult {
    let mut fixture = crate::testing::solve_elementary::fraction_realness(false)?;
    assert!(
        fixture
            .base
            .cas
            .check_proof(
                &fixture.base.context,
                &fixture.steps,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    Ok(())
}

/// An undeclared field coercion cannot supply the real root choice used by later classification.
/// ´test:integration:real-selection-rejects-complex-witness´
#[test]
fn real_selection_rejects_complex_witness() -> TestResult {
    let mut fixture = crate::testing::solve_elementary::choice_wrong_ambient()?;
    assert!(
        fixture
            .base
            .cas
            .check_proof(
                &fixture.base.context,
                &fixture.steps,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    Ok(())
}

/// Realness checking preserves the original refusal of an undefined zero power.
/// ´test:integration:complex-realness-preserves-partial-powers´
#[test]
fn complex_realness_preserves_partial_powers() -> TestResult {
    let mut fixture = crate::testing::solve_elementary::partial_power_realness()?;
    assert!(
        fixture
            .base
            .cas
            .check_proof(
                &fixture.base.context,
                &fixture.steps,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    Ok(())
}

/// An earlier checked equality cannot support transport when its evidence slot is omitted.
/// ´test:integration:conjugation-rejects-unreferenced-evidence´
#[test]
fn conjugation_rejects_unreferenced_evidence() -> TestResult {
    let mut fixture = crate::testing::solve_elementary::referenced_realness(0)?;
    let prefix = fixture.steps.get(..2).ok_or("missing checked equality")?;
    fixture.base.cas.check_proof(
        &fixture.base.context,
        prefix,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let checked = fixture.base.cas.check_proof(
        &fixture.base.context,
        &fixture.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    );
    let Err(fault) = checked else {
        return Err("unreferenced evidence was consumed".into());
    };
    assert_eq!(
        fault.kind,
        nrz_cas::FaultKind::Invalid(nrz_cas::InvalidCode::Rule)
    );
    Ok(())
}
