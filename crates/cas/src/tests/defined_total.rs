// SPDX-License-Identifier: AGPL-3.0-only
//! The structural route leaves the existing Product partition boundary intact.

use crate::testing::common::{TestResult, default_budget};
use crate::testing::series_residual::{Fixture, Term};
use crate::{FaultKind, InvalidCode};

#[test]
fn defined_total_does_not_partition_product() -> TestResult {
    let mut f = Fixture::totality(false)?;
    let (function, _) = f.term(&Term::X, 5)?;
    for produced in [false, true] {
        if produced {
            let witness = f
                .cas
                .defined_total(&function, &f.context, &mut default_budget()?)?;
            assert!(witness.is_unconditional());
        }
        assert_eq!(
            crate::region::partition_at(
                &f.cas.state,
                &[f.region.logical_id()],
                None,
                &mut default_budget()?
            )
            .err()
            .ok_or("Product partition accepted")?
            .kind,
            FaultKind::Invalid(InvalidCode::UnsupportedFragment)
        );
    }
    Ok(())
}

/// Sealing alone pays for all ten descriptors and the complete byte stream on this ledger.
#[test]
fn defined_total_sealing_boundaries() -> TestResult {
    use crate::Location;
    use crate::limits::{Budget, BudgetKind, Limits};
    use crate::testing::defined_total_minimal as inventory;
    let work = inventory::seal_work() + 37;
    let staged = 16 * inventory::PAYLOADS.iter().sum::<u64>()
        + 256 * inventory::REFERENCES.iter().sum::<u64>()
        + 19;
    for (kind, needed) in [
        (BudgetKind::Work, work),
        (BudgetKind::StagedPayloadBytes, staged),
    ] {
        for accepted in [true, false] {
            let (mut f, function) = inventory::fixture(false)?;
            let witness = f
                .cas
                .defined_total(&function, &f.context, &mut default_budget()?)?;
            let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
            let counts = f.cas.counts();
            let amount = needed - u64::from(!accepted);
            let mut limits = Limits::default();
            match kind {
                BudgetKind::Work => limits.work = amount,
                BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = amount,
                _ => unreachable!("fixture kinds"),
            }
            let mut budget = Budget::new(limits.clone())?;
            budget.work(37, Location::new("prior sealing Work"))?;
            let result = budget.with_retained_payload(
                19,
                Location::new("prior sealing payload"),
                |budget| f.cas.seal_scoped(witness.proof(), budget),
            );
            assert_eq!(result.is_ok(), accepted, "{kind:?} at {amount}");
            if let Err(fault) = result {
                assert!(
                    matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind)
                );
                if kind == BudgetKind::Work {
                    assert_eq!(budget.usage().work, work - 4);
                }
            } else {
                assert_eq!(budget.usage().work, work);
            }
            assert_eq!(budget.limits(), &limits);
            assert_eq!(f.cas.counts(), counts);
            assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
        }
    }
    Ok(())
}

/// Isolate the native checker so sealing cannot hide ownership of the traversal stack.
#[test]
fn defined_total_walk_stack_boundary() -> TestResult {
    use crate::limits::{Budget, BudgetKind, Limits};
    use crate::syntax::{Branch, Operator};
    use crate::{Exhaustion, Location};
    let mut f = crate::testing::defined_total::Fixture::new(Branch::StrictReal, false, 0, false)?;
    let x = f.term(Operator::Free("x".into()), &[])?;
    let negated = f.term(Operator::Neg, &[x])?;
    let function = f.term(Operator::Neg, &[negated])?;
    let draft = f.draft(&function)?;
    let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
    let counts = f.cas.counts();
    // Three active syntax levels retain an identity and a next-child index each.
    // The largest metadata is x: operator (1+4+1), Real sort (3), empty scope (4),
    // absent binder (1). Neg metadata is only nine bytes. Prior ownership adds 19.
    let slot = std::mem::size_of::<(crate::LogicalId, usize)>() as u64;
    let metadata = (1 + 4 + 1) + 3 + 4 + 1;
    let staged = 19 + 3 * slot + metadata;
    for accepted in [true, false] {
        let limits = Limits {
            staged_payload_bytes: staged - u64::from(!accepted),
            ..Limits::default()
        };
        let mut budget = Budget::new(limits.clone())?;
        budget.work(37, Location::new("prior walk Work"))?;
        let result =
            budget.with_retained_payload(19, Location::new("prior walk payload"), |budget| {
                crate::defined_total::check(&f.cas, &draft, &[], budget)
            });
        assert_eq!(result.is_ok(), accepted);
        assert_eq!(budget.limits(), &limits);
        if let Err(fault) = result {
            assert_eq!(
                fault.kind,
                FaultKind::Exhausted {
                    reason: Exhaustion::Limit,
                    budget: BudgetKind::StagedPayloadBytes,
                    limit: metadata - 1,
                    needed: metadata,
                }
            );
            // Five claim checks, frame shape and checks, stack push, two Neg visits
            // and descents, then the leaf visit and thirteen successful encoded bytes.
            let prefix = 5 + (2 * 9 + 5) + slot + 2 * (1 + 2 * 9 + 8 + 3 + slot) + 1 + metadata - 1;
            assert_eq!(budget.usage().work, 37 + prefix);
            assert_eq!(budget.usage().condition_steps, 3);
        }
        assert_eq!(f.cas.counts(), counts);
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
    }
    Ok(())
}
