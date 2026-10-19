// SPDX-License-Identifier: AGPL-3.0-only
//! Original-domain elementary certificates replay in independently constructed stores.

use crate::testing::common::TestResult;
use crate::testing::solve_elementary::{Fixture, hole, translation};
use nrz_cas::limits::{Budget, Limits};
use nrz_cas::proof::ApplicationDraft;

/// Set up each independent certificate from its displayed input mathematics.
fn supplied(case: u8) -> TestResult<(Fixture, ApplicationDraft)> {
    match case {
        0 => {
            let (mut fixture, problem) = hole()?;
            let entry = fixture.integer(-1, false)?;
            let step = fixture.member(&problem, &entry)?;
            Ok((fixture, step))
        }
        1 => translation(-1),
        _ => {
            let mut fixture = Fixture::new()?;
            let coefficient = fixture.free("a")?;
            let step = fixture.split(&coefficient)?;
            Ok((fixture, step))
        }
    }
}

/// Membership, reversible translation and full degree partitions all survive fresh canonical replay.
/// ´test:integration:elementary-guarded-fresh-replay´
#[test]
fn elementary_guarded_fresh_replay() -> TestResult {
    for case in 0..3 {
        let (mut source, step) = supplied(case)?;
        let witness = source.cas.check_proof(
            &source.context,
            std::slice::from_ref(&step),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let bytes = source
            .cas
            .export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
        let (mut target, expected_step) = supplied(case)?;
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
        assert_eq!(checked.judgment().residuals, []);
        assert_eq!(checked.hypotheses(), []);
        assert_eq!(
            target
                .cas
                .export_witness(&checked, &mut Budget::new(Limits::blind_cubic())?)?,
            bytes
        );
    }
    Ok(())
}

/// Primitive order, linked roots and the complete product all replay together in a new store.
/// ´test:integration:binomial-fresh-replay´
#[test]
fn binomial_fresh_replay() -> TestResult {
    use crate::testing::solve_elementary::BinomialFixture;
    for translated in [false, true] {
        let mut source = if translated {
            BinomialFixture::translated()?
        } else {
            BinomialFixture::new(2, 2)?
        };
        let checked = source.base.cas.check_proof(
            &source.base.context,
            &source.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let bytes = source
            .base
            .cas
            .export_witness(&checked, &mut Budget::new(Limits::blind_cubic())?)?;
        let mut target = if translated {
            BinomialFixture::translated()?
        } else {
            BinomialFixture::new(2, 2)?
        };
        let expected = target.base.cas.check_proof(
            &target.base.context,
            &target.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let replayed = target.base.cas.replay(
            &bytes,
            expected.judgment(),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        assert_eq!(replayed.judgment(), expected.judgment());
        assert_eq!(replayed.hypotheses(), []);
        assert_eq!(replayed.judgment().residuals, []);
        assert_eq!(
            target
                .base
                .cas
                .export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
            bytes
        );
    }
    Ok(())
}

/// Constructively refined selections retain their exact target and uniqueness strength in fresh replay.
/// ´test:integration:root-refinement-fresh-replay´
#[test]
fn root_refinement_fresh_replay() -> TestResult {
    for unique in [false, true] {
        let (mut source, step) = crate::testing::solve_elementary::root_refinement(2, unique)?;
        let checked = source.cas.check_proof(
            &source.context,
            std::slice::from_ref(&step),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let bytes = source
            .cas
            .export_witness(&checked, &mut Budget::new(Limits::blind_cubic())?)?;
        let (mut target, expected_step) =
            crate::testing::solve_elementary::root_refinement(2, unique)?;
        let expected = target.cas.check_proof(
            &target.context,
            std::slice::from_ref(&expected_step),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let replayed = target.cas.replay(
            &bytes,
            expected.judgment(),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        assert_eq!(replayed.judgment(), expected.judgment());
        assert_eq!(
            target
                .cas
                .export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
            bytes
        );
    }
    Ok(())
}

/// Each supplied case is reconstructed without importing producer state or an old witness handle.
fn final_case(case: u8) -> TestResult<crate::testing::solve_elementary::BinomialFixture> {
    use crate::testing::solve_elementary::{
        BinomialFixture, character_rank, conjugate_real, identity_family,
    };
    match case {
        0 => {
            let (base, steps) = identity_family(0)?;
            Ok(BinomialFixture { base, steps })
        }
        1 => {
            let (base, step) = character_rank()?;
            Ok(BinomialFixture {
                base,
                steps: vec![step],
            })
        }
        2 => {
            let (base, step) = conjugate_real(true)?;
            Ok(BinomialFixture {
                base,
                steps: vec![step],
            })
        }
        3 => BinomialFixture::multiplicity(),
        4 => BinomialFixture::presentation(),
        7 => crate::testing::solve_elementary::degree_drop_cover(),
        9 => crate::testing::solve_elementary::specialized_presentation(1),
        10 => crate::testing::solve_elementary::family_presentation(true),
        11 => crate::testing::solve_elementary::general_presentation(false),
        12 => crate::testing::solve_elementary::independent_cube_roots(false),
        13 => crate::testing::solve_elementary::linear_region(false, true),
        14 => crate::testing::solve_elementary::linear_region(true, true),
        15 => crate::testing::solve_elementary::binomial_region(true, false),
        _ => {
            let (base, step) = crate::testing::solve_elementary::linear_system(true)?;
            Ok(BinomialFixture {
                base,
                steps: vec![step],
            })
        }
    }
}

/// Universal families, linear recovery metadata, multiplicity, realness and presentation replay byte for byte.
/// ´test:integration:elementary-final-schemas-fresh-replay´
#[test]
fn elementary_final_schemas_fresh_replay() -> TestResult {
    for case in (0..16).filter(|case| !matches!(case, 5 | 6)) {
        let mut source = final_case(case)?;
        let checked = source
            .base
            .cas
            .check_proof(
                &source.base.context,
                &source.steps,
                &mut Budget::new(Limits::blind_cubic())?,
            )
            .map_err(|error| format!("case {case}, source proof: {error}"))?;
        let bytes = source
            .base
            .cas
            .export_witness(&checked, &mut Budget::new(Limits::blind_cubic())?)?;
        let mut target = final_case(case)?;
        let expected = target
            .base
            .cas
            .check_proof(
                &target.base.context,
                &target.steps,
                &mut Budget::new(Limits::blind_cubic())?,
            )
            .map_err(|error| format!("case {case}, expected proof: {error}"))?;
        let replayed = target.base.cas.replay(
            &bytes,
            expected.judgment(),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        assert_eq!(replayed.judgment(), expected.judgment());
        assert_eq!(replayed.hypotheses(), []);
        assert_eq!(replayed.judgment().residuals, []);
        assert_eq!(
            target
                .base
                .cas
                .export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
            bytes
        );
    }
    Ok(())
}

/// Algebraic inverse identities and all their interpretation maps survive fresh-store replay.
/// ´test:integration:checked-inverse-pivots-fresh-replay´
#[test]
fn checked_inverse_pivots_fresh_replay() -> TestResult {
    for scale in [false, true] {
        let mut source = crate::testing::solve_elementary::inverse_pivot(scale)?;
        let checked = source.base.cas.check_proof(
            &source.base.context,
            &source.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let bytes = source
            .base
            .cas
            .export_witness(&checked, &mut Budget::new(Limits::blind_cubic())?)?;
        let mut target = crate::testing::solve_elementary::inverse_pivot(scale)?;
        let expected = target.base.cas.check_proof(
            &target.base.context,
            &target.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let replayed = target.base.cas.replay(
            &bytes,
            expected.judgment(),
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        assert_eq!(replayed.judgment(), expected.judgment());
        assert_eq!(
            target
                .base
                .cas
                .export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
            bytes
        );
    }
    Ok(())
}

/// Recursive composition survives fresh-store replay with all closed branch identities intact.
/// ´test:integration:closed-strata-presentation-fresh-replay´
#[test]
fn closed_strata_presentation_fresh_replay() -> TestResult {
    let mut source = crate::testing::solve_elementary::closed_presentations()?;
    let checked = source.base.cas.check_proof(
        &source.base.context,
        &source.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let bytes = source
        .base
        .cas
        .export_witness(&checked, &mut Budget::new(Limits::blind_cubic())?)?;
    let mut target = crate::testing::solve_elementary::closed_presentations()?;
    let expected = target.base.cas.check_proof(
        &target.base.context,
        &target.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let replayed = target.base.cas.replay(
        &bytes,
        expected.judgment(),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(replayed.judgment(), expected.judgment());
    assert_eq!(replayed.hypotheses(), []);
    assert_eq!(replayed.judgment().residuals, []);
    assert_eq!(
        target
            .base
            .cas
            .export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}

/// Returned tower vectors preserve real choices, norm linkage and exact conjugate identities in fresh replay.
/// ´test:integration:linked-cubic-realness-fresh-replay´
#[test]
fn linked_cubic_realness_fresh_replay() -> TestResult {
    for bombelli in [false, true] {
        let mut source = crate::testing::solve_elementary::linked_cubic(bombelli, u8::MAX)?;
        let mut generated = Budget::new(Limits::blind_cubic())?;
        let checked =
            source
                .base
                .cas
                .check_proof(&source.base.context, &source.steps, &mut generated)?;
        assert!(generated.usage().coefficient_ops < Limits::blind_cubic().coefficient_ops / 4);
        assert_eq!(checked.hypotheses(), []);
        assert_eq!(checked.judgment().residuals, []);
        let bytes = source
            .base
            .cas
            .export_witness(&checked, &mut Budget::new(Limits::blind_cubic())?)?;
        let mut target = crate::testing::solve_elementary::linked_cubic(bombelli, u8::MAX)?;
        let expected = target.base.cas.check_proof(
            &target.base.context,
            &target.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let mut replay = Budget::new(Limits::blind_cubic())?;
        let replayed = target
            .base
            .cas
            .replay(&bytes, expected.judgment(), &mut replay)?;
        assert_eq!(
            generated.usage().coefficient_ops,
            replay.usage().coefficient_ops
        );
        assert_eq!(generated.usage().gcd_steps, replay.usage().gcd_steps);
        assert_eq!(replayed.judgment(), expected.judgment());
        assert_eq!(
            target
                .base
                .cas
                .export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
            bytes
        );
    }
    Ok(())
}

/// Fraction realness requires the original nonzero specialization guard and survives independent replay.
/// ´test:integration:fraction-realness-fresh-replay´
#[test]
fn fraction_realness_fresh_replay() -> TestResult {
    let mut source = crate::testing::solve_elementary::fraction_realness(true)?;
    let checked = source.base.cas.check_proof(
        &source.base.context,
        &source.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let bytes = source
        .base
        .cas
        .export_witness(&checked, &mut Budget::new(Limits::blind_cubic())?)?;
    let mut target = crate::testing::solve_elementary::fraction_realness(true)?;
    let expected = target.base.cas.check_proof(
        &target.base.context,
        &target.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let replayed = target.base.cas.replay(
        &bytes,
        expected.judgment(),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(replayed.judgment(), expected.judgment());
    assert_eq!(
        target
            .base
            .cas
            .export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}

/// Explicit evidence replays with its hypothesis, and duplicate slots import each equality only once.
/// ´test:integration:conjugation-referenced-evidence-fresh-replay´
#[test]
fn conjugation_referenced_evidence_fresh_replay() -> TestResult {
    let mut previous = None;
    for references in [1, 2] {
        let mut source = crate::testing::solve_elementary::referenced_realness(references)?;
        let mut generated = Budget::new(Limits::blind_cubic())?;
        let checked =
            source
                .base
                .cas
                .check_proof(&source.base.context, &source.steps, &mut generated)?;
        assert_eq!(checked.hypotheses().len(), 1);
        let usage = (
            generated.usage().coefficient_ops,
            generated.usage().gcd_steps,
        );
        if let Some(expected) = previous {
            assert_eq!(usage, expected);
        }
        previous = Some(usage);
        let bytes = source
            .base
            .cas
            .export_witness(&checked, &mut Budget::new(Limits::blind_cubic())?)?;
        let mut target = crate::testing::solve_elementary::referenced_realness(references)?;
        let expected = target.base.cas.check_proof(
            &target.base.context,
            &target.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let mut replay = Budget::new(Limits::blind_cubic())?;
        let replayed = target
            .base
            .cas
            .replay(&bytes, expected.judgment(), &mut replay)?;
        assert_eq!(replayed.judgment(), expected.judgment());
        assert_eq!(
            usage,
            (replay.usage().coefficient_ops, replay.usage().gcd_steps)
        );
        assert_eq!(
            target
                .base
                .cas
                .export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
            bytes
        );
    }
    Ok(())
}

/// A requested cubic product reconstruction is imported once across all roots and both components.
/// ´test:integration:conjugation-product-evidence-imported-once´
#[test]
fn conjugation_product_evidence_imported_once() -> TestResult {
    let mut previous = None;
    for duplicate in [false, true] {
        let mut fixture = crate::testing::solve_elementary::cubic_product_reference(duplicate)?;
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let checked =
            fixture
                .base
                .cas
                .check_proof(&fixture.base.context, &fixture.steps, &mut budget)?;
        assert_eq!(checked.hypotheses(), []);
        let usage = (budget.usage().coefficient_ops, budget.usage().gcd_steps);
        if let Some(expected) = previous {
            assert_eq!(usage, expected);
        }
        previous = Some(usage);
    }
    Ok(())
}

/// Positive cancellation retains exact assumptions and its ordinary downstream consumer on fresh replay.
/// ´test:integration:positive-cofactor-fresh-replay´
#[test]
fn positive_cofactor_fresh_replay() -> TestResult {
    for norm in [false, true] {
        let mut source = if norm {
            crate::testing::solve_elementary::positive_norm_cancellation(0)?
        } else {
            crate::testing::solve_elementary::positive_cancellation(
                crate::testing::solve_elementary::PositiveMutation::Valid,
            )?
        };
        let witness = source.base.cas.check_proof(
            &source.base.context,
            &source.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        assert_eq!(witness.hypotheses().len(), if norm { 0 } else { 3 });
        assert_eq!(witness.judgment().residuals, []);
        let bytes = source
            .base
            .cas
            .export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
        let mut fresh = nrz_cas::Cas::new(Limits::blind_cubic())?;
        let replayed = fresh.inspect(&bytes, &mut Budget::new(Limits::blind_cubic())?)?;
        assert_eq!(replayed.hypotheses().len(), witness.hypotheses().len());
        assert_eq!(
            fresh.export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
            bytes
        );
    }
    Ok(())
}

/// The complete public cancellation proof has a measured exact Work boundary and atomic refusal one below.
/// ´test:integration:positive-cofactor-work-boundary´
#[test]
fn positive_cofactor_work_boundary() -> TestResult {
    use crate::testing::solve_elementary::{PositiveMutation, positive_cancellation};
    let mut baseline = positive_cancellation(PositiveMutation::Valid)?;
    let mut measured = Budget::new(Limits::blind_cubic())?;
    baseline
        .base
        .cas
        .check_proof(&baseline.base.context, &baseline.steps, &mut measured)?;
    let required = measured.usage().work;
    // NRZCAS04 adds 9,059 witness-DAG Work and 138 record-encoding Work; arithmetic is unchanged.
    assert_eq!(required, 294_727);
    for cap in [
        required,
        required.checked_sub(1).ok_or("empty positive ledger")?,
    ] {
        let mut fixture = positive_cancellation(PositiveMutation::Valid)?;
        let mut limits = Limits::blind_cubic();
        limits.work = cap;
        let mut budget = Budget::new(limits)?;
        let before = fixture.base.cas.counts();
        let result =
            fixture
                .base
                .cas
                .check_proof(&fixture.base.context, &fixture.steps, &mut budget);
        if cap == required {
            result?;
            assert_eq!(budget.usage().work, required);
        } else {
            let error = result
                .err()
                .ok_or("positive cancellation accepted one below")?;
            assert!(
                matches!(error.kind, nrz_cas::FaultKind::Exhausted { budget: nrz_cas::limits::BudgetKind::Work, limit, needed, .. } if limit == cap && needed > cap)
            );
            assert_eq!(fixture.base.cas.counts(), before);
        }
    }
    Ok(())
}

/// Every prefix of the actual selected-root norm proof replays independently.
/// ´test:integration:positive-cofactor-norm-prefix´
#[test]
fn positive_cofactor_norm_prefix() -> TestResult {
    crate::testing::solve_elementary::positive_norm_prefix()
}

/// A real odd root witnesses a complex selection with zero imaginary part, including fresh replay.
/// ´test:integration:real-witness-complex-selection´
#[test]
fn real_witness_complex_selection() -> TestResult {
    let mut fixture = crate::testing::solve_elementary::real_witness_selection(0)?;
    let witness = fixture.base.cas.check_proof(
        &fixture.base.context,
        &fixture.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert!(witness.is_discharged());
    let bytes = fixture
        .base
        .cas
        .export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
    let mut fresh = nrz_cas::Cas::new(Limits::blind_cubic())?;
    let replayed = fresh.inspect(&bytes, &mut Budget::new(Limits::blind_cubic())?)?;
    assert!(replayed.is_discharged());
    assert_eq!(
        fresh.export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}

/// Witness inclusion retains the exact public Work boundary and refuses atomically one below.
/// ´test:integration:real-witness-work-boundary´
#[test]
fn real_witness_work_boundary() -> TestResult {
    let mut fixture = crate::testing::solve_elementary::real_witness_selection(0)?;
    let mut measured = Budget::new(Limits::blind_cubic())?;
    fixture
        .base
        .cas
        .check_proof(&fixture.base.context, &fixture.steps, &mut measured)?;
    let required = measured.usage().work;
    // NRZCAS04 adds 5,731 witness-DAG Work and 100 record-encoding Work; arithmetic is unchanged.
    assert_eq!(required, 127_145);
    for cap in [
        required,
        required.checked_sub(1).ok_or("empty inclusion ledger")?,
    ] {
        let mut fixture = crate::testing::solve_elementary::real_witness_selection(0)?;
        let mut limits = Limits::blind_cubic();
        limits.work = cap;
        let mut budget = Budget::new(limits)?;
        let before = fixture.base.cas.counts();
        let result =
            fixture
                .base
                .cas
                .check_proof(&fixture.base.context, &fixture.steps, &mut budget);
        if cap == required {
            result?;
            assert_eq!(budget.usage().work, required);
        } else {
            let fault = result.err().ok_or("inclusion accepted one below")?;
            assert!(
                matches!(fault.kind, nrz_cas::FaultKind::Exhausted { budget: nrz_cas::limits::BudgetKind::Work, limit, needed, .. } if limit == cap && needed > cap)
            );
            assert_eq!(fixture.base.cas.counts(), before);
        }
    }
    Ok(())
}
