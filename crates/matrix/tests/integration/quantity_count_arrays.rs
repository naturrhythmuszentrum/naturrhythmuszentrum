// SPDX-License-Identifier: AGPL-3.0-only
//! Exact count arrays retain shaped source evidence and publish no failed prefixes.

use super::quantity_counts::context;
use crate::conversion_testing::TestResult;
use nrz_bindings::readings::algebra::{FaultKind, limits::BudgetKind};
use nrz_bindings::{Budget, Limits, UnitError};
use nrz_matrix::{CountArray, CountError, CountHistory, CountQuantity, Extent};

/// Build each array from independently admitted exact declarations and a fresh owner.
fn array(source: &str, names: &[&str], rows: usize, columns: usize) -> TestResult<CountArray> {
    let context = context()?;
    let mut budget = Budget::new(Limits::default())?;
    let authority = context.count(source, u128::MAX, &mut budget)?;
    let elements = names
        .iter()
        .map(|name| context.count(name, u128::MAX, &mut budget))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CountArray::capture(
        authority.source().clone(),
        Extent::new(rows, columns)?,
        &elements,
        &mut budget,
    )?)
}

/// Exact integer oracles inspect values without a floating conversion or structure snapshot.
fn values(array: &CountArray) -> Vec<i128> {
    array.elements().iter().map(CountQuantity::value).collect()
}

/// Rectangular metadata and ordered source evidence survive every exact elementwise operation.
/// ´test:integration:quantity-count-array-exact-oracles´
/// ´claim:matrix:quantity-count-array-exact-oracles´ Count arrays preserve column order, checked extents, exact signed integer results and original operand histories without floating storage.
#[test]
fn quantity_count_array_exact_oracles() -> TestResult {
    let left = array(
        "negative",
        &["negative", "signed_two", "negative", "signed_two"],
        2,
        2,
    )?;
    let right = array(
        "signed_two",
        &["signed_two", "negative", "signed_two", "negative"],
        2,
        2,
    )?;
    let mut budget = Budget::new(Limits::default())?;
    assert_eq!(left.extent(), Extent::new(2, 2)?);
    assert_eq!(left.get(1, 0)?.value(), 2);
    assert_eq!(left.get(0, 1)?.value(), -3);
    let Err(CountError::Shape) = left.get(2, 0) else {
        panic!("row outside array must refuse");
    };
    let sum = left.add(&right, 1, &mut budget)?;
    assert_eq!(values(&sum), [-1, -1, -1, -1]);
    assert_eq!(
        values(&left.subtract(&right, 5, &mut budget)?),
        [-5, 5, -5, 5]
    );
    assert_eq!(values(&left.scale(-2, 6, &mut budget)?), [6, -4, 6, -4]);
    let doubled = left.scale(2, 6, &mut budget)?;
    assert_eq!(
        values(&doubled.quotient(2, 3, &mut budget)?),
        [-3, 2, -3, 2]
    );
    let CountHistory::Add(first, second) = sum.get(0, 0)?.history() else {
        panic!("sum must retain exact operands");
    };
    assert_eq!([first.value(), second.value()], [-3, 2]);
    assert_eq!(first.source().value(), -3);
    assert_eq!(sum.source().value(), -3);
    assert_eq!(sum.descriptor(), left.descriptor());
    assert_eq!(values(&left), [-3, 2, -3, 2]);
    assert_eq!(values(&right), [2, -3, 2, -3]);
    Ok(())
}

/// Shape and counted meaning remain mandatory even when an array contains no elements.
/// ´test:integration:quantity-count-array-shape-kind-refusals´
/// ´claim:matrix:quantity-count-array-shape-kind-refusals´ Array length, extent, indexing and counted-kind refusals retain all original values and empty arrays keep explicit source authority.
#[test]
fn quantity_count_array_shape_kind_refusals() -> TestResult {
    let left = array("negative", &["negative", "signed_two"], 2, 1)?;
    let row = array("negative", &["negative", "signed_two"], 1, 2)?;
    let other = array("two", &["two", "three"], 2, 1)?;
    let mut budget = Budget::new(Limits::default())?;
    let Err(CountError::Shape) = CountArray::capture(
        left.source().clone(),
        Extent::new(1, 1)?,
        left.elements(),
        &mut budget,
    ) else {
        panic!("array length must match its extent");
    };
    let Err(CountError::Units(UnitError::KindMismatch)) = CountArray::capture(
        other.source().clone(),
        left.extent(),
        left.elements(),
        &mut budget,
    ) else {
        panic!("array elements must match the explicit counted kind");
    };
    for result in [
        left.add(&row, 9, &mut budget),
        left.subtract(&row, 9, &mut budget),
    ] {
        let Err(CountError::Shape) = result else {
            panic!("equal element counts cannot replace equal extents");
        };
    }
    let Err(CountError::Units(UnitError::KindMismatch)) = left.add(&other, 9, &mut budget) else {
        panic!("different counted kinds must refuse");
    };
    assert_eq!(values(&left.add(&left, 6, &mut budget)?), [-6, 4]);
    assert_eq!(values(&left), [-3, 2]);
    Ok(())
}

/// Empty shapes keep counted meaning and the explicit nonzero-divisor obligation.
/// ´test:integration:quantity-count-array-empty-semantics´
/// ´claim:matrix:quantity-count-array-empty-semantics´ Empty exact arrays preserve their declared shape and source kind through arithmetic while mismatched kinds and zero divisors refuse.
#[test]
fn quantity_count_array_empty_semantics() -> TestResult {
    let mut budget = Budget::new(Limits::default())?;
    let empty = array("negative", &[], 0, 2)?;
    let different = array("two", &[], 0, 2)?;
    assert_eq!(empty.source().value(), -3);
    assert_eq!(empty.extent(), Extent::new(0, 2)?);
    let Err(CountError::Units(UnitError::KindMismatch)) = empty.add(&different, 0, &mut budget)
    else {
        panic!("empty arrays still require matching counted meanings");
    };
    let Err(CountError::ZeroDivisor) = empty.quotient(0, 0, &mut budget) else {
        panic!("empty arrays retain the original zero divisor obligation");
    };
    for result in [
        empty.add(&empty, 0, &mut budget)?,
        empty.subtract(&empty, 0, &mut budget)?,
        empty.scale(i128::MIN, 0, &mut budget)?,
        empty.quotient(-1, 0, &mut budget)?,
    ] {
        assert_eq!(values(&result), [] as [i128; 0]);
        assert_eq!(result.extent(), empty.extent());
        assert_eq!(result.descriptor(), empty.descriptor());
    }
    Ok(())
}

/// Late semantic failures discard every privately prepared element while retaining spent Work.
/// ´test:integration:quantity-count-array-late-refusals´
/// ´claim:matrix:quantity-count-array-late-refusals´ A late fractional, out-of-domain or magnitude-capacity result refuses the whole array and has an exact successful twin with unchanged inputs.
#[test]
fn quantity_count_array_late_refusals() -> TestResult {
    let left = array("signed_two", &["signed_two", "negative"], 2, 1)?;
    let bounded = array("bounded_two", &["bounded_two", "bounded_three"], 2, 1)?;
    let twos = array("bounded_two", &["bounded_two", "bounded_two"], 2, 1)?;
    let mut budget = Budget::new(Limits::default())?;
    let Err(CountError::Fractional) = left.quotient(2, 3, &mut budget) else {
        panic!("a late fractional partition must refuse the whole array");
    };
    assert!(budget.usage().work > 2);
    assert_eq!(
        values(&left.scale(2, 6, &mut budget)?.quotient(2, 3, &mut budget)?),
        [2, -3]
    );
    let spent = budget.usage().work;
    let Err(CountError::Capacity) = left.scale(2, 4, &mut budget) else {
        panic!("a late magnitude capacity failure must refuse the whole array");
    };
    assert!(budget.usage().work > spent);
    assert_eq!(values(&left.scale(2, 6, &mut budget)?), [4, -6]);
    let spent = budget.usage().work;
    let Err(CountError::Units(UnitError::CountValue)) = bounded.add(&twos, 5, &mut budget) else {
        panic!("a late actual-domain failure must refuse the whole array");
    };
    assert!(budget.usage().work > spent);
    assert_eq!(values(&twos.add(&twos, 4, &mut budget)?), [4, 4]);
    assert_eq!(values(&left), [2, -3]);
    assert_eq!(values(&bounded), [2, 3]);
    assert_eq!(values(&twos), [2, 2]);
    Ok(())
}

/// Both signed endpoints remain exact through successful twins of every overflow operation.
/// ´test:integration:quantity-count-array-overflow-refusals´
/// ´claim:matrix:quantity-count-array-overflow-refusals´ Late addition, subtraction, multiplication and signed-division overflow discard complete array candidates without changing either signed endpoint.
#[test]
fn quantity_count_array_overflow_refusals() -> TestResult {
    let maximum = array("negative", &["negative", "signed_max"], 2, 1)?;
    let minimum = array("negative", &["negative", "signed_min"], 2, 1)?;
    let twos = array("signed_two", &["signed_two", "signed_two"], 2, 1)?;
    let mut budget = Budget::new(Limits::default())?;
    for result in [
        maximum.add(&twos, u128::MAX, &mut budget),
        minimum.subtract(&twos, u128::MAX, &mut budget),
        maximum.scale(2, u128::MAX, &mut budget),
        minimum.quotient(-1, u128::MAX, &mut budget),
    ] {
        let Err(CountError::Overflow) = result else {
            panic!("late signed storage overflow must refuse the whole array");
        };
    }
    let zeros = twos.scale(0, 0, &mut budget)?;
    assert_eq!(
        values(&maximum.add(&zeros, u128::MAX, &mut budget)?),
        [-3, i128::MAX]
    );
    assert_eq!(
        values(&minimum.subtract(&zeros, u128::MAX, &mut budget)?),
        [-3, i128::MIN]
    );
    assert_eq!(
        values(&maximum.scale(1, u128::MAX, &mut budget)?),
        [-3, i128::MAX]
    );
    assert_eq!(
        values(&minimum.quotient(1, u128::MAX, &mut budget)?),
        [-3, i128::MIN]
    );
    assert_eq!(values(&maximum), [-3, i128::MAX]);
    assert_eq!(values(&minimum), [-3, i128::MIN]);
    Ok(())
}

/// Apply only the operation under measurement to the supplied caller ledger.
fn operation(
    action: u8,
    left: &CountArray,
    right: &CountArray,
    budget: &mut Budget,
) -> Result<CountArray, CountError> {
    match action {
        0 => CountArray::capture(
            left.source().clone(),
            left.extent(),
            left.elements(),
            budget,
        ),
        1 => left.add(right, 9, budget),
        2 => left.subtract(right, 9, budget),
        3 => left.scale(-2, 9, budget),
        4 => left.quotient(1, 9, budget),
        _ => unreachable!("closed array operation inventory"),
    }
}

/// Every array operation has fresh literal Work and complete retained-storage boundaries.
/// ´test:integration:quantity-count-array-work-boundaries´
/// ´claim:matrix:quantity-count-array-work-boundaries´ Exact array capture and arithmetic publish only at measured Work and retained-storage bounds while one-below failures preserve inputs and prior Work.
#[test]
fn quantity_count_array_work_boundaries() -> TestResult {
    for (action, needed, bytes) in [
        (0, 3, 6_535),
        (1, 459, 11_127),
        (2, 458, 11_127),
        (3, 458, 6_759),
        (4, 458, 6_759),
    ] {
        for boundary in 0..3 {
            for accepted in [true, false] {
                let left = array("negative", &["negative", "signed_two"], 2, 1)?;
                let right = array("signed_two", &["signed_two", "negative"], 2, 1)?;
                let mut limits = Limits::default();
                let expected = match boundary {
                    0 => {
                        limits.work = needed + 17 - u64::from(!accepted);
                        BudgetKind::Work
                    }
                    1 => {
                        limits.output_bytes = bytes - u64::from(!accepted);
                        BudgetKind::OutputBytes
                    }
                    _ => {
                        limits.staged_payload_bytes = bytes - u64::from(!accepted);
                        BudgetKind::StagedPayloadBytes
                    }
                };
                let mut budget = Budget::new(limits)?;
                budget.work(
                    17,
                    nrz_bindings::readings::algebra::Location::new("prior count array work"),
                )?;
                let result = operation(action, &left, &right, &mut budget);
                if accepted {
                    let result = result?;
                    assert_eq!(result.retained_payload_bytes(), bytes);
                    assert_eq!(budget.usage().work, needed + 17);
                    assert_eq!(result.extent(), left.extent());
                } else {
                    let Err(fault) = result else {
                        panic!("one-below array resource boundary must refuse");
                    };
                    array_resource_refusal(&fault, expected)?;
                }
                assert!(budget.usage().work >= 17);
                assert_eq!(values(&left), [-3, 2]);
                assert_eq!(values(&right), [2, -3]);
            }
        }
    }
    Ok(())
}

/// Nested exact arithmetic preserves the precise caller-resource cause.
fn array_resource_refusal(error: &CountError, expected: BudgetKind) -> TestResult {
    let (CountError::Resources(fault) | CountError::Units(UnitError::Arithmetic(fault))) = error
    else {
        return Err("expected array resource exhaustion".into());
    };
    let FaultKind::Exhausted { budget, .. } = fault.kind else {
        return Err("expected exhausted array policy".into());
    };
    assert_eq!(budget, expected);
    Ok(())
}
