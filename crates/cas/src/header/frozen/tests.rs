// SPDX-License-Identifier: AGPL-3.0-only
//! Frozen environments preserve canonical records and isolate fresh graph identities.

use super::FrozenEnvironment;
use crate::domain::HeaderDraft;
use crate::limits::{Budget, Limits};
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
/// ´claim:cas:frozen-environment-work´ Three and thirty-three declarations freeze at 3046 and 67240 Work and admit fresh graphs at 8100 and 15672, preserving canonical bytes.
#[test]
fn frozen_environment_work_boundaries() -> TestResult {
    for (count, freeze_work, admission_work, complete_work) in
        [(3, 3_046, 8_100, 11_146), (33, 67_240, 15_672, 82_912)]
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
        let frozen = FrozenEnvironment::freeze(&original, &mut Budget::new(Limits::default())?)?;
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
                    &mut Budget::new(Limits::default())?,
                )?;
                assert_ne!(actual[1], expected[1]);
                assert_eq!(
                    cas.canonical_snapshot(&mut Budget::new(Limits::default())?)?,
                    ordinary.canonical_snapshot(&mut Budget::new(Limits::default())?)?,
                );
                let header = cas.state.header(cas.checked(&actual[0])?)?;
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
    Budget::new(Limits {
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
