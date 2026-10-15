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
