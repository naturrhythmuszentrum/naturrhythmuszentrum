// SPDX-License-Identifier: AGPL-3.0-only
//! Fresh quantity transactions measure their complete caller-ledger increments.

use super::{TestResult, celsius, represented};
use nrz_bindings::{BindingSymbols, Budget, Descriptor, Limits};
use nrz_matrix::{
    ExecutionLimits, Heap, HeapLimits, NormalizedQuantity, Quantity, QuantityContext,
    QuantityError, QuantityInput, ShapeLimits,
};

/// Complete public operation vocabulary exercised by the resource boundary pairs.
#[derive(Clone, Copy, Debug)]
enum Action {
    Capture,
    Import,
    Point,
    Difference,
    Add,
    Subtract,
    PointDifference,
    Translate,
    Convert,
    Normalize,
    Restore,
    Narrow,
    Widen,
}

/// Fresh independent inputs whose declaration construction is outside the measured operation.
struct Fixture {
    /// Frozen reference and origin authority.
    context: QuantityContext,
    /// Sole numerical owner for this independent execution.
    heap: Heap,
    /// A Celsius point.
    point: Quantity<f64>,
    /// Compatible temperature difference.
    difference: Quantity<f64>,
    /// Continuous dimensionless value for precision operations.
    ratio: Quantity<f64>,
    /// Same-format binary32 input prepared without a measured conversion.
    single: Quantity<f32>,
    /// Original predeclared import inventory.
    symbols: BindingSymbols,
    /// Restoration input prepared outside every measured execution.
    normalized: NormalizedQuantity<f64>,
}

impl Fixture {
    /// Build identical fresh stores for every measured success or refusal.
    fn new() -> TestResult<Self> {
        let descriptor = celsius()?;
        let context = QuantityContext::new(
            crate::conversion_testing::reference_owner(&descriptor)?,
            ShapeLimits::default(),
            ExecutionLimits::default(),
        );
        let mut heap = Heap::new(HeapLimits::default())?;
        let point = represented(&context, &mut heap, &descriptor, 20.0_f64)?;
        let difference = represented(&context, &mut heap, &descriptor.difference(), 2.0_f64)?;
        let ratio = represented(&context, &mut heap, &Descriptor::ratio(), 0.1_f64)?;
        let single = represented(&context, &mut heap, &Descriptor::ratio(), 0.1_f32)?;
        let symbols = BindingSymbols::declare(
            &Descriptor::ratio(),
            [1, 1],
            [false, false],
            &mut Budget::new(Limits::default())?,
        )?;
        let normalized = context.normalize(
            &mut heap,
            &point,
            "reference",
            Some("origin"),
            &mut Budget::new(Limits::default())?,
        )?;
        Ok(Self {
            context,
            heap,
            point,
            difference,
            ratio,
            single,
            symbols,
            normalized,
        })
    }

    /// Execute exactly one public operation with the supplied caller ledger.
    fn execute(&mut self, action: Action, budget: &mut Budget) -> TestResult {
        let mut setup = Budget::new(Limits::default())?;
        match action {
            Action::Capture => {
                self.context
                    .capture(&self.heap, self.ratio.binding().clone(), budget)?;
            }
            Action::Import => {
                self.context.import(
                    &mut self.heap,
                    &self.symbols,
                    QuantityInput::represented(&[1.0_f64]),
                    budget,
                )?;
            }
            Action::Point => {
                self.point.point(&self.heap, budget)?;
            }
            Action::Difference => {
                self.difference.difference(&self.heap, budget)?;
            }
            Action::Add => {
                self.context
                    .add(&mut self.heap, &self.difference, &self.point, budget)?;
            }
            Action::Subtract => {
                self.context
                    .subtract(&mut self.heap, &self.point, &self.difference, budget)?;
            }
            Action::PointDifference => {
                let point = self.point.point(&self.heap, &mut setup)?;
                self.context
                    .point_difference(&mut self.heap, &point, &point, budget)?;
            }
            Action::Translate => {
                let point = self.point.point(&self.heap, &mut setup)?;
                let difference = self.difference.difference(&self.heap, &mut setup)?;
                self.context
                    .translate(&mut self.heap, &point, &difference, budget)?;
            }
            Action::Convert => {
                self.context.convert(
                    &mut self.heap,
                    &self.point,
                    &self.point.descriptor().coherent(),
                    budget,
                )?;
            }
            Action::Normalize => {
                self.context.normalize(
                    &mut self.heap,
                    &self.point,
                    "reference",
                    Some("origin"),
                    budget,
                )?;
            }
            Action::Restore => {
                self.context
                    .restore(&mut self.heap, &self.normalized, budget)?;
            }
            Action::Narrow => {
                self.context.narrow(&mut self.heap, &self.ratio, budget)?;
            }
            Action::Widen => {
                self.context.widen(&mut self.heap, &self.single, budget)?;
            }
        }
        Ok(())
    }
}

/// Every public operation has a literal success and one-below refusal in fresh stores.
/// ´test:integration:quantity-operation-work-boundaries´
/// ´claim:matrix:quantity-operation-work-boundaries´ Quantity operations pin caller Work at literal N and N minus one while refusals preserve the heap and spent ledger.
#[test]
fn quantity_operation_work_boundaries() -> TestResult {
    for (action, needed) in [
        (Action::Capture, 1),
        (Action::Import, 2),
        (Action::Point, 1),
        (Action::Difference, 1),
        (Action::Add, 12_008),
        (Action::Subtract, 12_008),
        (Action::PointDifference, 12_008),
        (Action::Translate, 12_008),
        (Action::Convert, 12_008),
        (Action::Normalize, 12_008),
        (Action::Restore, 12_008),
        (Action::Narrow, 12_008),
        (Action::Widen, 12_008),
    ] {
        for limit in [needed, needed - 1] {
            let mut fixture = Fixture::new()?;
            let usage = fixture.heap.provenance_usage();
            let stamp = fixture.point.view(&fixture.heap)?.stamp();
            let mut budget = Budget::new(Limits {
                work: limit,
                ..Limits::default()
            })?;
            let result = fixture.execute(action, &mut budget);
            if limit == needed {
                result?;
            } else {
                let Err(fault) = result else {
                    panic!("one-below quantity Work must refuse");
                };
                work_refusal(fault.as_ref(), limit, needed)?;
                assert_eq!(fixture.heap.provenance_usage(), usage);
            }
            assert_eq!(budget.usage().work, limit);
            assert_eq!(fixture.point.view(&fixture.heap)?.stamp(), stamp);
        }
    }
    Ok(())
}

/// Only the expected caller Work fault is accepted, in either scalar format.
fn work_refusal(error: &(dyn std::error::Error + 'static), limit: u64, needed: u64) -> TestResult {
    let fault = if let Some(QuantityError::<f64>::Resources(fault)) = error.downcast_ref() {
        fault
    } else if let Some(QuantityError::<f32>::Resources(fault)) = error.downcast_ref() {
        fault
    } else {
        return Err("expected quantity caller Work refusal".into());
    };
    let nrz_bindings::readings::algebra::FaultKind::Exhausted {
        budget,
        limit: actual_limit,
        needed: actual_needed,
        ..
    } = &fault.kind
    else {
        return Err("expected exhaustion".into());
    };
    assert_eq!(
        *budget,
        nrz_bindings::readings::algebra::limits::BudgetKind::Work
    );
    assert_eq!([*actual_limit, *actual_needed], [limit, needed]);
    Ok(())
}

/// A warm store still admits a fresh complete graph owner for each result.
/// ´test:integration:quantity-warm-conversion-work-boundary´
/// ´claim:matrix:quantity-warm-conversion-work´ Cold and warm conversions each cost 12008 caller Work and retain distinct complete output symbols.
#[test]
fn quantity_warm_conversion_work_boundary() -> TestResult {
    for limit in [24_016, 24_015] {
        let mut fixture = Fixture::new()?;
        let target = fixture.point.descriptor().coherent();
        let mut budget = Budget::new(Limits {
            work: limit,
            ..Limits::default()
        })?;
        let first =
            fixture
                .context
                .convert(&mut fixture.heap, &fixture.point, &target, &mut budget)?;
        assert_eq!(budget.usage().work, 12_008);
        let usage = fixture.heap.provenance_usage();
        let result =
            fixture
                .context
                .convert(&mut fixture.heap, &fixture.point, &target, &mut budget);
        if limit == 24_016 {
            let second = result?;
            assert_eq!(budget.usage().work - 12_008, 12_008);
            assert_eq!(first.descriptor(), second.descriptor());
            assert_ne!(first.symbol().node(), second.symbol().node());
            assert_ne!(
                first.binding().symbols().bound(),
                second.binding().symbols().bound()
            );
            assert_ne!(
                first.binding().symbols().error(),
                second.binding().symbols().error()
            );
        } else {
            let Err(fault) = result else {
                panic!("one-below warm conversion must refuse");
            };
            work_refusal(&fault, 24_015, 24_016)?;
            assert_eq!(fixture.heap.provenance_usage(), usage);
        }
        assert_eq!(budget.usage().work, limit);
        first.view(&fixture.heap)?;
    }
    Ok(())
}

/// Array entry work increases without repeating exact symbol construction for every element.
/// ´test:integration:quantity-array-conversion-work-boundary´
/// ´claim:matrix:quantity-array-conversion-work´ A four-entry quantity conversion costs 12011 caller Work and refuses atomically at 12010.
#[test]
fn quantity_array_conversion_work_boundary() -> TestResult {
    for limit in [12_011, 12_010] {
        let context = super::context()?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let mut setup = Budget::new(Limits::default())?;
        let descriptor = Descriptor::ratio();
        let symbols = BindingSymbols::declare(&descriptor, [4, 1], [false, false], &mut setup)?;
        let input = context.import(
            &mut heap,
            &symbols,
            QuantityInput::represented(&[1.0_f64, 2.0, 3.0, 4.0]),
            &mut setup,
        )?;
        let usage = heap.provenance_usage();
        let mut budget = Budget::new(Limits {
            work: limit,
            ..Limits::default()
        })?;
        let result = context.convert(&mut heap, &input, &descriptor, &mut budget);
        if limit == 12_011 {
            assert_eq!(
                result?.view(&heap)?.region().export()?,
                [1.0, 2.0, 3.0, 4.0]
            );
        } else {
            let Err(fault) = result else {
                panic!("one-below array conversion must refuse");
            };
            work_refusal(&fault, 12_010, 12_011)?;
            assert_eq!(heap.provenance_usage(), usage);
        }
        assert_eq!(budget.usage().work, limit);
    }
    Ok(())
}
