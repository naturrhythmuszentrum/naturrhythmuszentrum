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
fn context() -> TestResult<QuantityContext> {
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
            "18446744073709551615",
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
    Ok(QuantityContext::new(
        Rc::new(owner.freeze()?),
        ShapeLimits::default(),
        ExecutionLimits::default(),
    ))
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
    let three = context.count("three", u64::MAX, &mut budget)?;
    let two = context.count("two", u64::MAX, &mut budget)?;
    let sum = three.add(&two, u64::MAX, &mut budget)?;
    assert_eq!(sum.value(), 5);
    assert_eq!(three.subtract(&two, u64::MAX, &mut budget)?.value(), 1);
    assert_eq!(three.scale(2, u64::MAX, &mut budget)?.value(), 6);
    assert_eq!(two.quotient(2, u64::MAX, &mut budget)?.value(), 1);
    assert_eq!(sum.descriptor(), three.descriptor());
    let CountHistory::Add(left, right) = sum.history() else {
        panic!("sum retains both exact operands");
    };
    assert_eq!([left.value(), right.value()], [3, 2]);
    assert!(
        matches!(sum.source().evidence(), nrz_bindings::CountEvidence::Declared { name, .. } if name == "three")
    );
    let bounded_three = context.count("bounded_three", u64::MAX, &mut budget)?;
    let bounded_two = context.count("bounded_two", u64::MAX, &mut budget)?;
    assert_eq!(
        bounded_three
            .subtract(&bounded_two, u64::MAX, &mut budget)?
            .value(),
        1
    );
    let Err(CountError::Units(fault)) = bounded_three.add(&bounded_two, u64::MAX, &mut budget)
    else {
        panic!("the actual result must remain in its declared range");
    };
    assert_eq!(fault, UnitError::CountValue);
    Ok(())
}

/// Fractional and overflowing results never replace or round their original exact operands.
/// ´test:integration:quantity-count-fraction-overflow-and-kind-refusals´
/// ´claim:matrix:quantity-count-refusals´ Counts refuse fractional results, unsigned overflow, capacity overflow, zero divisors and incompatible counted things atomically.
#[test]
fn quantity_count_fraction_overflow_and_kind_refusals() -> TestResult {
    let context = context()?;
    let mut budget = Budget::new(Limits::default())?;
    let three = context.count("three", u64::MAX, &mut budget)?;
    let two = context.count("two", u64::MAX, &mut budget)?;
    let maximum = context.count("maximum", u64::MAX, &mut budget)?;
    let other = context.count("other", u64::MAX, &mut budget)?;
    let zero = context.count("zero", u64::MAX, &mut budget)?;
    assert_eq!(maximum.add(&zero, u64::MAX, &mut budget)?.value(), u64::MAX);
    let spent = budget.usage().work;
    let Err(CountError::Fractional) = three.quotient(2, u64::MAX, &mut budget) else {
        panic!("a fractional count must refuse");
    };
    assert_eq!(budget.usage().work, spent + 1);
    let Err(CountError::Overflow) = maximum.add(&two, u64::MAX, &mut budget) else {
        panic!("integer addition must not wrap");
    };
    let Err(CountError::Overflow) = maximum.scale(2, u64::MAX, &mut budget) else {
        panic!("integer multiplication must not wrap");
    };
    let Err(CountError::Overflow) = two.subtract(&three, u64::MAX, &mut budget) else {
        panic!("negative result cannot enter unsigned storage");
    };
    let Err(CountError::Capacity) = three.add(&two, 4, &mut budget) else {
        panic!("explicit capacity must hold");
    };
    let Err(CountError::ZeroDivisor) = three.quotient(0, u64::MAX, &mut budget) else {
        panic!("zero original divisor must refuse");
    };
    let Err(CountError::Units(fault)) = three.add(&other, u64::MAX, &mut budget) else {
        panic!("different counted things must refuse");
    };
    assert_eq!(fault, UnitError::KindMismatch);
    assert_eq!(
        [three.value(), two.value(), maximum.value()],
        [3, 2, u64::MAX]
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
    let three = context.count("three", u64::MAX, &mut budget)?;
    let two = context.count("two", u64::MAX, &mut budget)?;
    let zero = context.count("zero", u64::MAX, &mut budget)?;
    let other = context.count("other", u64::MAX, &mut budget)?;
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
    context.count("three", u64::MAX, &mut budget)?;
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
        let Err(CountError::Source(fault)) = context.count(name, u64::MAX, &mut budget) else {
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
            let three = context.count("three", u64::MAX, &mut setup)?;
            let two = context.count("two", u64::MAX, &mut setup)?;
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
            context.count("three", u64::MAX, budget)?;
        }
        1 => {
            CountQuantity::capture(three.source().clone(), budget)?;
        }
        2 => {
            three.add(two, u64::MAX, budget)?;
        }
        3 => {
            three.subtract(two, u64::MAX, budget)?;
        }
        4 => {
            three.scale(2, u64::MAX, budget)?;
        }
        5 => {
            two.quotient(2, u64::MAX, budget)?;
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
    let count = context.count("three", u64::MAX, &mut budget)?;
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
    let three = context.count("three", u64::MAX, &mut setup)?;
    let two = context.count("two", u64::MAX, &mut setup)?;
    for depth in [2, 1] {
        let mut budget = Budget::new(Limits {
            depth,
            ..Limits::default()
        })?;
        let result = three.add(&two, u64::MAX, &mut budget);
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
