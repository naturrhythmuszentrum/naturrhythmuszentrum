// SPDX-License-Identifier: AGPL-3.0-only
//! Original checked positive-power fixture built through public CAS syntax admission.

use super::TestResult;
use crate::{ExecutionAccount, Heap, HeapLimits, RetainedDyadicPower};
use nrz_bindings::DyadicPowerPreparation;
use nrz_bindings::{
    Budget, Limits,
    readings::algebra::{
        Cas, Handle, Rational, Reference,
        domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr},
        enclosure::RefinementPolicy,
        syntax::{
            AtomicSort, Declaration, Draft, Environment, ExpressionDraft, Field, Operator, Sort,
        },
    },
};
use nrz_scalar::{ArithmeticContext, DyadicPowerLimits};

/// Exact and managed budgets are fixed before each lifecycle begins.
pub(super) fn limits() -> Limits {
    Limits {
        work: 2_000_000_000,
        arithmetic_steps: 50_000_000,
        output_bytes: 4_194_304,
        staged_payload_bytes: 64_000_000,
        witness_bytes: 4_194_304,
        ..Limits::default()
    }
}

/// Give only normal-threshold preparation and exact oracles their larger CAS capacities.
pub(super) fn threshold_limits() -> Limits {
    Limits {
        component_bits: 8_192,
        intermediate_bits: 16_384,
        integer_exponent: 4096,
        literal_bytes: 4096,
        ..limits()
    }
}

/// The one-round policy permits a checked width-limit bound to establish inclusion.
pub(super) fn policy() -> RefinementPolicy {
    RefinementPolicy {
        version: 1,
        precision: 16,
        grid_ceiling: 256,
        rounds: 1,
        reduction_ceiling: 8,
        term_ceiling: 64,
        checking_work: 0,
    }
}

/// The scalar leaf receives its own explicit finite inventories.
pub(super) const fn scalar_limits() -> DyadicPowerLimits {
    DyadicPowerLimits {
        exponent_bits: 63,
        root_steps: 63,
        product_steps: 128,
    }
}

/// Construct one original graph with a free real coordinate and literal rational exponent.
pub(super) fn original(n: i64, d: u64) -> TestResult<(Cas, Handle, Handle)> {
    let mut budget = Budget::new(limits())?;
    original_rational(Rational::from_ratio(n, d, &mut budget)?)
}

/// Construct the normal-threshold graph under its isolated exact capacities.
pub(super) fn threshold_original(n: i64, d: u64) -> TestResult<(Cas, Handle, Handle)> {
    let mut budget = Budget::new(threshold_limits())?;
    original_rational_with_limits(Rational::from_ratio(n, d, &mut budget)?, threshold_limits())
}

/// Admit the same original shape with an exact exponent beyond carrier-sized denominators.
pub(super) fn original_rational(exponent: Rational) -> TestResult<(Cas, Handle, Handle)> {
    original_rational_with_limits(exponent, limits())
}

/// Admit one exact exponent graph under its caller-selected fixture capacities.
fn original_rational_with_limits(
    exponent: Rational,
    limits: Limits,
) -> TestResult<(Cas, Handle, Handle)> {
    let mut budget = Budget::new(limits.clone())?;
    let mut cas = Cas::new(limits)?;
    let sort = Sort::Value(AtomicSort::Scalar(Field::Real));
    let mut environment = Environment::default();
    environment.declarations.push(Declaration {
        key: "x".into(),
        display_name: "x".into(),
        sort: sort.clone(),
    });
    let drafts = [
        Draft::Header(HeaderDraft::Core(environment)),
        Draft::Region(RegionDraft {
            header: Reference::Local(0),
            scope: vec![],
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::Ambient,
        }),
        Draft::Context(ContextDraft::Root {
            header: Reference::Local(0),
            admissible: Reference::Local(1),
            assumptions: vec![],
        }),
    ];
    let [header, _, context]: [Handle; 3] = cas
        .admit(&drafts, &mut budget)?
        .try_into()
        .map_err(|_| "header fixture")?;
    let make = |operator: Operator, operands: Vec<Handle>| {
        Draft::Expression(ExpressionDraft {
            space: None,
            operator,
            sort: sort.clone(),
            header: Reference::Existing(header.clone()),
            scope: vec![],
            binding: None,
            operands: operands.into_iter().map(Reference::Existing).collect(),
        })
    };
    let base = cas
        .admit(&[make(Operator::Free("x".into()), vec![])], &mut budget)?
        .remove(0);
    let exponent = cas
        .admit(&[make(Operator::Rational(exponent), vec![])], &mut budget)?
        .remove(0);
    let target = cas
        .admit(
            &[make(Operator::PositivePower, vec![base, exponent])],
            &mut budget,
        )?
        .remove(0);
    Ok((cas, target, context))
}

/// Create a true retained owner and an independent cumulative execution account.
pub(super) fn prepare() -> TestResult<(Heap, ExecutionAccount, RetainedDyadicPower)> {
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = ExecutionAccount::new(Budget::new(limits())?)?;
    let law = prepare_on(&mut heap, &mut account, false)?;
    Ok((heap, account, law))
}

/// Prepare on a supplied ledger, optionally seeding a checked warm CAS registration.
pub(super) fn prepare_on(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    warm: bool,
) -> TestResult<RetainedDyadicPower> {
    let (mut cas, target, context) = original(1, 2)?;
    if warm {
        let mut work = 0;
        let seed = DyadicPowerPreparation::prepare(
            &mut cas,
            &target,
            &context,
            [1.0, 2.0],
            policy(),
            &mut account.ledger,
            &mut |_| false,
            &mut work,
        )?;
        assert!(work > 0 && seed.retained_bytes() > 0);
    }
    let law = heap
        .prepare_retained_dyadic_power(
            cas,
            &target,
            &context,
            [1.0, 2.0],
            policy(),
            scalar_limits(),
            &ArithmeticContext::new()?,
            &mut |_| false,
            account,
        )
        .map_err(|(error, _)| error)?;
    Ok(law)
}
