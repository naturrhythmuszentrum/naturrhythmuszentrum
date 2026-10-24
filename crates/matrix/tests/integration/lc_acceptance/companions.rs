// SPDX-License-Identifier: AGPL-3.0-only
//! Original uncertain L/C and every selected radius version feed both numerical operations.

use crate::kernel_oracle::Rational;
use crate::reading_testing::{TestResult, ledger};
use crate::spectral_testing::Fixture;
use nrz_matrix::{Binding, Coverage, DependencyPolicy, ExecutionLimits, Heap, ReadingBinding};
use nrz_scalar::ArithmeticContext;

type OperatorResult = (
    nrz_matrix::Outcome<Vec<nrz_matrix::PublishedBundle<f64>>>,
    Vec<ReadingBinding<f64>>,
);

/// Every value and companion mutation invalidates both publications, even when its bits are unchanged.
/// ´test:integration:lc-acceptance-uncertain-inputs-share-companion-lineage´
#[test]
fn lc_acceptance_uncertain_inputs_share_companion_lineage() -> TestResult {
    let fixture = Fixture::with_outputs(["5", "1/10", "1/2000"])
        .map_err(|e| format!("declarations: {e:?}"))?;
    for mutation in 0..6 {
        let (mut heap, sources) = fixture
            .import([
                [0.25_f64, 0.015_625, 0.000_976_562_5],
                [0.0625, 0.003_906_25, 0.000_244_140_625],
            ])
            .map_err(|e| format!("import: {e:?}"))?;
        let (outcome, readings) = execute(&fixture, &mut heap, &sources)?;
        let bundle = crate::kernel_testing::output(&outcome)
            .map_err(|e| format!("operator output: {e:?}"))?;
        operator_oracle(&heap, bundle)?;
        let plan = fixture
            .plan(sources.clone(), &mut ledger()?)
            .map_err(|e| format!("spectrum plan: {e:?}"))?;
        let publication = fixture
            .publication(plan, &mut ledger()?)
            .map_err(|e| format!("publication plan: {e:?}"))?;
        let context = ArithmeticContext::new().map_err(|e| format!("arithmetic: {e:?}"))?;
        let roots = heap
            .publish_lc_spectrum(&publication, &context, &mut ledger()?)
            .map_err(|e| format!("spectrum publication: {e:?}"))?;
        spectral_oracle(&heap, &roots, &context)?;
        mutate(&mut heap, &sources, mutation)?;
        assert!(heap.operation_record(outcome.record()).is_err());
        assert!(heap.reading_entries(&readings[0]).is_err());
        assert!(roots.reading(0, &heap, &context, &mut ledger()?).is_err());
        assert!(roots.reading(1, &heap, &context, &mut ledger()?).is_err());
    }
    Ok(())
}

/// Both native input channels retain the original companion definitions and parameter positions.
fn execute(
    fixture: &Fixture,
    heap: &mut Heap,
    sources: &[ReadingBinding<f64>; 2],
) -> TestResult<OperatorResult> {
    let operation = super::operator(fixture).map_err(|e| format!("operator definition: {e:?}"))?;
    let (graph, coverage) =
        super::operator_inventory().map_err(|e| format!("operator output inventory: {e:?}"))?;
    let selected = selections(heap, sources)?;
    let mut bound = Binding::from_symbols(operation, selected, ExecutionLimits::default())
        .validate(heap)
        .map_err(|e| format!("operator bind: {e:?}"))?;
    assert!(
        bound
            .dependencies()
            .iter()
            .all(|input| input.carries_bound() && input.carries_evaluation())
    );
    let readings = bound
        .predeclare_result_readings(graph, coverage, DependencyPolicy::WholeBase, &mut ledger()?)
        .map_err(|e| format!("result readings: {e:?}"))?;
    let views = bound
        .normalized_inputs()
        .map_err(|e| format!("input views: {e:?}"))?;
    let candidate = bound
        .evaluate(views)
        .map_err(|e| format!("operator evaluation: {e:?}"))?;
    let outcome = bound
        .publish(candidate)
        .map_err(|e| format!("operator publication: {e:?}"))?;
    Ok((outcome, readings))
}

/// Selection retains every original target edge without introducing fresh physical sources.
fn selections(
    heap: &mut Heap,
    sources: &[ReadingBinding<f64>; 2],
) -> TestResult<Vec<nrz_matrix::ValueBinding<f64>>> {
    let mut selected = Vec::new();
    for source in sources {
        let value = heap
            .select_reading(source, &mut ledger()?)
            .map_err(|e| format!("select original input: {e:?}"))?;
        let bound = value.symbols().bound().ok_or("selected total symbol")?;
        let error = value.symbols().error().ok_or("selected error symbol")?;
        for symbol in [bound, error] {
            assert!(
                source
                    .companions()
                    .iter()
                    .any(|companion| companion.companion().node() == symbol.node())
            );
            assert_eq!(symbol.target(), Some(value.symbol().node()));
        }
        selected.push(value);
    }
    Ok(selected)
}

/// Endpoint reciprocals bound the original physical family independently of the lowered recipe.
fn operator_oracle(heap: &Heap, bundle: nrz_matrix::PublishedBundle<f64>) -> TestResult {
    assert_eq!(
        heap.value(bundle.value())
            .map_err(|e| format!("operator coverage: {e:?}"))?
            .coverage(),
        Coverage::DeclaredBounds
    );
    for (index, endpoints) in [
        (1, [(-8, 75), (-8, 85)]),
        (2, [(16, 93_750), (16, 106_250)]),
    ] {
        let (center, total, error) = crate::kernel_testing::entry(heap, bundle, index)
            .map_err(|e| format!("operator entry: {e:?}"))?;
        for (numerator, denominator) in endpoints {
            assert!(Rational::new(numerator, denominator)?.covered(center, total)?);
        }
        assert!(error < total);
    }
    Ok(())
}

/// Correlated endpoint choices attain these two rates without any independence assumption.
fn spectral_oracle(
    heap: &Heap,
    roots: &nrz_matrix::PublishedLcSpectrum<f64>,
    context: &ArithmeticContext,
) -> TestResult {
    for (index, sign) in [(0, 1), (1, -1)] {
        let value = roots
            .reading(index, heap, context, &mut ledger()?)
            .map_err(|e| format!("spectral reading: {e:?}"))?[1];
        for denominator in [15, 17] {
            assert!(Rational::new(sign * 128, denominator)?.covered(value.center, value.total)?);
        }
        assert!(value.error < value.total);
    }
    Ok(())
}

/// Each original source and each of its own two parameter channels is a distinct invalidation trigger.
fn mutate(heap: &mut Heap, sources: &[ReadingBinding<f64>; 2], mutation: usize) -> TestResult {
    if mutation < 2 {
        let values = heap
            .reading_entries(&sources[mutation])
            .map_err(|e| format!("value before mutation: {e:?}"))?;
        heap.replace_reading_entries(&sources[mutation], &values, &mut ledger()?)
            .map_err(|e| format!("value mutation: {e:?}"))?;
    } else {
        let index = mutation - 2;
        let source = &sources[index / 2];
        let values = heap
            .reading_companion_entries(source, index)
            .map_err(|e| format!("companion before mutation: {e:?}"))?;
        heap.replace_reading_companion_entries(source, index, &values, &mut ledger()?)
            .map_err(|e| format!("companion mutation: {e:?}"))?;
    }
    Ok(())
}

/// Wrong targets, duplicate kinds and a missing required total cannot become selected coverage.
/// ´test:integration:lc-acceptance-refuses-substituted-companion-inventories´
#[test]
fn lc_acceptance_refuses_substituted_companion_inventories() -> TestResult {
    let fixture =
        Fixture::new(["5", "1/10", "1/2000"]).map_err(|e| format!("declarations: {e:?}"))?;
    let source = nrz_bindings::BindingSymbols::from_reading(
        std::rc::Rc::clone(&fixture.graph),
        fixture.coverage.readings()[0].reading(),
        &mut ledger()?,
    )
    .map_err(|e| format!("selected symbol: {e:?}"))?;
    let total = fixture.companions[0].companion();
    let error = fixture.companions[1].companion();
    let foreign = fixture.companions[2].companion();
    for inventory in [vec![foreign], vec![total, total], vec![error]] {
        assert!(
            source
                .clone()
                .with_reading_companions(&inventory, &mut ledger()?)
                .is_err()
        );
    }
    let valid = source
        .with_reading_companions(&[total, error], &mut ledger()?)
        .map_err(|e| format!("complete companions: {e:?}"))?;
    assert!(
        nrz_bindings::BindingSymbols::check(
            valid.value().clone(),
            valid.error().cloned(),
            valid.bound().cloned()
        )
        .is_err()
    );
    Ok(())
}

/// Invalid replacement parameters leave every original selection available and unchanged.
/// ´test:integration:lc-acceptance-companion-replacement-refuses-atomically´
#[test]
fn lc_acceptance_companion_replacement_refuses_atomically() -> TestResult {
    let fixture =
        Fixture::new(["5", "1/10", "1/2000"]).map_err(|e| format!("declarations: {e:?}"))?;
    let (mut heap, sources) = fixture
        .import([
            [0.25_f64, 0.015_625, 0.000_976_562_5],
            [0.0625, 0.003_906_25, 0.000_244_140_625],
        ])
        .map_err(|e| format!("import: {e:?}"))?;
    let selected = heap
        .select_reading(&sources[0], &mut ledger()?)
        .map_err(|e| format!("selection: {e:?}"))?;
    for (index, values) in [
        (0, vec![-1.0]),
        (0, vec![0.0]),
        (1, vec![1.0]),
        (1, vec![f64::NAN]),
        (0, Vec::new()),
        (2, vec![0.0]),
    ] {
        assert!(
            heap.replace_reading_companion_entries(&sources[0], index, &values, &mut ledger()?)
                .is_err()
        );
        assert!(heap.value_by_symbol(selected.symbol(), &selected).is_ok());
    }
    Ok(())
}

/// Only selecting the two scalar radius maps charges their six exact map-entry checks.
/// ´test:integration:lc-acceptance-companion-selection-cost-boundary´
#[test]
fn lc_acceptance_companion_selection_cost_boundary() -> TestResult {
    let fixture =
        Fixture::new(["5", "1/10", "1/2000"]).map_err(|e| format!("selection fixture: {e:?}"))?;
    let (mut heap, sources) = fixture
        .import([[0.25_f64, 0.0, 0.0], [0.0625, 0.0, 0.0]])
        .map_err(|e| format!("selection import: {e:?}"))?;
    let mut resources = ledger().map_err(|e| format!("selection ledger: {e:?}"))?;
    heap.select_reading(&sources[0], &mut resources)
        .map_err(|e| format!("measured selection: {e:?}"))?;
    assert_eq!(resources.usage().work, 6);
    for (limit, prior, success) in [(6, 0, true), (5, 0, false), (6, 1, false)] {
        let mut resources = super::costs::policy(0, limit, prior)
            .map_err(|e| format!("selection policy: {e:?}"))?;
        assert_eq!(
            heap.select_reading(&sources[1], &mut resources).is_ok(),
            success
        );
    }
    Ok(())
}
