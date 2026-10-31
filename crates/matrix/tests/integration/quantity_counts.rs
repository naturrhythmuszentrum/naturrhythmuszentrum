// SPDX-License-Identifier: AGPL-3.0-only
//! Exact count arithmetic and explicit rational ratios retain original immutable evidence.

use crate::conversion_testing::TestResult;
use nrz_bindings::readings::algebra::Rational;
use nrz_bindings::{
    BaseDimension, Budget, Context, CountDomain, Declaration, Descriptor, Dimension, Evidence,
    ExponentPolicy, Kind, Limits, Measure, Registry, Role, UnitError,
};
use nrz_matrix::{
    CountError, CountHistory, CountQuantity, ExecutionLimits, QuantityContext, ShapeLimits,
};
use std::rc::Rc;

/// Freeze count sources, distinct continuous meanings and nonquantity metadata.
#[expect(
    clippy::default_trait_access,
    reason = "CAS policies are inferred through the bindings facade"
)]
pub(super) fn context() -> TestResult<QuantityContext> {
    let mut owner = Context::with_real_symbols(
        Registry::new(ExponentPolicy::Rational),
        Default::default(),
        &[
            "three",
            "two",
            "zero",
            "maximum",
            "other",
            "bounded_three",
            "bounded_two",
            "ratio",
            "angle",
            "amount",
            "expected",
            "metadata",
            "inputcount",
            "negative",
            "signed_two",
            "signed_min",
            "signed_max",
        ],
    )?;
    let events = Kind::Count("events".into(), CountDomain::NONNEGATIVE);
    let bounded = Kind::Count("bounded-events".into(), CountDomain::new(Some(0), Some(4))?);
    for (name, kind, value) in [
        ("three", events.clone(), "3"),
        ("two", events.clone(), "2"),
        ("zero", events, "0"),
        (
            "maximum",
            Kind::Count("events".into(), CountDomain::NONNEGATIVE),
            "170141183460469231731687303715884105727",
        ),
        (
            "other",
            Kind::Count("photons".into(), CountDomain::NONNEGATIVE),
            "2",
        ),
        ("bounded_three", bounded.clone(), "3"),
        ("bounded_two", bounded, "2"),
        ("ratio", Kind::Ratio, "2"),
        ("angle", Kind::Angle, "2"),
        (
            "amount",
            Kind::Named(
                "amount".into(),
                Box::new(Dimension::base(BaseDimension::Amount)),
            ),
            "2",
        ),
        (
            "expected",
            Kind::Named("expected-events".into(), Box::new(Dimension::zero())),
            "3/2",
        ),
    ] {
        owner.declare(
            name.into(),
            Declaration::Constant {
                descriptor: Descriptor::new(kind, Measure::coherent(), Role::Difference)?,
                value: Rational::parse(value, &mut Budget::new(Limits::default())?)?,
                evidence: Evidence::default(),
            },
        )?;
    }
    owner.declare(
        "metadata".into(),
        Declaration::Metadata("sample count".into()),
    )?;
    input_count(&mut owner)?;
    signed_counts(&mut owner)?;
    Ok(QuantityContext::new(
        Rc::new(owner.freeze()?),
        ShapeLimits::default(),
        ExecutionLimits::default(),
    ))
}

/// Signed source declarations share one explicit counted kind and unbounded integer domain.
fn signed_counts(owner: &mut Context) -> TestResult {
    for (name, value) in [
        ("negative", "-3"),
        ("signed_two", "2"),
        ("signed_min", "-170141183460469231731687303715884105728"),
        ("signed_max", "170141183460469231731687303715884105727"),
    ] {
        owner.declare(
            name.into(),
            Declaration::Constant {
                descriptor: Descriptor::new(
                    Kind::Count("signed-events".into(), CountDomain::SIGNED),
                    Measure::coherent(),
                    Role::Difference,
                )?,
                value: Rational::parse(value, &mut Budget::new(Limits::default())?)?,
                evidence: Evidence::default(),
            },
        )?;
    }
    Ok(())
}

/// An input descriptor alone supplies no exact whole-number value.
fn input_count(owner: &mut Context) -> TestResult {
    owner.declare(
        "inputcount".into(),
        Declaration::Input {
            descriptor: Descriptor::new(
                Kind::Count("events".into(), CountDomain::NONNEGATIVE),
                Measure::coherent(),
                Role::Difference,
            )?,
            nonzero: false,
            evidence: Evidence::default(),
        },
    )?;
    Ok(())
}

/// Ordinary arithmetic keeps exact counted meaning and the actual result domain.
/// ´test:integration:quantity-count-arithmetic-retains-exact-values´
/// ´claim:matrix:quantity-count-arithmetic´ Count addition, subtraction, integer scaling and whole division retain exact values and original source evidence.
#[test]
fn quantity_count_arithmetic_retains_exact_values() -> TestResult {
    let context = context()?;
    let mut budget = Budget::new(Limits::default())?;
    let three = context.count("three", u128::MAX, &mut budget)?;
    let two = context.count("two", u128::MAX, &mut budget)?;
    let sum = three.add(&two, u128::MAX, &mut budget)?;
    assert_eq!(sum.value(), 5);
    assert_eq!(three.subtract(&two, u128::MAX, &mut budget)?.value(), 1);
    assert_eq!(three.scale(2, u128::MAX, &mut budget)?.value(), 6);
    assert_eq!(two.quotient(2, u128::MAX, &mut budget)?.value(), 1);
    assert_eq!(sum.descriptor(), three.descriptor());
    let CountHistory::Add(left, right) = sum.history() else {
        panic!("sum retains both exact operands");
    };
    assert_eq!([left.value(), right.value()], [3, 2]);
    assert!(
        matches!(sum.source().evidence(), nrz_bindings::CountEvidence::Declared { name, .. } if name == "three")
    );
    let bounded_three = context.count("bounded_three", u128::MAX, &mut budget)?;
    let bounded_two = context.count("bounded_two", u128::MAX, &mut budget)?;
    assert_eq!(
        bounded_three
            .subtract(&bounded_two, u128::MAX, &mut budget)?
            .value(),
        1
    );
    let Err(CountError::Units(fault)) = bounded_three.add(&bounded_two, u128::MAX, &mut budget)
    else {
        panic!("the actual result must remain in its declared range");
    };
    assert_eq!(fault, UnitError::CountValue);
    Ok(())
}

/// Fractional and overflowing results never replace or round their original exact operands.
/// ´test:integration:quantity-count-fraction-overflow-and-kind-refusals´
/// ´claim:matrix:quantity-count-refusals´ Counts refuse fractional results, signed overflow, capacity overflow, zero divisors and incompatible counted things atomically.
#[test]
fn quantity_count_fraction_overflow_and_kind_refusals() -> TestResult {
    let context = context()?;
    let mut budget = Budget::new(Limits::default())?;
    let three = context.count("three", u128::MAX, &mut budget)?;
    let two = context.count("two", u128::MAX, &mut budget)?;
    let maximum = context.count("maximum", u128::MAX, &mut budget)?;
    let other = context.count("other", u128::MAX, &mut budget)?;
    let zero = context.count("zero", u128::MAX, &mut budget)?;
    assert_eq!(
        maximum.add(&zero, u128::MAX, &mut budget)?.value(),
        i128::MAX
    );
    let spent = budget.usage().work;
    let Err(CountError::Fractional) = three.quotient(2, u128::MAX, &mut budget) else {
        panic!("a fractional count must refuse");
    };
    assert_eq!(budget.usage().work, spent + 1);
    let Err(CountError::Overflow) = maximum.add(&two, u128::MAX, &mut budget) else {
        panic!("integer addition must not wrap");
    };
    let Err(CountError::Overflow) = maximum.scale(2, u128::MAX, &mut budget) else {
        panic!("integer multiplication must not wrap");
    };
    let Err(CountError::Units(fault)) = two.subtract(&three, u128::MAX, &mut budget) else {
        panic!("negative result must obey the nonnegative domain");
    };
    assert_eq!(fault, UnitError::CountValue);
    let Err(CountError::Capacity) = three.add(&two, 4, &mut budget) else {
        panic!("explicit capacity must hold");
    };
    let Err(CountError::ZeroDivisor) = three.quotient(0, u128::MAX, &mut budget) else {
        panic!("zero original divisor must refuse");
    };
    let Err(CountError::Units(fault)) = three.add(&other, u128::MAX, &mut budget) else {
        panic!("different counted things must refuse");
    };
    assert_eq!(fault, UnitError::KindMismatch);
    assert_eq!(
        [three.value(), two.value(), maximum.value()],
        [3, 2, i128::MAX]
    );
    Ok(())
}

/// A ratio is an explicit exact continuous result, never an implicit integer or floating cast.
/// ´test:integration:quantity-count-ratio-retains-original-divisor´
/// ´claim:matrix:quantity-count-ratio´ Explicit compatible count ratios use existing exact rational arithmetic and retain both original operands and divisor definedness.
#[test]
fn quantity_count_ratio_retains_original_divisor() -> TestResult {
    let context = context()?;
    let mut budget = Budget::new(Limits::default())?;
    let three = context.count("three", u128::MAX, &mut budget)?;
    let two = context.count("two", u128::MAX, &mut budget)?;
    let zero = context.count("zero", u128::MAX, &mut budget)?;
    let other = context.count("other", u128::MAX, &mut budget)?;
    assert!(
        context
            .count_ratio(&zero, &three, &mut budget)?
            .exact()
            .is_zero()
    );
    let ratio = context.count_ratio(&three, &two, &mut budget)?;
    assert_eq!(ratio.descriptor(), &Descriptor::ratio());
    assert_eq!(ratio.exact(), &Rational::from_ratio(3, 2, &mut budget)?);
    assert_eq!(
        ratio.operands().each_ref().map(CountQuantity::value),
        [3, 2]
    );
    let Err(CountError::ZeroDivisor) = context.count_ratio(&zero, &zero, &mut budget) else {
        panic!("zero numerator does not erase a zero original divisor");
    };
    let Err(CountError::Units(fault)) = context.count_ratio(&three, &other, &mut budget) else {
        panic!("different counted kinds cannot form this ratio");
    };
    assert_eq!(fault, UnitError::KindMismatch);
    Ok(())
}

/// Continuous quantities and structural metadata cannot enter exact count storage.
/// ´test:integration:quantity-count-storage-separates-continuous-meanings´
/// ´claim:matrix:quantity-count-separation´ Ratios, angles, amounts, continuous expectations, metadata and unknown names refuse exact count admission.
#[test]
fn quantity_count_storage_separates_continuous_meanings() -> TestResult {
    let context = context()?;
    let mut budget = Budget::new(Limits::default())?;
    context.count("three", u128::MAX, &mut budget)?;
    let mut heap = nrz_matrix::Heap::new(nrz_matrix::HeapLimits::default())?;
    for (name, center) in [
        ("ratio", 2.0_f64),
        ("angle", 2.0),
        ("amount", 2.0),
        ("expected", 1.5),
    ] {
        let checked = context.declarations().checked(name)?;
        context.declarations().validate_checked(checked)?;
        let symbols = nrz_bindings::BindingSymbols::declare(
            checked.descriptor(),
            [1, 1],
            [false, false],
            &mut budget,
        )?;
        let quantity = context.import(
            &mut heap,
            &symbols,
            nrz_matrix::QuantityInput::represented(std::slice::from_ref(&center)),
            &mut budget,
        )?;
        assert_eq!(
            quantity.view(&heap)?.region().get(0, 0)?.to_bits(),
            center.to_bits()
        );
    }
    for name in [
        "ratio",
        "angle",
        "amount",
        "expected",
        "metadata",
        "unknown",
        "inputcount",
    ] {
        let Err(CountError::Source(fault)) = context.count(name, u128::MAX, &mut budget) else {
            panic!("a noncount source must refuse");
        };
        assert!(matches!(*fault, nrz_bindings::DecisionError::CountKind));
    }
    Ok(())
}

/// Every exact count operation pins a literal boundary in a fresh declaration store.
/// ´test:integration:quantity-count-work-boundaries´
/// ´claim:matrix:quantity-count-work-boundaries´ Exact count constructors, arithmetic and explicit ratios use measured literal Work boundaries and retain spent Work on atomic refusal.
#[test]
fn quantity_count_work_boundaries() -> TestResult {
    for (action, needed) in [
        (0, 372),
        (1, 1),
        (2, 515),
        (3, 515),
        (4, 515),
        (5, 515),
        (6, 995),
    ] {
        for limit in [needed, needed - 1] {
            let context = context()?;
            let mut setup = Budget::new(Limits::default())?;
            let three = context.count("three", u128::MAX, &mut setup)?;
            let two = context.count("two", u128::MAX, &mut setup)?;
            let mut budget = Budget::new(Limits {
                work: limit,
                ..Limits::default()
            })?;
            let result = count_operation(action, &context, &three, &two, &mut budget);
            if limit == needed {
                result?;
                assert_eq!(budget.usage().work, needed);
            } else {
                let Err(fault) = result else {
                    panic!("one-below count Work must refuse");
                };
                count_work_refusal(&fault, limit, needed)?;
                assert!(budget.usage().work <= limit);
                if needed > 1 {
                    assert!(budget.usage().work > 0);
                }
            }
            assert_eq!([three.value(), two.value()], [3, 2]);
        }
    }
    Ok(())
}

/// Only the measured public operation receives the boundary ledger.
fn count_operation(
    action: u8,
    context: &QuantityContext,
    three: &CountQuantity,
    two: &CountQuantity,
    budget: &mut Budget,
) -> Result<(), CountError> {
    match action {
        0 => {
            context.count("three", u128::MAX, budget)?;
        }
        1 => {
            CountQuantity::capture(three.source().clone(), budget)?;
        }
        2 => {
            three.add(two, u128::MAX, budget)?;
        }
        3 => {
            three.subtract(two, u128::MAX, budget)?;
        }
        4 => {
            three.scale(2, u128::MAX, budget)?;
        }
        5 => {
            two.quotient(2, u128::MAX, budget)?;
        }
        6 => {
            context.count_ratio(three, two, budget)?;
        }
        _ => unreachable!("closed fixture action range"),
    }
    Ok(())
}

/// Preserve the typed Work cause through bindings source or domain wrappers.
fn count_work_refusal(error: &CountError, expected_limit: u64, expected_needed: u64) -> TestResult {
    use nrz_bindings::readings::algebra::{FaultKind, limits::BudgetKind};
    let fault = match error {
        CountError::Resources(fault) | CountError::Units(UnitError::Arithmetic(fault)) => fault,
        CountError::Source(fault) => {
            let nrz_bindings::DecisionError::Algebra(fault) = fault.as_ref() else {
                return Err("expected source Work exhaustion".into());
            };
            fault
        }
        _ => return Err("expected count Work exhaustion".into()),
    };
    let FaultKind::Exhausted {
        budget,
        limit,
        needed,
        ..
    } = &fault.kind
    else {
        return Err("expected exact resource exhaustion".into());
    };
    assert_eq!(*budget, BudgetKind::Work);
    assert_eq!([*limit, *needed], [expected_limit, expected_needed]);
    Ok(())
}

/// Count standard uncertainty is explicitly continuous and does not replace its discrete target.
/// ´test:integration:quantity-count-standard-uncertainty-is-continuous´
/// ´claim:matrix:quantity-count-continuous-uncertainty´ A fractional count standard uncertainty enters floating storage under its independent descriptor while its count remains exact and discrete.
#[test]
fn quantity_count_standard_uncertainty_is_continuous() -> TestResult {
    let context = context()?;
    let mut budget = Budget::new(Limits::default())?;
    let count = context.count("three", u128::MAX, &mut budget)?;
    let standard = nrz_bindings::uncertainty(
        nrz_bindings::UncertaintyMode::Standard,
        count.descriptor(),
        None,
        None,
    )?;
    assert!(standard.kind().count_domain().is_none());
    let symbols =
        nrz_bindings::BindingSymbols::declare(&standard, [1, 1], [false, false], &mut budget)?;
    let mut heap = nrz_matrix::Heap::new(nrz_matrix::HeapLimits::default())?;
    let uncertainty = context.import(
        &mut heap,
        &symbols,
        nrz_matrix::QuantityInput::represented(&[0.5_f64]),
        &mut budget,
    )?;
    assert_eq!(uncertainty.descriptor(), &standard);
    assert_eq!(uncertainty.view(&heap)?.region().get(0, 0)?, 0.5);
    assert_eq!(
        uncertainty.view(&heap)?.coverage(),
        nrz_matrix::Coverage::RepresentedInputs
    );
    let Err(fault) = count.descriptor().compatible_kind(uncertainty.descriptor()) else {
        panic!("uncertainty cannot replace its exact count");
    };
    assert_eq!(fault, UnitError::KindMismatch);
    assert_eq!(count.value(), 3);
    Ok(())
}

/// Immutable integer lineage has explicit depth and retained output ceilings.
/// ´test:integration:quantity-count-lineage-capacity-refusals´
/// ´claim:matrix:quantity-count-lineage-capacity´ Count results refuse exhausted lineage depth and output metadata while keeping original exact values and spent Work.
#[test]
fn quantity_count_lineage_capacity_refusals() -> TestResult {
    let context = context()?;
    let mut setup = Budget::new(Limits::default())?;
    let three = context.count("three", u128::MAX, &mut setup)?;
    let two = context.count("two", u128::MAX, &mut setup)?;
    for depth in [2, 1] {
        let mut budget = Budget::new(Limits {
            depth,
            ..Limits::default()
        })?;
        let result = three.add(&two, u128::MAX, &mut budget);
        if depth == 2 {
            assert_eq!(result?.value(), 5);
        } else {
            let Err(CountError::Resources(fault)) = result else {
                panic!("insufficient lineage depth must refuse");
            };
            assert!(matches!(
                fault.kind,
                nrz_bindings::readings::algebra::FaultKind::Exhausted {
                    budget: nrz_bindings::readings::algebra::limits::BudgetKind::Depth,
                    limit: 1,
                    needed: 2,
                    ..
                }
            ));
        }
        assert!(budget.usage().work > 0);
        assert_eq!([three.value(), two.value()], [3, 2]);
    }
    let mut budget = Budget::new(Limits {
        output_bytes: 1,
        ..Limits::default()
    })?;
    let Err(CountError::Resources(fault)) =
        CountQuantity::capture(three.source().clone(), &mut budget)
    else {
        panic!("count ownership metadata must fit its output ceiling");
    };
    assert!(matches!(
        fault.kind,
        nrz_bindings::readings::algebra::FaultKind::Exhausted {
            budget: nrz_bindings::readings::algebra::limits::BudgetKind::OutputBytes,
            ..
        }
    ));
    assert_eq!(budget.usage().work, 1);
    assert_eq!(three.value(), 3);
    Ok(())
}

/// Signed arithmetic preserves exact integer values and refuses every failed result atomically.
/// ´test:integration:quantity-signed-count-arithmetic´
/// ´claim:matrix:quantity-signed-count-arithmetic´ Signed counts admit negative arithmetic, preserve domain-before-capacity ordering and refuse i128 overflow including minimum divided by minus one.
#[test]
fn quantity_signed_count_arithmetic() -> TestResult {
    let context = context()?;
    let mut budget = Budget::new(Limits::default())?;
    let negative = context.count("negative", 3, &mut budget)?;
    let two = context.count("signed_two", 2, &mut budget)?;
    let minimum = context.count("signed_min", u128::MAX, &mut budget)?;
    let maximum = context.count("signed_max", u128::MAX, &mut budget)?;
    assert_eq!(negative.add(&two, 1, &mut budget)?.value(), -1);
    assert_eq!(negative.subtract(&two, 5, &mut budget)?.value(), -5);
    assert_eq!(negative.scale(-2, 6, &mut budget)?.value(), 6);
    assert_eq!(negative.scale(2, 6, &mut budget)?.value(), -6);
    assert_eq!(negative.quotient(-3, 1, &mut budget)?.value(), 1);
    assert_eq!(
        minimum.quotient(1, 1_u128 << 127, &mut budget)?.value(),
        i128::MIN
    );
    assert_eq!(maximum.scale(1, u128::MAX, &mut budget)?.value(), i128::MAX);
    for result in [
        maximum.add(&two, u128::MAX, &mut budget),
        minimum.subtract(&two, u128::MAX, &mut budget),
        maximum.scale(2, u128::MAX, &mut budget),
        minimum.quotient(-1, u128::MAX, &mut budget),
    ] {
        let Err(CountError::Overflow) = result else {
            panic!("signed overflow must refuse");
        };
    }
    let Err(CountError::Fractional) = negative.quotient(2, u128::MAX, &mut budget) else {
        panic!("negative fractional tally must refuse");
    };
    let Err(CountError::Capacity) = negative.scale(2, 5, &mut budget) else {
        panic!("absolute-magnitude capacity must hold");
    };
    let ordinary = context.count("two", u128::MAX, &mut budget)?;
    let three = context.count("three", u128::MAX, &mut budget)?;
    for result in [
        ordinary.subtract(&three, 0, &mut budget),
        ordinary.scale(i128::MIN, 0, &mut budget),
    ] {
        let Err(CountError::Units(fault)) = result else {
            panic!("mathematical domain must precede machine capacity");
        };
        assert_eq!(fault, UnitError::CountValue);
    }
    assert_eq!(
        [
            negative.value(),
            two.value(),
            minimum.value(),
            maximum.value()
        ],
        [-3, 2, i128::MIN, i128::MAX]
    );
    Ok(())
}

/// Signed source evidence survives the numerical ratio boundary and original divisor checks.
/// ´test:integration:quantity-signed-count-ratios´
/// ´claim:matrix:quantity-signed-count-ratios´ Signed count ratios preserve both original operands and exact negative dyadic or nonrepresentable rational meaning in each scalar format.
#[test]
fn quantity_signed_count_ratios() -> TestResult {
    signed_ratio::<f32>()?;
    signed_ratio::<f64>()
}

/// Independent exact rational containment checks both signs of the original divisor.
fn signed_ratio<P: nrz_scalar::Scalar + 'static>() -> TestResult {
    let context = context()?;
    let mut budget = Budget::new(Limits::default())?;
    let negative = context.count("negative", 3, &mut budget)?;
    let two = context.count("signed_two", 2, &mut budget)?;
    let mut heap = nrz_matrix::Heap::new(nrz_matrix::HeapLimits::default())?;
    for (left, right, numerator, denominator) in
        [(&negative, &two, -3, 2), (&two, &negative, -2, 3)]
    {
        let ratio = context.count_ratio(left, right, &mut budget)?;
        let value = context.materialize_count_ratio::<P>(&mut heap, &ratio, &mut budget)?;
        let view = value.view(&heap)?;
        let center = view.region().get(0, 0)?.to_f64();
        let error = heap
            .companion(view.evaluation().ok_or("numerical error")?)?
            .region()
            .get(0, 0)?
            .to_f64();
        assert!(crate::conversion_oracle::covers(
            center,
            error,
            numerator,
            denominator
        )?);
        if denominator == 2 {
            assert_eq!([center, error], [-1.5, 0.0]);
        }
        assert_eq!(
            value.count_ratio_sources()[0]
                .operands()
                .each_ref()
                .map(CountQuantity::value),
            [left.value(), right.value()]
        );
    }
    Ok(())
}

/// Every signed operation has a fresh-store literal N/N−1 Work boundary.
/// ´test:integration:quantity-signed-count-work-boundaries´
/// ´claim:matrix:quantity-signed-count-work-boundaries´ Signed construction, addition, subtraction, scaling, partition and ratio construction publish only at their measured Work boundaries and preserve prior spending on refusal.
#[test]
fn quantity_signed_count_work_boundaries() -> TestResult {
    for (action, needed) in [(0, 95), (1, 228), (2, 228), (3, 227), (4, 227), (5, 996)] {
        for accepted in [true, false] {
            let context = context()?;
            let mut setup = Budget::new(Limits::default())?;
            let negative = context.count("negative", 3, &mut setup)?;
            let two = context.count("signed_two", 2, &mut setup)?;
            let work = needed - u64::from(!accepted);
            let mut budget = Budget::new(Limits {
                work,
                ..Limits::default()
            })?;
            let result = signed_operation(action, &context, &negative, &two, &mut budget);
            if accepted {
                result?;
                assert_eq!(budget.usage().work, needed);
            } else {
                let Err(fault) = result else {
                    panic!("signed operation must exhaust one below Work");
                };
                count_work_refusal(&fault, work, needed)?;
                assert!(budget.usage().work > 0);
            }
            assert_eq!([negative.value(), two.value()], [-3, 2]);
        }
    }
    Ok(())
}

/// Apply only the measured operation to the caller's independent cumulative ledger.
fn signed_operation(
    action: u8,
    context: &QuantityContext,
    negative: &CountQuantity,
    two: &CountQuantity,
    budget: &mut Budget,
) -> Result<(), CountError> {
    match action {
        0 => {
            context.count("negative", 3, budget)?;
        }
        1 => {
            negative.add(two, 1, budget)?;
        }
        2 => {
            negative.subtract(two, 5, budget)?;
        }
        3 => {
            negative.scale(-2, 6, budget)?;
        }
        4 => {
            negative.quotient(-3, 1, budget)?;
        }
        5 => {
            context.count_ratio(negative, two, budget)?;
        }
        _ => unreachable!("closed signed operation inventory"),
    }
    Ok(())
}
