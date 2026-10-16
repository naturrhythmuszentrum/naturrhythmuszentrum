// SPDX-License-Identifier: AGPL-3.0-only
//! Public canonical fraction admission, sharing, resource and atomicity contracts.

use crate::testing::common::TestResult;
use crate::testing::solve_coefficients::Fixture;
use nrz_cas::FaultKind;
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::solving::{CoefficientValueData, SolveView};

/// Coprime x and y are admitted without asserting a multivariate Bezout-to-one identity.
/// ´test:integration:fraction-coprime-variables´
#[test]
fn fraction_coprime_variables() -> TestResult {
    let mut fixture = Fixture::new()?;
    let numerator = fixture.polynomial(2, &[(vec![1, 0], 1)])?;
    let denominator = fixture.polynomial(2, &[(vec![0, 1], 1)])?;
    let draft = fixture.fraction(&numerator, &denominator);
    let values = fixture.cas.admit(
        std::slice::from_ref(&draft),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let value = values.first().ok_or("missing fraction")?;
    let SolveView::CoefficientValue(data) = fixture.cas.solve_view(value)? else {
        return Err("wrong fraction view".into());
    };
    assert_eq!(
        data.value,
        CoefficientValueData::Fraction {
            numerator,
            denominator
        }
    );
    let before = fixture.cas.counts();
    let shared = fixture.cas.admit(
        std::slice::from_ref(&draft),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(shared, values);
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// A common polynomial factor and a nonmonic denominator independently invalidate admission.
/// ´test:integration:fraction-normalization-mutations´
#[test]
fn fraction_normalization_mutations() -> TestResult {
    let mut fixture = Fixture::new()?;
    let numerator = fixture.polynomial(2, &[(vec![2, 0], 1), (vec![0, 0], -1)])?;
    let common = fixture.polynomial(2, &[(vec![1, 0], 1), (vec![0, 0], -1)])?;
    let scaled = fixture.polynomial(2, &[(vec![0, 1], 2), (vec![0, 0], 2)])?;
    for denominator in [&common, &scaled] {
        let draft = fixture.fraction(&numerator, denominator);
        let before = fixture.cas.counts();
        assert!(
            fixture
                .cas
                .admit(
                    std::slice::from_ref(&draft),
                    &mut Budget::new(Limits::blind_cubic())?
                )
                .is_err()
        );
        assert_eq!(fixture.cas.counts(), before);
    }
    Ok(())
}

/// Canonical gcd admission succeeds at its measured operation count and refuses at one fewer atomically.
/// ´test:integration:fraction-gcd-budget-boundary´
#[test]
fn fraction_gcd_budget_boundary() -> TestResult {
    let mut fixture = Fixture::new()?;
    let numerator = fixture.polynomial(2, &[(vec![1, 0], 1), (vec![0, 0], 1)])?;
    let denominator = fixture.polynomial(2, &[(vec![0, 1], 1)])?;
    let draft = fixture.fraction(&numerator, &denominator);
    let mut measured = Budget::new(Limits::blind_cubic())?;
    fixture
        .cas
        .admit(std::slice::from_ref(&draft), &mut measured)?;
    let count = measured.usage().gcd_steps;
    assert!(count > 0);
    for (limit, succeeds) in [(count, true), (count - 1, false)] {
        let mut peer = Fixture::new()?;
        let top = peer.polynomial(2, &[(vec![1, 0], 1), (vec![0, 0], 1)])?;
        let bottom = peer.polynomial(2, &[(vec![0, 1], 1)])?;
        let proposed = peer.fraction(&top, &bottom);
        let before = peer.cas.counts();
        let mut budget = Budget::new(Limits {
            gcd_steps: limit,
            ..Limits::blind_cubic()
        })?;
        let result = peer.cas.admit(std::slice::from_ref(&proposed), &mut budget);
        if succeeds {
            assert!(result.is_ok());
        } else {
            let failure = result.err().ok_or("short gcd budget accepted")?;
            assert!(matches!(
                failure.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::GcdSteps,
                    ..
                }
            ));
            assert_eq!(peer.cas.counts(), before);
        }
    }
    Ok(())
}

/// Unit replay checks the inverse, gcd quotients and Bezout identities independently and atomically.
/// ´test:integration:tower-unit-mutations´
#[test]
fn tower_unit_mutations() -> TestResult {
    use crate::testing::solve_coefficients::UnitFixture;
    use nrz_cas::proof::{Certificate, Statement};
    use nrz_cas::solving::{SolveCertificate, SolveStatement};
    let mut fixture = UnitFixture::new()?;
    let valid_step = fixture.step()?;
    let checked = fixture.base.cas.check_proof(
        &fixture.base.context,
        std::slice::from_ref(&valid_step),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(checked.hypotheses(), []);
    assert_eq!(checked.judgment().residuals, []);
    let Statement::Solve(statement) = &checked.judgment().statement else {
        return Err("wrong unit judgment".into());
    };
    let SolveStatement::Unit(unit) = statement.as_ref() else {
        return Err("wrong unit conclusion".into());
    };
    assert_eq!(unit.value, fixture.generator);
    for mutation in 0..3 {
        let mut step = valid_step.clone();
        if mutation == 0 {
            let Statement::Solve(statement) = &mut step.conclusion else {
                return Err("wrong unit statement".into());
            };
            let SolveStatement::Unit(unit) = statement.as_mut() else {
                return Err("wrong unit variant".into());
            };
            unit.inverse.clone_from(&fixture.zero);
        } else {
            let Certificate::Solve(certificate) = &mut step.certificate else {
                return Err("wrong certificate".into());
            };
            let SolveCertificate::ExtensionUnit(unit) = certificate.as_mut() else {
                return Err("wrong unit certificate".into());
            };
            if mutation == 1 {
                unit.gcd.clone_from(&fixture.modulus);
            } else {
                unit.quotients.swap(0, 1);
            }
        }
        let before = fixture.base.cas.counts();
        assert!(
            fixture
                .base
                .cas
                .check_proof(
                    &fixture.base.context,
                    std::slice::from_ref(&step),
                    &mut Budget::new(Limits::blind_cubic())?
                )
                .is_err()
        );
        assert_eq!(fixture.base.cas.counts(), before);
    }
    Ok(())
}

/// Both coprime components are covered; changed Bezout data and a selected root sent to the wrong child refuse.
/// ´test:integration:tower-split-mutations´
#[test]
fn tower_split_mutations() -> TestResult {
    use crate::testing::solve_coefficients::SplitFixture;
    use nrz_cas::proof::Certificate;
    use nrz_cas::solving::SolveCertificate;
    let mut fixture = SplitFixture::new()?;
    let checked = fixture.unit.base.cas.check_proof(
        &fixture.unit.base.context,
        &fixture.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(checked.hypotheses(), []);
    assert_eq!(checked.judgment().residuals, []);
    let children = fixture.children()?;
    let selections = fixture.selections()?;
    assert_ne!(children[0], children[1]);
    assert_ne!(selections[0], selections[1]);
    for mutation in 0..3 {
        let mut steps = fixture.steps.clone();
        let last = steps.last_mut().ok_or("missing split")?;
        let Certificate::Solve(certificate) = &mut last.certificate else {
            return Err("wrong certificate".into());
        };
        let SolveCertificate::ComponentSplit(split) = certificate.as_mut() else {
            return Err("wrong split certificate".into());
        };
        match mutation {
            0 => split.selections.swap(0, 1),
            1 => split.bezout.swap(0, 1),
            _ => split.square_free = 0,
        }
        let before = fixture.unit.base.cas.counts();
        assert!(
            fixture
                .unit
                .base
                .cas
                .check_proof(
                    &fixture.unit.base.context,
                    &steps,
                    &mut Budget::new(Limits::blind_cubic())?
                )
                .is_err()
        );
        assert_eq!(fixture.unit.base.cas.counts(), before);
    }
    Ok(())
}

/// Tower certificate replay has a measured arithmetic boundary and publishes no prefix on refusal.
/// ´test:integration:tower-replay-arithmetic-boundary´
#[test]
fn tower_replay_arithmetic_boundary() -> TestResult {
    use crate::testing::solve_coefficients::UnitFixture;
    let mut fixture = UnitFixture::new()?;
    let valid_step = fixture.step()?;
    let mut measured = Budget::new(Limits::blind_cubic())?;
    fixture.base.cas.check_proof(
        &fixture.base.context,
        std::slice::from_ref(&valid_step),
        &mut measured,
    )?;
    let count = measured.usage().arithmetic_steps;
    assert!(count > 0);
    for (limit, succeeds) in [(count, true), (count - 1, false)] {
        let mut fresh = UnitFixture::new()?;
        let fresh_step = fresh.step()?;
        let before = fresh.base.cas.counts();
        let result = fresh.base.cas.check_proof(
            &fresh.base.context,
            std::slice::from_ref(&fresh_step),
            &mut Budget::new(Limits {
                arithmetic_steps: limit,
                ..Limits::blind_cubic()
            })?,
        );
        if succeeds {
            assert!(result.is_ok());
        } else {
            let failure = result.err().ok_or("short arithmetic budget accepted")?;
            assert!(matches!(
                failure.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::ArithmeticSteps,
                    ..
                }
            ));
            assert_eq!(fresh.base.cas.counts(), before);
        }
    }
    Ok(())
}

/// Independent Gaussian multiplication certifies (2+i)^3 = 2+11i and rejects a sign mutation.
/// ´test:integration:coefficient-identity-mutation´
#[test]
fn coefficient_identity_mutation() -> TestResult {
    use crate::testing::solve_coefficients::gaussian_identity;
    let (mut valid, step) = gaussian_identity(11)?;
    let checked = valid.cas.check_proof(
        &valid.context,
        std::slice::from_ref(&step),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(checked.hypotheses(), []);
    assert_eq!(checked.judgment().residuals, []);
    let (mut false_claim, changed) = gaussian_identity(-11)?;
    let before = false_claim.cas.counts();
    assert!(
        false_claim
            .cas
            .check_proof(
                &false_claim.context,
                std::slice::from_ref(&changed),
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    assert_eq!(false_claim.cas.counts(), before);
    assert_eq!(nrz_cas::solving::active_schema_bitmap() & 0b1101, 0b1101);
    Ok(())
}

/// FTA, primitive unity and the named real-root subcases check their exact existence hypotheses.
/// ´test:integration:root-adjunction-subcases´
#[test]
fn root_adjunction_subcases() -> TestResult {
    use crate::testing::solve_coefficients::AdjunctionFixture;
    use nrz_cas::proof::Statement;
    use nrz_cas::solving::{AdjunctionMode, SolveStatement};
    use nrz_cas::syntax::Field;
    let cases = [
        (
            vec![(vec![2], 1), (vec![0], 1)],
            Field::Complex,
            AdjunctionMode::ComplexFTA,
            None,
        ),
        (
            vec![(vec![2], 1), (vec![0], 1)],
            Field::Complex,
            AdjunctionMode::AlgebraicClosure,
            None,
        ),
        (
            vec![(vec![2], 1), (vec![1], 1), (vec![0], 1)],
            Field::Complex,
            AdjunctionMode::PrimitiveUnity,
            Some(3),
        ),
        (
            vec![(vec![6], 1), (vec![3], 1), (vec![0], 1)],
            Field::Complex,
            AdjunctionMode::PrimitiveUnity,
            Some(9),
        ),
        (
            vec![(vec![3], 1), (vec![0], -2)],
            Field::Real,
            AdjunctionMode::RealOddRoot,
            Some(3),
        ),
        (
            vec![(vec![3], 1), (vec![0], 8)],
            Field::Real,
            AdjunctionMode::RealOddRoot,
            Some(3),
        ),
        (
            vec![(vec![2], 1), (vec![0], -2)],
            Field::Real,
            AdjunctionMode::PositiveRealSquareRoot,
            Some(2),
        ),
    ];
    for (polynomial, field, mode, order) in cases {
        let mut fixture = AdjunctionFixture::new(&polynomial, &field, mode, order)?;
        let checked = fixture.base.cas.check_proof(
            &fixture.base.context,
            &fixture.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        assert_eq!(checked.hypotheses(), []);
        assert_eq!(checked.judgment().residuals, []);
        let Statement::Solve(statement) = &checked.judgment().statement else {
            return Err("wrong root statement".into());
        };
        let SolveStatement::RootExistence(existence) = statement.as_ref() else {
            return Err("existence was strengthened".into());
        };
        assert_eq!(existence.relation, fixture.selection);
        assert_ne!(fixture.relation, fixture.selection);
    }
    Ok(())
}

/// Changed primitive order, missing leading evidence, real sign and uniqueness claims are independently rejected.
/// ´test:integration:root-adjunction-mutations´
#[test]
fn root_adjunction_mutations() -> TestResult {
    use crate::testing::solve_coefficients::AdjunctionFixture;
    use nrz_cas::proof::{Certificate, Statement};
    use nrz_cas::solving::{
        AdjunctionMode, SolveCertificate, SolveStatement, StatementSelectionUnique,
    };
    use nrz_cas::syntax::Field;
    let mut fixture = AdjunctionFixture::new(
        &[(vec![2], 1), (vec![1], 1), (vec![0], 1)],
        &Field::Complex,
        AdjunctionMode::PrimitiveUnity,
        Some(3),
    )?;
    for mutation in 0..3 {
        let mut steps = fixture.steps.clone();
        let step = steps.last_mut().ok_or("missing root step")?;
        if mutation == 2 {
            step.conclusion = Statement::Solve(Box::new(SolveStatement::SelectionUnique(
                StatementSelectionUnique {
                    selection: fixture.selection.clone(),
                    region: fixture.base.region.clone(),
                },
            )));
        } else {
            let Certificate::Solve(certificate) = &mut step.certificate else {
                return Err("wrong root certificate".into());
            };
            let SolveCertificate::RootAdjunction(adjunction) = certificate.as_mut() else {
                return Err("wrong root variant".into());
            };
            if mutation == 0 {
                adjunction.order = Some(4);
            } else {
                adjunction.leading_unit = 1;
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
    let mut negative = AdjunctionFixture::new(
        &[(vec![2], 1), (vec![0], 1)],
        &Field::Real,
        AdjunctionMode::PositiveRealSquareRoot,
        Some(2),
    )?;
    let before = negative.base.cas.counts();
    assert!(
        negative
            .base
            .cas
            .check_proof(
                &negative.base.context,
                &negative.steps,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    assert_eq!(negative.base.cas.counts(), before);
    Ok(())
}

/// Primitive-unity construction and proof admission share an exact measured arithmetic boundary.
/// ´test:integration:root-adjunction-arithmetic-boundary´
#[test]
fn root_adjunction_arithmetic_boundary() -> TestResult {
    use crate::testing::solve_coefficients::AdjunctionFixture;
    use nrz_cas::solving::AdjunctionMode;
    use nrz_cas::syntax::Field;
    let polynomial = [(vec![2], 1), (vec![1], 1), (vec![0], 1)];
    let mut fixture = AdjunctionFixture::new(
        &polynomial,
        &Field::Complex,
        AdjunctionMode::PrimitiveUnity,
        Some(3),
    )?;
    let mut measured = Budget::new(Limits::blind_cubic())?;
    fixture
        .base
        .cas
        .check_proof(&fixture.base.context, &fixture.steps, &mut measured)?;
    let count = measured.usage().arithmetic_steps;
    assert!(count > 0);
    for (limit, succeeds) in [(count, true), (count - 1, false)] {
        let mut fresh = AdjunctionFixture::new(
            &polynomial,
            &Field::Complex,
            AdjunctionMode::PrimitiveUnity,
            Some(3),
        )?;
        let before = fresh.base.cas.counts();
        let result = fresh.base.cas.check_proof(
            &fresh.base.context,
            &fresh.steps,
            &mut Budget::new(Limits {
                arithmetic_steps: limit,
                ..Limits::blind_cubic()
            })?,
        );
        if succeeds {
            assert!(result.is_ok());
        } else {
            let failure = result.err().ok_or("short root budget accepted")?;
            assert!(matches!(
                failure.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::ArithmeticSteps,
                    ..
                }
            ));
            assert_eq!(fresh.base.cas.counts(), before);
        }
    }
    Ok(())
}

/// Zero has a nonnegative real square root, while strict positivity and principal metadata require their own evidence.
/// ´test:integration:real-root-zero-and-principal-refusal´
#[test]
fn real_root_zero_and_principal_refusal() -> TestResult {
    use crate::testing::solve_coefficients::AdjunctionFixture;
    use crate::testing::solve_graph::admit;
    use nrz_cas::Reference;
    use nrz_cas::solving::{
        AdjunctionMode, RootSelectionData, SelectionMode, SolveDraft, SolveView,
    };
    use nrz_cas::syntax::{Draft, Field};
    let mut fixture = AdjunctionFixture::new(
        &[(vec![2], 1)],
        &Field::Real,
        AdjunctionMode::PositiveRealSquareRoot,
        Some(2),
    )?;
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
    let selected = fixture.base.positive_selection(&fixture.relation, false)?;
    let root = fixture.base.adjunction(
        &fixture.relation,
        Some(&selected),
        AdjunctionMode::PositiveRealSquareRoot,
        Some(2),
    );
    let unit = fixture.steps.first().ok_or("missing unit")?.clone();
    fixture.base.cas.check_proof(
        &fixture.base.context,
        &[unit.clone(), root],
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let SolveView::RootSelection(data) = fixture.base.cas.solve_view(&selected)? else {
        return Err("wrong selection".into());
    };
    let draft = Draft::Solve(SolveDraft::RootSelection(RootSelectionData {
        header: Reference::Existing(data.header.clone()),
        scope: data.scope.clone(),
        relations: data
            .relations
            .iter()
            .cloned()
            .map(Reference::Existing)
            .collect(),
        predicate: Reference::Existing(data.predicate.clone()),
        mode: SelectionMode::Principal,
        convention: Some("positive-root".into()),
        interval: None,
    }));
    let principal = admit(&mut fixture.base.cas, &draft)?;
    let proposed = fixture.base.adjunction(
        &fixture.relation,
        Some(&principal),
        AdjunctionMode::PositiveRealSquareRoot,
        Some(2),
    );
    let before = fixture.base.cas.counts();
    assert!(
        fixture
            .base
            .cas
            .check_proof(
                &fixture.base.context,
                &[unit, proposed],
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    assert_eq!(fixture.base.cas.counts(), before);
    Ok(())
}

/// A formal nonzero denominator is guarded until the exact scoped parameter hypothesis is replayed.
/// ´test:integration:fraction-regional-definedness´
#[test]
fn fraction_regional_definedness() -> TestResult {
    use crate::testing::solve_graph::admit;
    use nrz_cas::syntax::PredicateKind;
    let mut fixture = Fixture::new()?;
    let one = fixture.polynomial(2, &[(vec![0, 0], 1)])?;
    let divisor = fixture.polynomial(2, &[(vec![1, 0], 1)])?;
    let draft = fixture.fraction(&one, &divisor);
    let value = admit(&mut fixture.cas, &draft)?;
    let map = fixture.identity_map(&fixture.fractions.clone())?;
    let unguarded = fixture.identity(&value, &value, &map);
    let before = fixture.cas.counts();
    assert!(
        fixture
            .cas
            .check_proof(
                &fixture.context,
                std::slice::from_ref(&unguarded),
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), before);
    let parameter = fixture.free("x")?;
    let assumption = fixture.assumption(&parameter, PredicateKind::NonZero)?;
    let mut guarded = fixture.identity(&value, &value, &map);
    guarded.premises = vec![0];
    let checked = fixture.cas.check_proof(
        &fixture.context,
        &[assumption, guarded.clone()],
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(checked.hypotheses().len(), 1);
    assert_eq!(checked.judgment().residuals, []);
    let other = fixture.free("y")?;
    let unrelated = fixture.assumption(&other, PredicateKind::NonZero)?;
    guarded.context.clone_from(&fixture.context);
    let before = fixture.cas.counts();
    assert!(
        fixture
            .cas
            .check_proof(
                &fixture.context,
                &[unrelated, guarded],
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// Constrained children preserve one shared predicate; changing only one child cannot claim a complete cover.
/// ´test:integration:tower-constrained-selection-transport´
#[test]
fn tower_constrained_selection_transport() -> TestResult {
    use crate::testing::solve_coefficients::SplitFixture;
    use nrz_cas::proof::Certificate;
    use nrz_cas::solving::{SolveCertificate, SolveView};
    let mut fixture = SplitFixture::new()?;
    let mut selected = Vec::new();
    let original = fixture.selections()?.to_vec();
    for handle in &original {
        let SolveView::RootSelection(selection) = fixture.unit.base.cas.solve_view(handle)? else {
            return Err("wrong selection".into());
        };
        let relation = selection
            .relations
            .first()
            .ok_or("missing relation")?
            .clone();
        selected.push(fixture.unit.base.positive_selection(&relation, true)?);
    }
    let step = fixture.steps.last_mut().ok_or("missing cover")?;
    let Certificate::Solve(certificate) = &mut step.certificate else {
        return Err("wrong certificate".into());
    };
    let SolveCertificate::ComponentSplit(split) = certificate.as_mut() else {
        return Err("wrong split".into());
    };
    split.selections.clone_from(&selected);
    let checked = fixture.unit.base.cas.check_proof(
        &fixture.unit.base.context,
        &fixture.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(checked.hypotheses(), []);
    let last = fixture.steps.last_mut().ok_or("missing cover")?;
    let Certificate::Solve(certificate) = &mut last.certificate else {
        return Err("wrong certificate".into());
    };
    let SolveCertificate::ComponentSplit(split) = certificate.as_mut() else {
        return Err("wrong split".into());
    };
    *split
        .selections
        .get_mut(1)
        .ok_or("missing right selection")? =
        original.get(1).ok_or("missing unrestricted child")?.clone();
    let before = fixture.unit.base.cas.counts();
    assert!(
        fixture
            .unit
            .base
            .cas
            .check_proof(
                &fixture.unit.base.context,
                &fixture.steps,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    assert_eq!(fixture.unit.base.cas.counts(), before);
    Ok(())
}

/// An optional splitting chain reconstructs each factor over its actual parent quotient.
/// ´test:integration:root-adjunction-splitting-chain´
#[test]
fn root_adjunction_splitting_chain() -> TestResult {
    use crate::testing::solve_coefficients::AdjunctionFixture;
    for (sign, succeeds) in [(1, true), (-1, false)] {
        let mut fixture = AdjunctionFixture::splitting_chain(sign)?;
        let before = fixture.base.cas.counts();
        let result = fixture.base.cas.check_proof(
            &fixture.base.context,
            &fixture.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        );
        if succeeds {
            let checked = result?;
            assert_eq!(checked.hypotheses(), []);
        } else {
            assert!(result.is_err());
            assert_eq!(fixture.base.cas.counts(), before);
        }
    }
    Ok(())
}

/// A symbolic radicand keeps its exact sign hypothesis; omitting it cannot certify existence.
/// ´test:integration:real-root-symbolic-sign-premise´
#[test]
fn real_root_symbolic_sign_premise() -> TestResult {
    use crate::testing::solve_coefficients::AdjunctionFixture;
    let mut fixture = AdjunctionFixture::parameter_square_root()?;
    let checked = fixture.base.cas.check_proof(
        &fixture.base.context,
        &fixture.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(checked.hypotheses().len(), 1);
    assert_eq!(checked.judgment().residuals, []);
    fixture
        .steps
        .last_mut()
        .ok_or("missing adjunction")?
        .premises = vec![1];
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
    Ok(())
}

/// Exact rational identities use independent expected fractions and reject changed numerators.
/// ´test:integration:coefficient-rational-identity-oracle´
#[test]
fn coefficient_rational_identity_oracle() -> TestResult {
    use crate::testing::solve_graph::{admit, expression};
    use nrz_cas::Reference;
    use nrz_cas::syntax::{AtomicSort, Field, Operator, Sort};
    for (expected, succeeds) in [("29/30", true), ("31/30", false)] {
        let mut fixture = Fixture::new()?;
        let left = fixture.rational_value("3/10")?;
        let right = fixture.rational_value("2/3")?;
        let independent = fixture.rational_value(expected)?;
        let rational = fixture.rational.clone();
        let map = fixture.scalar_map(&rational, &Field::Real)?;
        let first = fixture.literal(&left, &map)?;
        let second = fixture.literal(&right, &map)?;
        let expected = fixture.literal(&independent, &map)?;
        let total = admit(
            &mut fixture.cas,
            &expression(
                Reference::Existing(fixture.header.clone()),
                Operator::Add,
                Sort::Value(AtomicSort::Scalar(Field::Real)),
                vec![Reference::Existing(first), Reference::Existing(second)],
            ),
        )?;
        let step = fixture.identity(&total, &expected, &map);
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

/// Independent fraction products need their scoped nonzero premise and reject a changed second factor.
/// ´test:integration:coefficient-fraction-identity-oracle´
#[test]
fn coefficient_fraction_identity_oracle() -> TestResult {
    use crate::testing::solve_coefficients::fraction_identity;
    for succeeds in [true, false] {
        let (mut fixture, steps) = fraction_identity(succeeds)?;
        let before = fixture.cas.counts();
        let result = fixture.cas.check_proof(
            &fixture.context,
            &steps,
            &mut Budget::new(Limits::blind_cubic())?,
        );
        if succeeds {
            assert_eq!(result?.hypotheses().len(), 1);
        } else {
            assert!(result.is_err());
            assert_eq!(fixture.cas.counts(), before);
        }
    }
    Ok(())
}

/// Real-root orders survive decoding, while missing, zero and wrong orders refuse atomically.
/// ´test:integration:real-root-order-mutations´
#[test]
fn real_root_order_mutations() -> TestResult {
    use crate::testing::solve_coefficients::AdjunctionFixture;
    use nrz_cas::proof::Certificate;
    use nrz_cas::solving::{AdjunctionMode, SolveCertificate};
    use nrz_cas::syntax::Field;
    for (mode, degree) in [
        (AdjunctionMode::RealOddRoot, 3),
        (AdjunctionMode::PositiveRealSquareRoot, 2),
    ] {
        let polynomial = [(vec![degree], 1), (vec![0], -2)];
        let mut fixture = AdjunctionFixture::new(&polynomial, &Field::Real, mode, Some(degree))?;
        let checked = fixture.base.cas.check_proof(
            &fixture.base.context,
            &fixture.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        assert_eq!(checked.hypotheses(), []);
        assert_eq!(checked.judgment().residuals, []);
        for order in [None, Some(0), Some(degree + 1)] {
            let mut steps = fixture.steps.clone();
            let step = steps.last_mut().ok_or("missing real-root step")?;
            let Certificate::Solve(certificate) = &mut step.certificate else {
                return Err("wrong certificate".into());
            };
            let SolveCertificate::RootAdjunction(adjunction) = certificate.as_mut() else {
                return Err("wrong root certificate".into());
            };
            adjunction.order = order;
            let before = fixture.base.cas.counts();
            let result = fixture.base.cas.check_proof(
                &fixture.base.context,
                &steps,
                &mut Budget::new(Limits::blind_cubic())?,
            );
            assert!(result.is_err());
            assert_eq!(fixture.base.cas.counts(), before);
        }
    }
    Ok(())
}

/// Exact regional zero and reciprocal identities succeed only in their respective fresh split cells.
/// ´test:integration:coefficient-guard-proof-cells´
#[test]
fn coefficient_guard_proof_cells() -> TestResult {
    use crate::testing::solve_coefficients::{regional_fixture, regional_identity};
    for cell in 0..2 {
        let mut fixture = regional_fixture(cell)?;
        for zero in [true, false] {
            let step = regional_identity(&mut fixture, zero, true)?;
            let before = fixture.cas.counts();
            let result = fixture.cas.check_proof(
                &fixture.context,
                std::slice::from_ref(&step),
                &mut Budget::new(Limits::blind_cubic())?,
            );
            if zero == (cell == 0) {
                let witness = result?;
                assert_eq!(witness.hypotheses(), []);
                assert_eq!(witness.judgment().residuals, []);
            } else {
                assert!(result.is_err());
                assert_eq!(fixture.cas.counts(), before);
            }
        }
        let changed = regional_identity(&mut fixture, false, false)?;
        assert!(
            fixture
                .cas
                .check_proof(
                    &fixture.context,
                    std::slice::from_ref(&changed),
                    &mut Budget::new(Limits::blind_cubic())?
                )
                .is_err()
        );
    }
    Ok(())
}

/// A denominator change fails even for a formally reflexive identity, and map scope cannot be borrowed.
/// ´test:integration:coefficient-guard-denominator-scope´
#[test]
fn coefficient_guard_denominator_scope() -> TestResult {
    use crate::testing::solve_coefficients::regional_fixture;
    use crate::testing::solve_graph::admit;
    let mut fixture = regional_fixture(1)?;
    let one = fixture.polynomial(2, &[(vec![0, 0], 1)])?;
    let other = fixture.polynomial(2, &[(vec![0, 1], 1)])?;
    let value = fixture.fraction(&one, &other);
    let value = admit(&mut fixture.cas, &value)?;
    let domain = fixture.fractions.clone();
    let map = fixture.identity_map(&domain)?;
    let step = fixture.identity(&value, &value, &map);
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
    let mut wrong =
        crate::testing::solve_coefficients::regional_identity(&mut fixture, false, true)?;
    let x = fixture.free("x")?;
    let cells = fixture.guard_cells(&x)?;
    fixture.on_region(&cells[0])?;
    wrong.context.clone_from(&fixture.context);
    assert!(
        fixture
            .cas
            .check_proof(
                &fixture.context,
                std::slice::from_ref(&wrong),
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    Ok(())
}

/// Canonical polynomial literals retain the ancestor map scope and compare normalized coefficients.
/// ´test:integration:coefficient-literal-regional-guard´
#[test]
fn coefficient_literal_regional_guard() -> TestResult {
    use crate::testing::solve_graph::admit;
    let mut fixture = Fixture::new()?;
    let top = fixture.polynomial(2, &[(vec![1, 0], 1), (vec![0, 1], 1)])?;
    let one = fixture.polynomial(2, &[(vec![0, 0], 1)])?;
    let draft = fixture.fraction(&top, &one);
    let value = admit(&mut fixture.cas, &draft)?;
    let map = fixture.parameter_map()?;
    let literal = fixture.literal(&value, &map)?;
    let cells = fixture.guard_cells(&literal)?;
    let reciprocal = fixture.fraction(&one, &top);
    let reciprocal = admit(&mut fixture.cas, &reciprocal)?;
    for (cell, succeeds) in [(&cells[0], false), (&cells[1], true)] {
        fixture.on_region(cell)?;
        let domain = fixture.fractions.clone();
        let map = fixture.identity_map(&domain)?;
        let step = fixture.identity(&reciprocal, &reciprocal, &map);
        let result = fixture.cas.check_proof(
            &fixture.context,
            std::slice::from_ref(&step),
            &mut Budget::new(Limits::blind_cubic())?,
        );
        assert_eq!(result.is_ok(), succeeds);
    }
    Ok(())
}

/// Finite localization remains mandatory even when the cell proves the coefficient itself is zero.
/// ´test:integration:coefficient-regional-localization-retained´
#[test]
fn coefficient_regional_localization_retained() -> TestResult {
    use crate::testing::solve_coefficients::regional_fixture;
    use crate::testing::solve_graph::admit;
    use nrz_cas::Reference;
    use nrz_cas::solving::{CoefficientMapData, MapMode, MapTarget, SolveDraft};
    use nrz_cas::syntax::Draft;
    let mut fixture = regional_fixture(0)?;
    let top = fixture.polynomial(2, &[(vec![1, 0], 1)])?;
    let other = fixture.polynomial(2, &[(vec![0, 1], 1)])?;
    let one = fixture.polynomial(2, &[(vec![0, 0], 1)])?;
    let draft = fixture.fraction(&top, &one);
    let value = admit(&mut fixture.cas, &draft)?;
    let map = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
            source: Reference::Existing(fixture.fractions.clone()),
            target: MapTarget::CoefficientDomain {
                domain: Reference::Existing(fixture.fractions.clone()),
            },
            mode: MapMode::Embedding,
            region: Reference::Existing(fixture.region.clone()),
            images: Vec::new(),
            denominators: vec![Reference::Existing(other.clone())],
        })),
    )?;
    let mut step = fixture.identity(&value, &value, &map);
    let nrz_cas::proof::Certificate::Solve(certificate) = &mut step.certificate else {
        return Err("missing certificate".into());
    };
    let nrz_cas::solving::SolveCertificate::CoefficientIdentity(identity) = certificate.as_mut()
    else {
        return Err("wrong certificate".into());
    };
    identity.denominators.push(other);
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
    Ok(())
}

/// Independent denominator replay accepts powers and products only after exact factor exhaustion.
/// ´test:integration:coefficient-nonzero-product-proofs´
#[test]
fn coefficient_nonzero_product_proofs() -> TestResult {
    for case in 0..10 {
        let (mut fixture, value, succeeds) =
            crate::testing::solve_coefficients::nonzero_closure_fixture(case)?;
        let domain = fixture.fractions.clone();
        let map = fixture.identity_map(&domain)?;
        let step = fixture.identity(&value, &value, &map);
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

/// Every reciprocal power refuses on the exact zero child, even in a reflexive equality.
/// ´test:integration:coefficient-reciprocal-power-zero-cell´
#[test]
fn coefficient_reciprocal_power_zero_cell() -> TestResult {
    for exponent in 1..=3 {
        let mut fixture = crate::testing::solve_coefficients::regional_fixture(0)?;
        let value = fixture.reciprocal(&[(vec![exponent, 0], 1)])?;
        let domain = fixture.fractions.clone();
        let map = fixture.identity_map(&domain)?;
        let step = fixture.identity(&value, &value, &map);
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

/// A proved product is a whole divisor, and alternative overlapping guards cannot hide a valid route.
/// ´test:integration:coefficient-overlapping-nonzero-factors´
#[test]
fn coefficient_overlapping_nonzero_factors() -> TestResult {
    use nrz_cas::syntax::Operator;
    for reverse in [false, true] {
        let mut fixture = Fixture::new()?;
        let x = fixture.free("x")?;
        let y = fixture.free("y")?;
        let product = fixture.guard_expression(Operator::Mul, &[x.clone(), y.clone()])?;
        fixture.on_nonzero(&product)?;
        let unknown = fixture.reciprocal(&[(vec![1, 0], 1)])?;
        let domain = fixture.fractions.clone();
        let map = fixture.identity_map(&domain)?;
        let step = fixture.identity(&unknown, &unknown, &map);
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
        let square = fixture.guard_expression(Operator::IntegerPower(2), &[x])?;
        let cube = fixture.guard_expression(Operator::IntegerPower(3), &[y])?;
        let factors = if reverse {
            [cube, square]
        } else {
            [square, cube]
        };
        for factor in &factors {
            fixture.on_nonzero(factor)?;
        }
        let value = fixture.reciprocal(&[(vec![2, 3], 1)])?;
        let map = fixture.identity_map(&domain)?;
        let step = fixture.identity(&value, &value, &map);
        assert_eq!(
            fixture
                .cas
                .check_proof(
                    &fixture.context,
                    std::slice::from_ref(&step),
                    &mut Budget::new(Limits::blind_cubic())?
                )?
                .hypotheses(),
            []
        );
    }
    Ok(())
}

/// Existing coefficient limits meter the new closure and preserve atomic failure at one below usage.
/// ´test:integration:coefficient-nonzero-closure-limits´
#[test]
fn coefficient_nonzero_closure_limits() -> TestResult {
    let (mut fixture, value, _) = crate::testing::solve_coefficients::nonzero_closure_fixture(5)?;
    let domain = fixture.fractions.clone();
    let map = fixture.identity_map(&domain)?;
    let step = fixture.identity(&value, &value, &map);
    let mut measured = Budget::new(Limits::blind_cubic())?;
    fixture
        .cas
        .check_proof(&fixture.context, std::slice::from_ref(&step), &mut measured)?;
    let count = measured.usage().gcd_steps;
    assert!(count > 0);
    for (limit, succeeds) in [(count, true), (count - 1, false)] {
        let (mut peer, value, _) = crate::testing::solve_coefficients::nonzero_closure_fixture(5)?;
        let domain = peer.fractions.clone();
        let map = peer.identity_map(&domain)?;
        let step = peer.identity(&value, &value, &map);
        let before = peer.cas.counts();
        let mut budget = Budget::new(Limits {
            gcd_steps: limit,
            ..Limits::blind_cubic()
        })?;
        let result = peer
            .cas
            .check_proof(&peer.context, std::slice::from_ref(&step), &mut budget);
        if succeeds {
            assert!(result.is_ok());
        } else {
            let error = result.err().ok_or("short closure budget accepted")?;
            assert!(matches!(
                error.kind,
                nrz_cas::FaultKind::Exhausted {
                    budget: nrz_cas::limits::BudgetKind::GcdSteps,
                    ..
                }
            ));
            assert_eq!(peer.cas.counts(), before);
        }
    }
    Ok(())
}

/// A finite localization power is discharged exactly, while adding an unproved factor still refuses.
/// ´test:integration:coefficient-power-localization-proof´
#[test]
fn coefficient_power_localization_proof() -> TestResult {
    use nrz_cas::Reference;
    use nrz_cas::proof::Certificate;
    use nrz_cas::solving::{CoefficientMapData, MapMode, MapTarget, SolveCertificate, SolveDraft};
    use nrz_cas::syntax::Draft;
    for (powers, succeeds) in [([3, 0], true), ([3, 1], false)] {
        let (mut fixture, value, _) =
            crate::testing::solve_coefficients::nonzero_closure_fixture(2)?;
        let denominator = fixture.polynomial(2, &[(powers.to_vec(), 1)])?;
        let map = crate::testing::solve_graph::admit(
            &mut fixture.cas,
            &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
                source: Reference::Existing(fixture.fractions.clone()),
                target: MapTarget::CoefficientDomain {
                    domain: Reference::Existing(fixture.fractions.clone()),
                },
                mode: MapMode::Embedding,
                region: Reference::Existing(fixture.region.clone()),
                images: Vec::new(),
                denominators: vec![Reference::Existing(denominator.clone())],
            })),
        )?;
        let mut step = fixture.identity(&value, &value, &map);
        let Certificate::Solve(certificate) = &mut step.certificate else {
            return Err("missing coefficient certificate".into());
        };
        let SolveCertificate::CoefficientIdentity(identity) = certificate.as_mut() else {
            return Err("wrong coefficient certificate".into());
        };
        identity.denominators.push(denominator);
        let before = fixture.cas.counts();
        let result = fixture.cas.check_proof(
            &fixture.context,
            std::slice::from_ref(&step),
            &mut Budget::new(Limits::blind_cubic())?,
        );
        assert_eq!(result.is_ok(), succeeds);
        if !succeeds {
            assert_eq!(fixture.cas.counts(), before);
        }
    }
    Ok(())
}
/// An earlier coefficient equality remains the sole premise of the unchanged relation route.
/// ´test:integration:coefficient-existing-relation-premise´
#[test]
fn coefficient_existing_relation_premise() -> TestResult {
    use crate::testing::solve_coefficients::existing_relation_identity;
    let (mut fixture, steps) = existing_relation_identity()?;
    let checked = fixture.cas.check_proof(
        &fixture.context,
        &steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(checked.hypotheses(), []);
    assert_eq!(checked.judgment().residuals, []);
    let mut missing = steps.last().ok_or("missing relation application")?.clone();
    missing.premises.clear();
    let before = fixture.cas.counts();
    assert!(
        fixture
            .cas
            .check_proof(
                &fixture.context,
                std::slice::from_ref(&missing),
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// A complete cubic Vieta frame closes without residuals and allocates no computational tower.
/// ´test:integration:coefficient-scoped-vieta-discharge´
#[test]
fn coefficient_scoped_vieta_discharge() -> TestResult {
    use crate::testing::solve_coefficients::{ScopedMutation, scoped_identity};
    for mutation in [ScopedMutation::Valid, ScopedMutation::SymmetricPermutation] {
        let (mut fixture, steps) = scoped_identity(mutation)?;
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let checked = fixture
            .cas
            .check_proof(&fixture.context, &steps, &mut budget)?;
        assert_eq!(checked.hypotheses(), []);
        assert_eq!(checked.judgment().residuals, []);
        assert_eq!(budget.usage().tower_levels, 0);
        assert_eq!(
            checked.judgment().statement,
            steps.last().ok_or("missing closure")?.conclusion
        );
    }
    Ok(())
}

/// Every incomplete, unrelated, undefined or still-root-dependent system refuses atomic publication.
/// ´test:integration:coefficient-scoped-vieta-mutations´
#[test]
fn coefficient_scoped_vieta_mutations() -> TestResult {
    use crate::testing::solve_coefficients::{ScopedMutation, scoped_identity};
    for mutation in [
        ScopedMutation::ChangedEndpoints,
        ScopedMutation::UndefinedEndpoint,
        ScopedMutation::SlotAtomSwap,
        ScopedMutation::ForeignContext,
        ScopedMutation::ForeignRegion,
        ScopedMutation::OmittedSlot,
        ScopedMutation::MissingHypothesis,
        ScopedMutation::MissingRelation,
        ScopedMutation::ExtraRelation,
        ScopedMutation::Polynomial,
        ScopedMutation::Degree,
        ScopedMutation::UnprovedDegree,
        ScopedMutation::Leading,
        ScopedMutation::RootConclusion,
        ScopedMutation::RealBinder,
        ScopedMutation::ScalarRootConclusion,
        ScopedMutation::RootRegion,
        ScopedMutation::ParentRootAssumption,
    ] {
        let (mut fixture, steps) = scoped_identity(mutation)?;
        let before = fixture.cas.counts();
        assert!(
            fixture
                .cas
                .check_proof(
                    &fixture.context,
                    &steps,
                    &mut Budget::new(Limits::blind_cubic())?
                )
                .is_err(),
            "{mutation:?}"
        );
        assert_eq!(fixture.cas.counts(), before);
    }
    Ok(())
}

/// Ordinary scoped equality use retains each undischarged hypothesis as its exact residual at closure.
/// ´test:integration:coefficient-scoped-hypotheses-retained´
#[test]
fn coefficient_scoped_hypotheses_retained() -> TestResult {
    use crate::testing::solve_coefficients::{ScopedMutation, scoped_identity};
    use nrz_cas::proof::Statement;
    let (mut fixture, steps) = scoped_identity(ScopedMutation::Retained)?;
    let checked = fixture.cas.check_proof(
        &fixture.context,
        &steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(checked.hypotheses(), []);
    assert_eq!(checked.judgment().residuals.len(), 3);
    for step in steps.iter().skip(2).take(3) {
        let Statement::PredicateFact { predicate, region } = &step.conclusion else {
            return Err("missing equality fact".into());
        };
        assert!(
            checked
                .judgment()
                .residuals
                .iter()
                .any(|residual| residual.predicate == *predicate && residual.region == *region)
        );
    }
    Ok(())
}

/// An asymmetric relation checks the declared slot order independently and retains its hypothesis.
/// ´test:integration:coefficient-scoped-asymmetric-order´
#[test]
fn coefficient_scoped_asymmetric_order() -> TestResult {
    use crate::testing::solve_coefficients::asymmetric_relation;
    let (mut fixture, steps) = asymmetric_relation(false)?;
    let checked = fixture.cas.check_proof(
        &fixture.context,
        &steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(checked.judgment().residuals.len(), 1);
    let (mut fixture, steps) = asymmetric_relation(true)?;
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

/// Exact predicate and restriction scopes are checked before publishing a bound context.
/// ´test:integration:coefficient-context-scope-mutations´
#[test]
fn coefficient_context_scope_mutations() -> TestResult {
    use crate::testing::solve_coefficients::scoped_context_mismatch;
    for (local, restriction) in [(false, false), (true, false), (true, true)] {
        let (mut fixture, draft) = scoped_context_mismatch(local, restriction)?;
        let before = fixture.cas.counts();
        assert!(
            fixture
                .cas
                .admit(
                    std::slice::from_ref(&draft),
                    &mut Budget::new(Limits::blind_cubic())?
                )
                .is_err()
        );
        assert_eq!(fixture.cas.counts(), before);
    }
    Ok(())
}

/// Existing scalar proof and request consumers refuse scoped contexts with their exact typed reason.
/// ´test:integration:coefficient-scoped-consumer-refusals´
#[test]
fn coefficient_scoped_consumer_refusals() -> TestResult {
    use crate::testing::solve_coefficients::{scoped_request, scoped_scalar};
    use nrz_cas::InvalidCode;
    use nrz_cas::request::Outcome;
    let (mut fixture, step) = scoped_scalar(true)?;
    let before = fixture.cas.counts();
    let fault = fixture
        .cas
        .check_proof(
            &fixture.context,
            std::slice::from_ref(&step),
            &mut Budget::new(Limits::blind_cubic())?,
        )
        .err()
        .ok_or("scalar proof accepted scoped context")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedFragment)
    );
    assert_eq!(fixture.cas.counts(), before);
    let (mut fixture, request) = scoped_request(true)?;
    let before = fixture.cas.counts();
    let Outcome::Invalid { fault, .. } = fixture.cas.run(&request) else {
        return Err("scalar producer accepted scoped context".into());
    };
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedFragment)
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// Empty-scope scalar consumers preserve their existing proof and request acceptance.
/// ´test:integration:coefficient-closed-consumer-regression´
#[test]
fn coefficient_closed_consumer_regression() -> TestResult {
    use crate::testing::solve_coefficients::{scoped_request, scoped_scalar};
    use nrz_cas::request::Outcome;
    let (mut fixture, step) = scoped_scalar(false)?;
    let witness = fixture.cas.check_proof(
        &fixture.context,
        std::slice::from_ref(&step),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(witness.hypotheses(), []);
    let (mut fixture, request) = scoped_request(false)?;
    assert!(matches!(fixture.cas.run(&request), Outcome::Proved { .. }));
    Ok(())
}

/// Root-existence support cannot carry a mathematical dependency on the formal-root binder.
/// ´test:integration:coefficient-scoped-adjunction-refusal´
#[test]
fn coefficient_scoped_adjunction_refusal() -> TestResult {
    use crate::testing::solve_coefficients::scoped_dependent_adjunction;
    use nrz_cas::InvalidCode;
    let (mut fixture, steps) = scoped_dependent_adjunction()?;
    let before = fixture.cas.counts();
    let fault = fixture
        .cas
        .check_proof(
            &fixture.context,
            &steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )
        .err()
        .ok_or("root-dependent supporting theorem accepted")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedFragment)
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// Both proof and producer boundaries inspect a premise's context as well as the receiving context.
/// ´test:integration:coefficient-scoped-premise-refusal´
#[test]
fn coefficient_scoped_premise_refusal() -> TestResult {
    use crate::testing::solve_coefficients::scoped_premise_fixture;
    use nrz_cas::InvalidCode;
    use nrz_cas::proof::{ApplicationDraft, Certificate, ConstructorLaw, Primitive, Statement};
    use nrz_cas::request::Outcome;
    let (mut fixture, mut steps, mut request) = scoped_premise_fixture()?;
    let witness = fixture.cas.check_proof(
        &fixture.context,
        &steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    request.premises.push(witness);
    let before = fixture.cas.counts();
    let Outcome::Invalid { fault, .. } = fixture.cas.run(&request) else {
        return Err("producer consumed bound-context premise".into());
    };
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedFragment)
    );
    assert_eq!(fixture.cas.counts(), before);
    let previous = u32::try_from(steps.len().checked_sub(1).ok_or("missing scoped proof")?)?;
    steps.push(ApplicationDraft {
        rule: Primitive::RegionConstruct,
        context: request.context.clone(),
        conclusion: Statement::Constructor {
            region: fixture.cas.admissible_region(&request.context)?,
            law: ConstructorLaw::Membership,
        },
        premises: vec![previous],
        substitutions: Vec::new(),
        certificate: Certificate::None,
    });
    let fault = fixture
        .cas
        .check_proof(
            &request.context,
            &steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )
        .err()
        .ok_or("native consumer accepted bound-context premise")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedFragment)
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}
