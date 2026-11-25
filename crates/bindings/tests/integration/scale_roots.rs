// SPDX-License-Identifier: AGPL-3.0-only
//! Exact root reuse preserves content, canonical evidence, policy and atomic publication.

use nrz_bindings::{
    Budget, Descriptor, ExponentPolicy, Kind, Limits, Measure, Registry, Role, Scale, UnitError,
};
use nrz_cas::{ExactConstant, Fault, FaultKind, Rational, limits::BudgetKind};

/// Public exact descriptor checks have no numerical-storage prerequisite.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Each baseline ledger starts with the unchanged default policy.
fn ledger() -> Result<Budget, Fault> {
    Budget::new(Limits::default())
}

/// A dimensionless coordinate can carry any admitted positive exact scale.
fn descriptor(scale: i64, name: &str) -> TestResult<Descriptor> {
    Ok(Descriptor::new(
        Kind::Ratio,
        Measure::new(
            name,
            Scale::Exact(Rational::from_ratio(scale, 1, &mut ledger()?)?),
            Rational::zero(),
        )?,
        Role::Difference,
    )?)
}

/// Compare literal canonical bytes or the immutable exact graph, never a debug rendering.
fn bytes(value: &ExactConstant) -> TestResult<Vec<u8>> {
    if let Some(number) = value.as_rational() {
        Ok(number.canonical_bytes(&mut ledger()?)?)
    } else {
        let (graph, _) = value.graph().ok_or("missing exact graph")?;
        Ok(graph.canonical_snapshot(&mut ledger()?)?)
    }
}

/// A unit shortcut and every cached result preserve the original exact authority.
/// ´test:integration:scale-root-reuse-canonical-evidence´
/// ´claim:bindings:scale-root-reuse-canonical-evidence´ Unit, rational-square and irrational scale roots preserve cold exact values, positive witnesses and canonical bytes on content-keyed reuse.
#[test]
fn scale_root_reuse_canonical_evidence() -> TestResult {
    for scale in [1, 4, 2] {
        let registry = Registry::new(ExponentPolicy::Rational);
        let source = descriptor(scale, "first")?;
        let equal = descriptor(scale, "other")?;
        let original = source.measure().scale_witness().sqrt(&mut ledger()?)?;
        let cold = registry.coordinate_square_root(&source, &mut ledger()?)?;
        let warm = registry.coordinate_square_root(&equal, &mut ledger()?)?;
        for result in [&cold, &warm] {
            assert_eq!(result.measure().scale(), &original);
            assert_eq!(result.measure().scale().positive(), original.positive());
            assert_eq!(bytes(result.measure().scale())?, bytes(&original)?);
            assert_eq!(result.kind(), &Kind::Ratio);
            assert_eq!(result.role(), Role::Difference);
            assert_eq!(result.measure().offset(), &Rational::zero());
        }
        assert_eq!(warm.measure().name(), "sqrt(other)");
    }
    Ok(())
}

/// Literal boundaries cover preparation, content reuse, scale misses and policy misses.
/// ´test:integration:scale-root-reuse-work-boundaries´
/// ´claim:bindings:scale-root-reuse-work-boundaries´ First, repeated, changed-scale and changed-policy roots preserve literal N and N-minus-one Work boundaries with unchanged exact evidence and atomic refusal.
#[test]
fn scale_root_reuse_work_boundaries() -> TestResult {
    // The irrational warm scale-two miss adds 12 retained kind + 6 local kind
    // + 18 identity + 24 vertex charges - 6 unjournaled inverses = 56_916 Work over unchanged operation Work 56_862.
    for (primed, scale, changed, required) in [
        (false, 4, false, 17_472),
        (true, 4, false, 332),
        (true, 9, false, 17_546),
        (true, 4, true, 17_543),
        (false, 1, false, 176),
        (true, 2, false, 56_916),
    ] {
        for admitted in [false, true] {
            let registry = Registry::new(ExponentPolicy::Rational);
            if primed {
                registry.coordinate_square_root(&descriptor(4, "scale")?, &mut ledger()?)?;
            }
            work_boundary(&registry, scale, changed, required, admitted)?;
        }
    }
    Ok(())
}

/// The final reporting-name charge distinguishes N-minus-one without probing for a boundary.
fn work_boundary(
    registry: &Registry,
    scale: i64,
    changed: bool,
    required: u64,
    admitted: bool,
) -> TestResult {
    let source = descriptor(scale, "scale")?;
    let mut limits = Limits {
        work: required - u64::from(!admitted),
        ..Limits::default()
    };
    if changed {
        limits.name_bytes -= 1;
    }
    let mut budget = Budget::new(limits.clone())?;
    let result = registry.coordinate_square_root(&source, &mut budget);
    if admitted {
        let result = result?;
        let original = source.measure().scale_witness().sqrt(&mut ledger()?)?;
        assert_eq!(bytes(result.measure().scale())?, bytes(&original)?);
        assert_eq!(budget.usage().work, required);
    } else {
        let Err(UnitError::Arithmetic(fault)) = result else {
            panic!("one-below exact root Work must refuse");
        };
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            }
        ));
        assert_eq!(budget.usage().work, required - 11);
        limits.work = required;
        let mut retry = Budget::new(limits)?;
        registry.coordinate_square_root(&source, &mut retry)?;
        assert_eq!(retry.usage().work, required);
    }
    Ok(())
}

/// A changed restrictive policy cannot borrow a checked result from a more permissive call.
/// ´test:integration:scale-root-reuse-policy-refusal´
/// ´claim:bindings:scale-root-reuse-policy-refusal´ A restrictive changed policy misses the root cache and refuses at the same construction limit as a cold registry without replacing its retained judgment.
#[test]
fn scale_root_reuse_policy_refusal() -> TestResult {
    let source = descriptor(4, "scale")?;
    let registry = Registry::new(ExponentPolicy::Rational);
    registry.coordinate_square_root(&source, &mut ledger()?)?;
    for owner in [&registry, &Registry::new(ExponentPolicy::Rational)] {
        let mut budget = Budget::new(Limits {
            staged_vertices: 1,
            ..Limits::default()
        })?;
        let Err(UnitError::Arithmetic(fault)) = owner.coordinate_square_root(&source, &mut budget)
        else {
            panic!("changed construction policy must refuse the scale root");
        };
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::StagedVertices,
                ..
            }
        ));
    }
    let mut warm = Budget::new(Limits {
        work: 332,
        ..Limits::default()
    })?;
    registry.coordinate_square_root(&source, &mut warm)?;
    assert_eq!(warm.usage().work, 332);
    Ok(())
}

/// Failed consumers leave the previous slot intact and cannot publish a new preparation.
/// ´test:integration:scale-root-reuse-consumer-refusal´
/// ´claim:bindings:scale-root-reuse-consumer-refusal´ Refusing after exact scale preparation preserves spent Work and leaves the next same-content operation cold.
#[test]
fn scale_root_reuse_consumer_refusal() -> TestResult {
    let source = descriptor(4, "scale")?;
    let registry = Registry::new(ExponentPolicy::Rational);
    let mut refused = ledger()?;
    let result: Result<(), UnitError> =
        registry.with_coordinate_square_root(&source, &mut refused, |_, _| {
            Err(UnitError::InvalidScale)
        });
    let Err(fault) = result else {
        panic!("the explicit consumer refusal must survive preparation");
    };
    assert_eq!(fault, UnitError::InvalidScale);
    let mut retry = ledger()?;
    registry.coordinate_square_root(&source, &mut retry)?;
    assert_eq!(retry.usage().work, refused.usage().work);
    Ok(())
}

/// Cache ownership must not narrow the existing exact-store threading contract.
/// ´test:integration:scale-root-reuse-send-sync´
/// ´claim:bindings:scale-root-reuse-send-sync´ The bindings registry and CAS remain Send and Sync with retained scale-root judgments.
#[test]
fn scale_root_reuse_send_sync() {
    send_sync::<Registry>();
    send_sync::<nrz_cas::Cas>();
}

/// Enforce the public threading contract at compile time.
const fn send_sync<T: Send + Sync>() {}

/// Independently allocated exact graphs meet by canonical content, not owner identity.
/// ´test:integration:scale-root-reuse-expression-content´
/// ´claim:bindings:scale-root-reuse-expression-content´ Independently constructed pi-squared scale witnesses share the checked root while preserving exact pi and its canonical graph bytes.
#[test]
fn scale_root_reuse_expression_content() -> TestResult {
    let registry = Registry::new(ExponentPolicy::Rational);
    let mut costs = Vec::new();
    let pi = ExactConstant::named(nrz_cas::syntax::Constant::Pi, &mut ledger()?)?;
    for name in ["first", "other"] {
        let independent = ExactConstant::named(nrz_cas::syntax::Constant::Pi, &mut ledger()?)?;
        let scale = independent.pow(2, &mut ledger()?)?;
        let source = Descriptor::new(
            Kind::Ratio,
            Measure::new(name, Scale::Expression(scale), Rational::zero())?,
            Role::Difference,
        )?;
        let original = source.measure().scale_witness().sqrt(&mut ledger()?)?;
        let mut budget = ledger()?;
        let result = registry.coordinate_square_root(&source, &mut budget)?;
        assert_eq!(result.measure().scale(), &pi);
        assert_eq!(bytes(result.measure().scale())?, bytes(&original)?);
        costs.push(budget.usage().work);
    }
    assert!(costs[1] < costs[0]);
    Ok(())
}

/// Alternating scales hit without reordering insertion age; a fifth key evicts the oldest.
/// ´test:integration:scale-root-reuse-alternation-boundaries´
/// ´claim:bindings:scale-root-reuse-alternation-boundaries´ Alternating 4 and 9 scale roots reuse both preparations, and deterministic FIFO eviction preserves literal N and N-minus-one Work at every call.
#[test]
fn scale_root_reuse_alternation_boundaries() -> TestResult {
    let sequence = [
        (4, 17_472),
        (9, 17_546),
        (4, 332),
        (9, 406),
        (16, 17_812),
        (25, 17_886),
        (36, 17_960),
        (4, 17_768),
    ];
    for (position, (scale, required)) in sequence.iter().enumerate() {
        for admitted in [false, true] {
            let registry = Registry::new(ExponentPolicy::Rational);
            for (prime, _) in sequence.iter().take(position) {
                registry.coordinate_square_root(&descriptor(*prime, "scale")?, &mut ledger()?)?;
            }
            work_boundary(&registry, *scale, false, *required, admitted)?;
        }
    }
    Ok(())
}

/// A nested successful owner operation changes the generation observed by an outer preparation.
/// ´test:integration:scale-root-reuse-stale-publication´
/// ´claim:bindings:scale-root-reuse-stale-publication´ A completed nested root remains cached when an earlier pending preparation observes a changed generation and discards its insertion.
#[test]
fn scale_root_reuse_stale_publication() -> TestResult {
    let registry = Registry::new(ExponentPolicy::Rational);
    let first = descriptor(4, "scale")?;
    let second = descriptor(9, "scale")?;
    let result: Result<Descriptor, UnitError> =
        registry.with_coordinate_square_root(&first, &mut ledger()?, |first, _| {
            registry.coordinate_square_root(&second, &mut Budget::new(Limits::default())?)?;
            Ok(first)
        });
    assert_eq!(
        result?.measure().rational_scale()?,
        &Rational::from_ratio(2, 1, &mut ledger()?)?
    );
    let mut warm = ledger()?;
    registry.coordinate_square_root(&second, &mut warm)?;
    assert_eq!(warm.usage().work, 332);
    Ok(())
}
