// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical coefficient closures survive the ordinary typed witness lifecycle.

use crate::testing::common::TestResult;
use nrz_cas::Cas;
use nrz_cas::limits::{Budget, Limits};
use nrz_cas::proof::VerifiedWitness;

/// A checked fraction identity keeps its explicit scoped nonzero hypothesis in the reachable proof.
fn fraction_witness() -> TestResult<(Cas, VerifiedWitness)> {
    let (mut fixture, steps) = crate::testing::solve_coefficients::fraction_identity(true)?;
    let witness = fixture.cas.check_proof(
        &fixture.context,
        &steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    Ok((fixture.cas, witness))
}

/// Fresh replay revalidates multivariate canonical fractions and retains the explicit used hypothesis.
/// ´test:integration:fraction-closure-replay´
#[test]
fn fraction_closure_replay() -> TestResult {
    let (source, witness) = fraction_witness()?;
    let bytes = source.export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
    let (mut target, expected) = fraction_witness()?;
    let replayed = target.replay(
        &bytes,
        expected.judgment(),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(replayed.judgment(), expected.judgment());
    assert_eq!(replayed.hypotheses().len(), 1);
    assert_eq!(
        target.export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}

/// Exported full component cover replays both children, their exact root restrictions and the derivative unit.
/// ´test:integration:tower-components-fresh-replay´
#[test]
fn tower_components_fresh_replay() -> TestResult {
    use crate::testing::solve_coefficients::SplitFixture;
    let mut source = SplitFixture::new()?;
    let witness = source.unit.base.cas.check_proof(
        &source.unit.base.context,
        &source.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let bytes = source
        .unit
        .base
        .cas
        .export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
    let mut target = SplitFixture::new()?;
    let expected = target.unit.base.cas.check_proof(
        &target.unit.base.context,
        &target.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let replayed = target.unit.base.cas.replay(
        &bytes,
        expected.judgment(),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(replayed.judgment(), expected.judgment());
    assert_eq!(replayed.hypotheses(), []);
    assert_eq!(replayed.judgment().residuals, []);
    assert_eq!(
        target
            .unit
            .base
            .cas
            .export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}

/// The Gaussian identity has a discharged, byte-stable fresh-store replay lifecycle.
/// ´test:integration:coefficient-gaussian-fresh-replay´
#[test]
fn coefficient_gaussian_fresh_replay() -> TestResult {
    use crate::testing::solve_coefficients::gaussian_identity;
    let (mut source, step) = gaussian_identity(11)?;
    let witness = source.cas.check_proof(
        &source.context,
        std::slice::from_ref(&step),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let bytes = source
        .cas
        .export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
    let (mut target, expected_step) = gaussian_identity(11)?;
    let expected = target.cas.check_proof(
        &target.context,
        std::slice::from_ref(&expected_step),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let checked = target.cas.replay(
        &bytes,
        expected.judgment(),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(checked.judgment(), expected.judgment());
    assert_eq!(checked.hypotheses(), []);
    assert_eq!(
        target
            .cas
            .export_witness(&checked, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}

/// Linked algebraic values, exact units and a reducible-level cover replay together in both components.
/// ´test:integration:linked-adjunction-component-lifecycle´
#[test]
fn linked_adjunction_component_lifecycle() -> TestResult {
    use crate::testing::solve_coefficients::SplitFixture;
    let mut source = SplitFixture::linked()?;
    linked_component_domains(&source)?;
    let witness = source.unit.base.cas.check_proof(
        &source.unit.base.context,
        &source.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let bytes = source
        .unit
        .base
        .cas
        .export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
    let mut target = SplitFixture::linked()?;
    let expected = target.unit.base.cas.check_proof(
        &target.unit.base.context,
        &target.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let checked = target.unit.base.cas.replay(
        &bytes,
        expected.judgment(),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(checked.judgment(), expected.judgment());
    assert_eq!(checked.hypotheses(), []);
    assert_eq!(checked.judgment().residuals, []);
    assert_eq!(
        target
            .unit
            .base
            .cas
            .export_witness(&checked, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    assert_eq!(nrz_cas::solving::active_schema_bitmap() & 15, 15);
    Ok(())
}

/// Each projected v-u relation really has the corresponding child quotient as its coefficient domain.
fn linked_component_domains(
    fixture: &crate::testing::solve_coefficients::SplitFixture,
) -> TestResult {
    use nrz_cas::proof::Statement;
    use nrz_cas::solving::{SolveStatement, SolveView};
    let mut domains = Vec::new();
    let children = fixture.children()?;
    for step in &fixture.steps {
        let Statement::Solve(statement) = &step.conclusion else {
            continue;
        };
        let SolveStatement::RootExistence(existence) = statement.as_ref() else {
            continue;
        };
        let SolveView::RootSelection(selection) =
            fixture.unit.base.cas.solve_view(&existence.relation)?
        else {
            return Err("missing exact root selection".into());
        };
        let relation = selection
            .relations
            .first()
            .ok_or("missing linked relation")?;
        let SolveView::RootRelation(relation) = fixture.unit.base.cas.solve_view(relation)? else {
            return Err("wrong linked relation".into());
        };
        let SolveView::Polynomial(polynomial) =
            fixture.unit.base.cas.solve_view(&relation.polynomial)?
        else {
            return Err("wrong linked polynomial".into());
        };
        if children.contains(&polynomial.domain) {
            domains.push(polynomial.domain.clone());
        }
    }
    assert_eq!(domains.as_slice(), children.as_slice());
    Ok(())
}

/// Both exact cells replay without residual hypotheses and reject a different expected regional claim.
/// ´test:integration:coefficient-guards-fresh-replay´
#[test]
fn coefficient_guards_fresh_replay() -> TestResult {
    use crate::testing::solve_coefficients::{regional_fixture, regional_identity};
    for cell in 0..2 {
        let mut source = regional_fixture(cell)?;
        let step = regional_identity(&mut source, cell == 0, true)?;
        let witness = source.cas.check_proof(
            &source.context,
            std::slice::from_ref(&step),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let bytes = source
            .cas
            .export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
        let mut target = regional_fixture(cell)?;
        let step = regional_identity(&mut target, cell == 0, true)?;
        let expected = target.cas.check_proof(
            &target.context,
            std::slice::from_ref(&step),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let checked = target.cas.replay(
            &bytes,
            expected.judgment(),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        assert_eq!(checked.judgment(), expected.judgment());
        assert_eq!(checked.hypotheses(), []);
        assert_eq!(checked.judgment().residuals, []);
        assert_eq!(
            target
                .cas
                .export_witness(&checked, &mut Budget::new(Limits::blind_cubic())?)?,
            bytes
        );
        let mut wrong = expected.judgment().clone();
        let parameter = target.free("x")?;
        let children = target.guard_cells(&parameter)?;
        target.on_region(&children[0])?;
        wrong.context.clone_from(&target.context);
        let before = target.cas.counts();
        assert!(
            target
                .cas
                .replay(&bytes, &wrong, &mut Budget::new(Limits::blind_cubic())?)
                .is_err()
        );
        assert_eq!(target.cas.counts(), before);
    }
    Ok(())
}

/// Guard products and powers replay in a fresh graph with exact expected scope and canonical bytes.
/// ´test:integration:coefficient-nonzero-closure-fresh-replay´
#[test]
fn coefficient_nonzero_closure_fresh_replay() -> TestResult {
    use crate::testing::solve_coefficients::nonzero_closure_fixture;
    for case in [0, 1, 2, 4, 5, 7] {
        let (mut source, value, _) = nonzero_closure_fixture(case)?;
        let domain = source.fractions.clone();
        let map = source.identity_map(&domain)?;
        let step = source.identity(&value, &value, &map);
        let witness = source.cas.check_proof(
            &source.context,
            std::slice::from_ref(&step),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let bytes = source
            .cas
            .export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
        let (mut target, value, _) = nonzero_closure_fixture(case)?;
        let domain = target.fractions.clone();
        let map = target.identity_map(&domain)?;
        let step = target.identity(&value, &value, &map);
        let expected = target.cas.check_proof(
            &target.context,
            std::slice::from_ref(&step),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let replayed = target.cas.replay(
            &bytes,
            expected.judgment(),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        assert_eq!(replayed.judgment(), expected.judgment());
        assert_eq!(replayed.hypotheses(), []);
        assert_eq!(replayed.judgment().residuals, []);
        assert_eq!(
            target
                .cas
                .export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
            bytes
        );
    }
    Ok(())
}

/// Powers justified by a sealed fraction premise keep that hypothesis through independent fresh replay.
/// ´test:integration:coefficient-nonzero-premise-fresh-replay´
#[test]
fn coefficient_nonzero_premise_fresh_replay() -> TestResult {
    use crate::testing::solve_coefficients::nonzero_fraction_power_identity;
    let (mut source, steps) = nonzero_fraction_power_identity()?;
    let witness = source.cas.check_proof(
        &source.context,
        &steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let bytes = source
        .cas
        .export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
    let (mut target, steps) = nonzero_fraction_power_identity()?;
    let expected = target.cas.check_proof(
        &target.context,
        &steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let replayed = target.cas.replay(
        &bytes,
        expected.judgment(),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(replayed.judgment(), expected.judgment());
    assert_eq!(replayed.hypotheses().len(), 1);
    assert_eq!(replayed.judgment().residuals, []);
    assert_eq!(
        target
            .cas
            .export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    let mut unproved = steps.last().ok_or("missing identity")?.clone();
    unproved.premises.clear();
    let before = target.cas.counts();
    assert!(
        target
            .cas
            .check_proof(
                &target.context,
                std::slice::from_ref(&unproved),
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    assert_eq!(target.cas.counts(), before);
    Ok(())
}

/// Fresh-store replay reconstructs all scoped relation and existential-discharge evidence.
/// ´test:integration:coefficient-scoped-vieta-fresh-replay´
#[test]
fn coefficient_scoped_vieta_fresh_replay() -> TestResult {
    use crate::testing::solve_coefficients::{ScopedMutation, scoped_identity};
    use nrz_cas::domain::Layer;
    use nrz_cas::proof::Judgment;
    let (mut source, steps) = scoped_identity(ScopedMutation::Valid)?;
    let witness = source.cas.check_proof(
        &source.context,
        &steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let bytes = source
        .cas
        .export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
    let (mut target, target_steps) = scoped_identity(ScopedMutation::Valid)?;
    let expected = Judgment {
        context: target.context.clone(),
        layer: Layer::Logic,
        space: None,
        statement: target_steps
            .last()
            .ok_or("missing target closure")?
            .conclusion
            .clone(),
        residuals: Vec::new(),
    };
    let checked = target
        .cas
        .replay(&bytes, &expected, &mut Budget::new(Limits::blind_cubic())?)?;
    assert_eq!(checked.judgment(), &expected);
    assert_eq!(checked.hypotheses(), []);
    assert_eq!(
        target
            .cas
            .export_witness(&checked, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}

/// Fresh replay repeats the quotient modulus and source-to-target coefficient interpretation.
/// ´test:integration:coefficient-quotient-specialization-fresh-replay´
#[test]
fn coefficient_quotient_specialization_fresh_replay() -> TestResult {
    use crate::testing::solve_coefficients::{SpecializationMutation, specialization_identity};
    for polynomial in [false, true] {
        let (mut source, step) =
            specialization_identity(SpecializationMutation::Valid, polynomial)?;
        let witness = source.cas.check_proof(
            &source.context,
            std::slice::from_ref(&step),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let bytes = source
            .cas
            .export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
        let (mut target, step) =
            specialization_identity(SpecializationMutation::Valid, polynomial)?;
        let expected = nrz_cas::proof::Judgment {
            context: target.context.clone(),
            layer: nrz_cas::domain::Layer::Logic,
            space: None,
            statement: step.conclusion,
            residuals: Vec::new(),
        };
        let replayed =
            target
                .cas
                .replay(&bytes, &expected, &mut Budget::new(Limits::blind_cubic())?)?;
        assert_eq!(replayed.judgment(), &expected);
        assert_eq!(replayed.hypotheses(), []);
        assert_eq!(
            target
                .cas
                .export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
            bytes
        );
    }
    Ok(())
}
