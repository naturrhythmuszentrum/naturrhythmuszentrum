// SPDX-License-Identifier: AGPL-3.0-only
//! Measure the physical source admission required before a live family trace can exist.

use super::{TestResult, ledger};
use nrz_bindings::readings::algebra::limits::BudgetKind;
use nrz_bindings::readings::algebra::{FaultKind, InvalidCode, Location, Rational};
use nrz_bindings::{
    BaseDimension, Context, DecisionError, DecisionExpression, Declaration, Descriptor, Dimension,
    Evidence, ExponentPolicy, Kind, Measure, Observable, Registry, Role,
};
use std::rc::Rc;

/// A checked physical quotient has one varying duration and one exact period.
fn physical_family() -> TestResult<Rc<nrz_bindings::CheckedDeclarations>> {
    let descriptor = Descriptor::new(
        Kind::Named(
            "duration".into(),
            Box::new(Dimension::base(BaseDimension::Time)),
        ),
        Measure::coherent(),
        Role::Difference,
    )?;
    let mut owner = Context::with_real_symbols(
        Registry::new(ExponentPolicy::Rational),
        nrz_bindings::Limits::default(),
        &["duration", "period", "quotient"],
    )?;
    owner.declare(
        "duration".into(),
        Declaration::Input {
            descriptor: descriptor.clone(),
            nonzero: true,
            evidence: Evidence {
                source: "duration-family".into(),
                assurance: "declared bound".into(),
            },
        },
    )?;
    owner.declare(
        "period".into(),
        Declaration::Constant {
            descriptor,
            value: Rational::from_ratio(1, 4, &mut ledger()?)?,
            evidence: Evidence::default(),
        },
    )?;
    let expression = owner.parse("original family quotient", "duration/period")?;
    owner.declare(
        "quotient".into(),
        Declaration::Derived(Observable {
            descriptor: Descriptor::ratio(),
            expression,
            evidence: Evidence::default(),
        }),
    )?;
    Ok(Rc::new(owner.freeze()?))
}

/// Physical input declarations cannot reach the closed decision association by nominal relabeling.
/// ´test:integration:kernel-physical-family-admission-baseline´
/// ´claim:matrix:kernel-physical-family-admission-baseline´
#[test]
fn kernel_physical_family_admission_baseline() -> TestResult {
    for work in [1268, 1267] {
        let owner = physical_family()?;
        let before = owner.cas()?.counts();
        let mut budget = nrz_bindings::Budget::new(nrz_bindings::Limits {
            work,
            ..nrz_bindings::Limits::default()
        })?;
        let limits = budget.limits().clone();
        budget.work(17, Location::new("prior physical family"))?;
        let Err(DecisionError::Algebra(fault)) =
            DecisionExpression::declared(Rc::clone(&owner), "quotient", &mut budget)
        else {
            panic!("open physical source must refuse closed decision admission");
        };
        if work == 1268 {
            assert_eq!(
                fault.kind,
                FaultKind::Invalid(InvalidCode::UnsupportedFragment)
            );
            assert_eq!(fault.location, Location::new("exact constant expression"));
            assert_eq!(budget.usage().work, work);
        } else {
            let FaultKind::Exhausted {
                budget: kind,
                limit,
                needed,
                ..
            } = fault.kind
            else {
                panic!("one-below physical source admission must exhaust Work");
            };
            assert_eq!(kind, BudgetKind::Work);
            assert_eq!(limit, work);
            assert_eq!(needed, 1268);
            assert!(budget.usage().work >= 17);
        }
        assert_eq!(owner.cas()?.counts(), before);
        assert_eq!(budget.limits(), &limits);
    }
    Ok(())
}
