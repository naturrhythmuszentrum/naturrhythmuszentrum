// SPDX-License-Identifier: AGPL-3.0-only
//! Publication boundaries retain earlier charges and leave no partially published root family.

use crate::spectral_testing::{Fixture, TestResult, ledger};
use nrz_bindings::{
    Budget, Limits,
    readings::algebra::{Location, limits::BudgetKind},
};
use nrz_matrix::{Heap, LcSpectralPublication};
use nrz_scalar::ArithmeticContext;
type Case = (Heap, LcSpectralPublication<f64>);

fn setup(fixture: &Fixture) -> TestResult<Case> {
    let (heap, inputs) = fixture.import([[1.0_f64, 0.125, 0.0]; 2])?;
    let spectrum = fixture.plan(inputs, &mut ledger()?)?;
    let publication = fixture.publication(spectrum, &mut ledger()?)?;
    Ok((heap, publication))
}

/// N succeeds and N-1 refuses without publishing any root or component, even on a retry.
/// ´test:integration:lc-spectral-publication-work-is-atomic´
#[test]
fn lc_spectral_publication_work_is_atomic() -> TestResult {
    let fixture = Fixture::with_outputs(["5", "1/10", "1/2000"])?;
    let context = ArithmeticContext::new()?;
    let (mut heap, publication) = setup(&fixture)?;
    let mut measured = ledger()?;
    measured.work(17, Location::new("earlier publication work"))?;
    heap.publish_lc_spectrum(&publication, &context, &mut measured)?;
    let required = measured.usage().work;
    for work in [required, required - 1] {
        let (mut heap, publication) = setup(&fixture)?;
        let mut budget = Budget::new(Limits {
            work,
            ..Limits::default()
        })?;
        budget.work(17, Location::new("earlier publication work"))?;
        let result = heap.publish_lc_spectrum(&publication, &context, &mut budget);
        assert_eq!(result.is_ok(), work == required, "{result:?}");
        if result.is_err() {
            let spent = budget.usage().work;
            assert!(
                heap.publish_lc_spectrum(&publication, &context, &mut budget)
                    .is_err()
            );
            assert!(budget.usage().work >= spent);
            let published = heap.publish_lc_spectrum(&publication, &context, &mut ledger()?)?;
            assert_eq!(published.readings().bindings().len(), 6);
        }
    }
    Ok(())
}

/// Staged and output limits distinguish publication plans from execution and retain caller payload.
/// ´test:integration:lc-spectral-publication-payload-boundaries´
#[test]
fn lc_spectral_publication_payload_boundaries() -> TestResult {
    let fixture = Fixture::with_outputs(["5", "1/10", "1/2000"])?;
    let mut costs = Vec::new();
    for kind in [BudgetKind::StagedPayloadBytes, BudgetKind::OutputBytes] {
        for planning in [true, false] {
            let mut lower = 1;
            let mut upper = Limits::default().output_bytes;
            while lower < upper {
                let middle = lower + (upper - lower) / 2;
                if attempt(&fixture, kind, middle, 0, planning)? {
                    upper = middle;
                } else {
                    lower = middle + 1;
                }
            }
            assert!(attempt(&fixture, kind, upper, 0, planning)?);
            assert!(!attempt(&fixture, kind, upper - 1, 0, planning)?);
            if kind == BudgetKind::StagedPayloadBytes {
                assert!(attempt(&fixture, kind, upper + 23, 23, planning)?);
                assert!(!attempt(&fixture, kind, upper + 22, 23, planning)?);
            }
            costs.push(upper);
        }
    }
    assert_eq!(costs, vec![503_608, 1_010_208, 503_608, 505_400]);
    Ok(())
}

fn attempt(
    fixture: &Fixture,
    kind: BudgetKind,
    limit: u64,
    retained: u64,
    planning: bool,
) -> TestResult<bool> {
    let mut limits = Limits::default();
    match kind {
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = limit,
        _ => limits.output_bytes = limit,
    }
    let mut budget = Budget::new(limits)?;
    let (mut heap, inputs) = fixture.import([[1.0_f64, 0.125, 0.0]; 2])?;
    let spectrum = fixture.plan(inputs, &mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    Ok(budget
        .with_retained_payload(
            retained,
            Location::new("caller retained output metadata"),
            |budget| {
                if planning {
                    fixture.publication(spectrum, budget).map(|_| ())
                } else {
                    let publication = fixture.publication(spectrum, &mut ledger()?)?;
                    heap.publish_lc_spectrum(&publication, &context, budget)?;
                    Ok(())
                }
            },
        )
        .is_ok())
}

/// Complete-header closure grows only when output declarations belong to the physical header.
/// ´test:integration:lc-spectral-construction-phase-boundaries´
#[test]
fn lc_spectral_construction_phase_boundaries() -> TestResult {
    use crate::spectral_testing::construction;
    let expected = [
        [108_364, 444_642, 475_992, 1_080_903, 0, 4_945, 2_109_901],
        [37_235, 345_992, 376_830, 944_766, 54_239, 2_972, 1_759_062],
        [37_235, 345_992, 376_830, 944_766, 54_239, 2_972, 1_759_062],
    ];
    for (mode, row) in expected.into_iter().enumerate() {
        assert_eq!(construction(mode, [Limits::default().work; 5])?, row);
    }
    for phase in 0..5 {
        let required = expected[1][phase];
        for limit in [required, required - 1] {
            let mut limits = [Limits::default().work; 5];
            limits[phase] = limit;
            assert_eq!(construction(1, limits).is_ok(), limit == required);
        }
    }
    Ok(())
}

/// The complete output definition preserves earlier Work and staged bytes at exact boundaries.
/// ´test:integration:lc-spectral-result-definition-boundaries´
#[test]
fn lc_spectral_result_definition_boundaries() -> TestResult {
    use nrz_bindings::readings::LcSpectralResultDefinition;
    let fixture = Fixture::with_outputs(["5", "1/10", "1/2000"])?;
    let outputs = fixture.outputs.as_ref().ok_or("outputs")?;
    let definition = &outputs.definition;
    let required = outputs.definition_work;
    let bytes = definition.retained_payload_bytes();
    for (kind, need, prior) in [
        (BudgetKind::Work, required, 17),
        (BudgetKind::StagedPayloadBytes, bytes, 23),
        (BudgetKind::OutputBytes, bytes, 0),
    ] {
        for limit in [need + prior, need + prior - 1] {
            let mut limits = Limits::default();
            match kind {
                BudgetKind::Work => limits.work = limit,
                BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = limit,
                _ => limits.output_bytes = limit,
            }
            let mut budget = Budget::new(limits)?;
            if kind == BudgetKind::Work {
                budget.work(prior, Location::new("earlier definition work"))?;
            }
            let retained = if kind == BudgetKind::StagedPayloadBytes {
                prior
            } else {
                0
            };
            let result = budget.with_retained_payload(
                retained,
                Location::new("retained definition"),
                |budget| {
                    LcSpectralResultDefinition::new(
                        &fixture.graph,
                        fixture.law.clone(),
                        definition.outputs().clone(),
                        definition.components().clone(),
                        budget,
                    )
                },
            );
            assert_eq!(result.is_ok(), limit == need + prior, "{result:?}");
        }
    }
    Ok(())
}

/// Separate physical and output declarations retain their cumulative exact Work boundaries.
/// ´test:integration:lc-spectral-complete-construction-work-boundaries´
#[test]
fn lc_spectral_complete_construction_work_boundaries() {
    let mut costs = [0; 2];
    for (phase, cost) in costs.iter_mut().enumerate() {
        let mut lower = 1;
        let mut upper = Limits::default().work;
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            let mut limits = [Limits::default().work; 2];
            limits[phase] = middle;
            if Fixture::with_construction_limits(limits).is_ok() {
                upper = middle;
            } else {
                lower = middle + 1;
            }
        }
        *cost = upper;
        for work in [upper, upper - 1] {
            let mut limits = [Limits::default().work; 2];
            limits[phase] = work;
            assert_eq!(
                Fixture::with_construction_limits(limits).is_ok(),
                work == upper
            );
        }
    }
    assert_eq!(costs, [1_704_823, 1_534_564]);
}

/// Measure each declaration kind and its native proof, separately from the immutable shared law.
/// ´test:integration:lc-spectral-output-construction-costs´
#[test]
fn lc_spectral_output_construction_costs() -> TestResult {
    let fixture = Fixture::with_outputs(["5", "1/10", "1/2000"])?;
    let outputs = fixture.outputs.as_ref().ok_or("outputs")?;
    let definition = &outputs.definition;
    let overlaps = outputs.coverage.overlaps();
    let first = overlaps.first().ok_or("overlap proof")?;
    for relation in overlaps {
        assert_eq!(relation.witness().judgment(), first.witness().judgment());
        assert_eq!(relation.witness().proof(), first.witness().proof());
    }

    let witnesses = definition
        .outputs()
        .iter()
        .map(nrz_bindings::readings::Representation::witness)
        .chain(
            definition
                .components()
                .iter()
                .map(nrz_bindings::readings::ReadingRelation::witness),
        )
        .collect::<Vec<_>>();
    assert!(definition.law().shares_allocation(&fixture.law));
    let family = definition.outputs()[0].reading().family();
    for output in definition.outputs() {
        assert!(std::ptr::eq(
            family.original_entries().as_ptr(),
            output.reading().family().original_entries().as_ptr()
        ));
    }
    for (index, witness) in witnesses.iter().enumerate() {
        assert!(
            !witnesses[..index]
                .iter()
                .any(|prior| prior.proof() == witness.proof())
        );
    }
    assert_eq!(
        (
            &outputs.costs,
            witnesses
                .iter()
                .map(|proof| proof.exported_byte_len())
                .collect::<Vec<_>>()
        ),
        (
            &crate::spectral_testing::Costs {
                setup: 63_788,
                views: [
                    [106_250, 93_043],
                    [106_171, 93_107],
                    [95_263, 87_404],
                    [95_263, 87_404],
                    [95_611, 84_768],
                    [95_707, 84_832]
                ],
                components: [
                    [89_788, 88_792],
                    [89_965, 88_969],
                    [89_820, 88_824],
                    [90_029, 89_033]
                ],
                overlaps: [[68_974, 66_722]; 6],
                companions: [
                    [11_550, 5167],
                    [11_550, 5167],
                    [11_582, 5199],
                    [11_582, 5199],
                    [10_564, 3979],
                    [10_564, 3979],
                    [10_596, 4011],
                    [10_596, 4011]
                ],
                adapters: 718,
            },
            vec![2741, 2741, 2644, 2644, 2651, 2651, 2768, 2774, 2768, 2774]
        )
    );
    Ok(())
}

/// Publication planning retains earlier caller Work and refuses exactly one unit below its cost.
/// ´test:integration:lc-spectral-publication-planning-work-boundary´
#[test]
fn lc_spectral_publication_planning_work_boundary() -> TestResult {
    let fixture = Fixture::with_outputs(["5", "1/10", "1/2000"])?;
    let (_heap, inputs) = fixture.import([[1.0_f64, 0.0, 0.0]; 2])?;
    let spectrum = fixture.plan(inputs, &mut ledger()?)?;
    for work in [140, 139] {
        let mut budget = Budget::new(Limits {
            work,
            ..Limits::default()
        })?;
        budget.work(17, Location::new("earlier planning work"))?;
        assert_eq!(
            fixture.publication(spectrum.clone(), &mut budget).is_ok(),
            work == 140
        );
    }
    Ok(())
}
