// SPDX-License-Identifier: AGPL-3.0-only
//! Compatibility proposals count local slots and reserve their complete output.

use crate::budget::{Budget, BudgetKind, Limits};
use crate::proof_model::{ApplicationDraft, Certificate, Statement};
use crate::solving::radicals::construction::Construction;
use crate::testing::common::TestResult;

fn fixture() -> TestResult<Construction> {
    let (mut source, _) = crate::testing::solve_blind::symbolic(&["p"])?;
    let (other, _) = crate::testing::solve_blind::symbolic(&["q"])?;
    let one = source.integer(1, false)?;
    let predicate = source.equation(&one, &one)?;
    let fact = Statement::PredicateFact {
        predicate: predicate.clone(),
        region: source.region.clone(),
    };
    let foreign = Statement::PredicateFact {
        predicate,
        region: other.region,
    };
    let subset = Statement::Subset {
        subset: source.region.clone(),
        superset: source.region.clone(),
    };
    let steps = [
        (other.context, fact.clone()),
        (source.context.clone(), subset),
        (source.context.clone(), fact.clone()),
        (source.context.clone(), foreign),
        (source.context.clone(), fact),
    ]
    .into_iter()
    .map(|(context, conclusion)| ApplicationDraft {
        rule: crate::rule::Primitive::RegionConstruct,
        context,
        conclusion,
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    })
    .collect();
    Ok(Construction {
        header: source.header,
        context: source.context,
        region: source.region,
        steps,
    })
}

/// Foreign contexts do not consume local slots; other regions and nonpredicate statements are omitted.
/// ´test:unit:root-compatibility-slots-and-boundaries´
#[test]
fn root_compatibility_slots_and_boundaries() -> TestResult {
    let proposal = fixture()?;
    let mut exact = Budget::new(Limits {
        work: 7,
        staged_payload_bytes: 8,
        ..Limits::default()
    })?;
    assert_eq!(super::compatibility(&proposal, &mut exact)?, vec![1, 3]);
    assert_eq!(exact.usage().work, 7);
    for (kind, limits) in [
        (
            BudgetKind::Work,
            Limits {
                work: 6,
                ..Limits::default()
            },
        ),
        (
            BudgetKind::StagedPayloadBytes,
            Limits {
                staged_payload_bytes: 7,
                ..Limits::default()
            },
        ),
    ] {
        let fault = super::compatibility(&proposal, &mut Budget::new(limits)?)
            .err()
            .ok_or("short compatibility budget")?;
        assert!(matches!(fault.kind, crate::FaultKind::Exhausted { budget, .. } if budget == kind));
    }
    let mut cumulative = Budget::new(Limits {
        work: 7,
        ..Limits::default()
    })?;
    cumulative.work(1, crate::Location::new("prior compatibility work"))?;
    assert!(super::compatibility(&proposal, &mut cumulative).is_err());
    Ok(())
}

/// No premises need no Work or output payload and produce an empty compatibility vector.
/// ´test:unit:root-compatibility-empty-boundary´
#[test]
fn root_compatibility_empty_boundary() -> TestResult {
    let mut proposal = fixture()?;
    proposal.steps.clear();
    let mut budget = Budget::new(Limits {
        work: 0,
        staged_payload_bytes: 0,
        ..Limits::default()
    })?;
    assert_eq!(
        super::compatibility(&proposal, &mut budget)?,
        [] as [u32; 0]
    );
    assert_eq!(budget.usage().work, 0);
    Ok(())
}
