// SPDX-License-Identifier: AGPL-3.0-only
//! Frozen environments preserve canonical records and isolate fresh graph identities.

use super::FrozenEnvironment;
use crate::domain::HeaderDraft;
use crate::limits::{Budget, Limits};
use crate::solving::{ScalarSpaceData, SolveDraft};
use crate::syntax::{
    AtomicSort, Declaration, Draft, Environment, ExpressionDraft, Extent, Field, Operator, Sort,
    SymbolRelation,
};
use crate::{Cas, Reference};

/// Common fallible test result.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Fixed scalar-shaped quantity sort.
fn sort() -> Sort {
    Sort::Value(AtomicSort::Matrix {
        field: Field::Real,
        rows: Extent::Literal(1),
        columns: Extent::Literal(1),
    })
}

/// Three used names and an optional unused declaration suffix.
fn environment(count: usize) -> Environment {
    Environment {
        declarations: (0..count)
            .map(|index| {
                let key = if index < 3 {
                    format!("q{:020}_{index}", 0)
                } else {
                    format!("z{index:020}_0")
                };
                Declaration {
                    display_name: key.clone(),
                    key,
                    sort: sort(),
                }
            })
            .collect(),
        ..Environment::default()
    }
}

/// One expression retaining the exact local header.
fn expression(operator: Operator, sort: Sort, operands: Vec<Reference>) -> Draft {
    Draft::Expression(ExpressionDraft {
        space: None,
        operator,
        sort,
        header: Reference::Local(0),
        scope: Vec::new(),
        binding: None,
        operands,
    })
}

/// Fixed three symbols and their two ordered companion relations.
fn drafts(header: HeaderDraft) -> Vec<Draft> {
    let mut result = vec![Draft::Header(header)];
    for index in 0..3 {
        result.push(expression(
            Operator::Free(format!("q{:020}_{index}", 0)),
            sort(),
            Vec::new(),
        ));
    }
    for (index, relation) in [
        (2, SymbolRelation::DeterministicBound),
        (3, SymbolRelation::NumericalError),
    ] {
        result.push(expression(
            Operator::SymbolRelation(relation),
            Sort::Value(AtomicSort::Boolean),
            vec![Reference::Local(index), Reference::Local(1)],
        ));
    }
    result
}

/// Freeze and admission retain literal boundaries even with thirty unused declarations.
/// ´test:unit:frozen-environment-work-boundaries´
/// ´claim:cas:frozen-environment-work´ Three and thirty-three declarations freeze at 3046 and 67240 Work and admit fresh graphs at 7915 and 13237, preserving canonical bytes.
#[test]
fn frozen_environment_work_boundaries() -> TestResult {
    for (count, freeze_work, admission_work, complete_work) in
        [(3, 3_046, 7_915, 10_961), (33, 67_240, 13_237, 80_477)]
    {
        let original = environment(count);
        for limit in [freeze_work, freeze_work - 1] {
            let mut budget = allowance(limit)?;
            let result = FrozenEnvironment::freeze(&original, &mut budget);
            if limit == freeze_work {
                result?;
                assert_eq!(budget.usage().work, freeze_work);
            } else {
                let Err(fault) = result else {
                    panic!("one-below freeze must refuse");
                };
                exhausted(&fault, limit, freeze_work);
            }
            assert!(budget.usage().work > 0);
            assert!(budget.usage().work <= limit);
        }
        let frozen = FrozenEnvironment::freeze(
            &original,
            &mut crate::budget::Budget::new(Limits::default())?,
        )?;
        for limit in [admission_work, admission_work - 1] {
            let mut cas = Cas::new(Limits::default())?;
            let before = cas.counts();
            let mut budget = allowance(limit)?;
            let result = cas.admit(&drafts(HeaderDraft::Frozen(frozen.clone())), &mut budget);
            if limit == admission_work {
                let actual = result?;
                assert_eq!(budget.usage().work, admission_work);
                let mut ordinary = Cas::new(Limits::default())?;
                let expected = ordinary.admit(
                    &drafts(HeaderDraft::Core(original.clone())),
                    &mut crate::budget::Budget::new(Limits::default())?,
                )?;
                assert_ne!(actual[1], expected[1]);
                assert_eq!(
                    cas.canonical_snapshot(&mut crate::budget::Budget::new(Limits::default())?)?,
                    ordinary
                        .canonical_snapshot(&mut crate::budget::Budget::new(Limits::default())?)?,
                );
                let actual_id = cas.checked(&actual[0], &mut budget)?;
                let header = cas.state.header(actual_id, &mut budget)?;
                assert!(std::sync::Arc::ptr_eq(&header.environment.0, &frozen.0));
            } else {
                let Err(fault) = result else {
                    panic!("one-below fresh admission must refuse");
                };
                exhausted(&fault, limit, admission_work);
                assert_eq!(cas.counts(), before);
            }
            assert!(budget.usage().work > 0);
            assert!(budget.usage().work <= limit);
        }
        for limit in [complete_work, complete_work - 1] {
            let mut cas = Cas::new(Limits::default())?;
            let before = cas.counts();
            let mut budget = allowance(limit)?;
            let frozen = FrozenEnvironment::freeze(&original, &mut budget)?;
            let result = cas.admit(&drafts(HeaderDraft::Frozen(frozen)), &mut budget);
            if limit == complete_work {
                result?;
                assert_eq!(budget.usage().work, complete_work);
            } else {
                let Err(fault) = result else {
                    panic!("one-below complete cold admission must refuse");
                };
                exhausted(&fault, limit, complete_work);
                assert_eq!(cas.counts(), before);
            }
            assert!(budget.usage().work > 0);
            assert!(budget.usage().work <= limit);
        }
    }
    Ok(())
}

/// A new ledger changes only its explicit Work allowance.
fn allowance(work: u64) -> Result<Budget, crate::Fault> {
    crate::budget::Budget::new(Limits {
        work,
        ..Limits::default()
    })
}

/// Only the named Work exhaustion is expected.
fn exhausted(fault: &crate::Fault, limit: u64, needed: u64) {
    let crate::FaultKind::Exhausted {
        budget,
        limit: actual_limit,
        needed: actual_needed,
        ..
    } = &fault.kind
    else {
        panic!("expected Work exhaustion");
    };
    assert_eq!(*budget, crate::limits::BudgetKind::Work);
    assert_eq!([*actual_limit, *actual_needed], [limit, needed]);
}

/// Segmented and ordinary keys select the same graph record in either insertion order.
/// ´test:unit:frozen-header-keys-preserve-mixed-interning´
/// ´claim:cas:frozen-header-mixed-interning´ Ordinary and shared header keys intern to identical existing records in either insertion order and with either scalar-space allocation order without changing canonical graph bytes.
#[test]
fn frozen_header_keys_preserve_mixed_interning() -> TestResult {
    let original = environment(3);
    let frozen = FrozenEnvironment::freeze(
        &original,
        &mut crate::budget::Budget::new(Limits::default())?,
    )?;
    for (shared_first, complex_first) in
        [(false, false), (true, false), (false, true), (true, true)]
    {
        let mut cas = Cas::new(Limits::default())?;
        if complex_first {
            let space = Draft::Solve(SolveDraft::ScalarSpace(ScalarSpaceData {
                specification: "scalar.complex".into(),
                version: 1,
            }));
            cas.admit(
                std::slice::from_ref(&space),
                &mut crate::budget::Budget::new(Limits::default())?,
            )?;
        }
        let headers = [
            HeaderDraft::Core(original.clone()),
            HeaderDraft::Frozen(frozen.clone()),
        ];
        let first = usize::from(shared_first);
        let expected = cas.admit(
            &drafts(headers[first].clone()),
            &mut crate::budget::Budget::new(Limits::default())?,
        )?;
        let before = cas.canonical_snapshot(&mut crate::budget::Budget::new(Limits::default())?)?;
        let counts = cas.counts();
        let actual = cas.admit(
            &drafts(headers[1 - first].clone()),
            &mut crate::budget::Budget::new(Limits::default())?,
        )?;
        assert_eq!(actual, expected);
        assert_eq!(cas.counts(), counts);
        assert_eq!(
            cas.canonical_snapshot(&mut crate::budget::Budget::new(Limits::default())?)?,
            before
        );
    }
    Ok(())
}

/// Replacing the complete declaration signature cannot select a header from its prior revision.
/// ´test:unit:frozen-header-replaced-environment-refuses-stale-drafts´
/// ´claim:cas:frozen-header-replaced-environment´ Both ordinary and shared admission refuse stale symbol sorts after environment replacement atomically, while matching replacement drafts retain identical canonical bytes.
#[test]
fn frozen_header_replaced_environment_refuses_stale_drafts() -> TestResult {
    let original = environment(3);
    let mut replacement = original.clone();
    let changed_sort = Sort::Value(AtomicSort::Matrix {
        field: Field::Real,
        rows: Extent::Literal(2),
        columns: Extent::Literal(1),
    });
    for declaration in &mut replacement.declarations {
        declaration.sort = changed_sort.clone();
    }
    let frozen = FrozenEnvironment::freeze(
        &replacement,
        &mut crate::budget::Budget::new(Limits::default())?,
    )?;
    let mut snapshots = Vec::new();
    for header in [HeaderDraft::Core(replacement), HeaderDraft::Frozen(frozen)] {
        let mut cas = Cas::new(Limits::default())?;
        let prior = if matches!(header, HeaderDraft::Frozen(_)) {
            HeaderDraft::Frozen(FrozenEnvironment::freeze(
                &original,
                &mut crate::budget::Budget::new(Limits::default())?,
            )?)
        } else {
            HeaderDraft::Core(original.clone())
        };
        let old = cas.admit(
            &drafts(prior),
            &mut crate::budget::Budget::new(Limits::default())?,
        )?;
        let before = cas.canonical_snapshot(&mut crate::budget::Budget::new(Limits::default())?)?;
        let counts = cas.counts();
        let mut budget = crate::budget::Budget::new(Limits::default())?;
        let Err(fault) = cas.admit(&drafts(header.clone()), &mut budget) else {
            panic!("replacement environment must refuse stale Free sorts");
        };
        assert_eq!(
            fault.kind,
            crate::FaultKind::Invalid(crate::InvalidCode::SymbolSortMismatch)
        );
        assert!(budget.usage().work > 0);
        assert_eq!(cas.counts(), counts);
        assert_eq!(
            cas.canonical_snapshot(&mut crate::budget::Budget::new(Limits::default())?)?,
            before
        );
        let mut matching = drafts(header);
        for draft in &mut matching {
            if let Draft::Expression(value) = draft
                && matches!(value.operator, Operator::Free(_))
            {
                value.sort = changed_sort.clone();
            }
        }
        let fresh = cas.admit(
            &matching,
            &mut crate::budget::Budget::new(Limits::default())?,
        )?;
        assert_ne!(fresh[0], old[0]);
        assert_ne!(fresh[1], old[1]);
        snapshots
            .push(cas.canonical_snapshot(&mut crate::budget::Budget::new(Limits::default())?)?);
    }
    assert_eq!(snapshots[0], snapshots[1]);
    Ok(())
}
