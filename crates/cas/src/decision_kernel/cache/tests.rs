// SPDX-License-Identifier: AGPL-3.0-only
//! Exact retained meanings, complete keys and atomic store-local lifecycle.

use super::*;
use crate::budget::Limits;
use crate::decision::{EnclosureOrigin, RationalEnclosure, ScalarKernelLaw, ScalarKernelRecord};
use crate::domain_model::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
use crate::graph::{Draft, ExpressionDraft, Handle, Reference};
use crate::model::{AtomicSort, Environment, Field, Operator, Sort};
use crate::number::Rational;
use crate::testing::common::TestResult;
use crate::{FaultKind, InvalidCode};

/// Inspect the sole checked inventory through its authenticated graph node.
fn inventory(cas: &Cas) -> Result<&ScalarInventory, Fault> {
    let id = cas
        .state
        .scalar_judgments
        .values()
        .next()
        .ok_or_else(|| Fault::invalid(InvalidCode::Reference, AT))?;
    cas.scalar_inventory(*id)
}

/// Ordinary ledger without larger limits or defaults.
fn ledger() -> Result<Budget, Fault> {
    Budget::new(Limits::default())
}

/// Closed real syntax sharing the fixture's immutable header.
fn expression(operator: Operator, operands: Vec<Reference>) -> Draft {
    Draft::Expression(ExpressionDraft {
        space: None,
        operator,
        sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
        header: Reference::Local(0),
        scope: Vec::new(),
        binding: None,
        operands,
    })
}

/// The original reciprocal of two fifths and a complete outward scalar trace.
fn fixture() -> TestResult<(Cas, RationalEnclosure)> {
    let mut budget = ledger()?;
    let value = Rational::from_ratio(2, 5, &mut budget)?;
    let drafts = [
        Draft::Header(HeaderDraft::Core(Environment::default())),
        expression(Operator::Rational(value.clone()), Vec::new()),
        expression(Operator::Rational(Rational::one()), Vec::new()),
        expression(
            Operator::Div,
            vec![Reference::Local(2), Reference::Local(1)],
        ),
        Draft::Region(RegionDraft {
            header: Reference::Local(0),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::Ambient,
        }),
        Draft::Context(ContextDraft::Root {
            header: Reference::Local(0),
            admissible: Reference::Local(4),
            assumptions: Vec::new(),
        }),
    ];
    let mut cas = Cas::new(Limits::default())?;
    let handles = cas.admit(&drafts, &mut budget)?;
    let [_, _, _, target, family, context]: [Handle; 6] =
        handles.try_into().map_err(|_| "fixture")?;
    let lower = Rational::one();
    let upper = Rational::from_ratio(4, 1, &mut budget)?;
    let certificate = ScalarKernelCertificate {
        version: 1,
        expression: target.clone(),
        context: context.clone(),
        input_family: family.clone(),
        records: vec![
            ScalarKernelRecord {
                law: ScalarKernelLaw::Rational(value),
                lower: Rational::from_ratio(1, 4, &mut budget)?,
                upper: Rational::from_ratio(3, 4, &mut budget)?,
            },
            ScalarKernelRecord {
                law: ScalarKernelLaw::Reciprocal { input: 0 },
                lower: lower.clone(),
                upper: upper.clone(),
            },
        ],
    };
    Ok((
        cas,
        RationalEnclosure {
            target,
            context,
            input_family: family,
            lower,
            upper,
            origin: EnclosureOrigin::ScalarKernel(Box::new(certificate)),
        },
    ))
}

/// The original checked meanings move into one graph node and are shared on repetition.
/// ´test:unit:scalar-record-cache-reuses-meanings´
/// ´claim:cas:scalar-record-cache-reuses-meanings´
#[test]
fn scalar_record_cache_reuses_meanings() -> TestResult {
    let (mut cas, contract) = fixture()?;
    let snapshot = cas.counts();
    let mut first = ledger()?;
    cas.check_rational_enclosure(&contract, &mut first)?;
    let checked = Arc::clone(&inventory(&cas)?.values);
    let committed = cas.canonical_snapshot(&mut ledger()?)?;
    let mut second = ledger()?;
    cas.check_rational_enclosure(&contract, &mut second)?;
    let inventory = inventory(&cas)?;
    assert_eq!(cas.state.scalar_judgments.len(), 1);
    assert_eq!(checked.len(), 2);
    assert!(Arc::ptr_eq(&checked, &inventory.values));
    assert_eq!(cas.counts().vertices, snapshot.vertices + 1);
    assert_eq!(cas.counts().edges, snapshot.edges + 4);
    assert_eq!(first.usage().work, 22_560);
    assert_eq!(second.usage().work, 22_016);
    assert_eq!(cas.retained_judgment_bytes(), 1_424);
    assert_eq!(inventory.key.len(), 104);
    assert_eq!(first.checked_records.bytes, 1_488);
    assert_eq!(second.checked_records.bytes, 1_488);
    assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, committed);
    Ok(())
}

/// Equal printed component sizes cannot substitute a different exact input under a retained key.
/// ´test:unit:scalar-record-cache-content-forgery´
/// ´claim:cas:scalar-record-cache-content-forgery´
#[test]
fn scalar_record_cache_content_forgery() -> TestResult {
    let (mut cas, mut contract) = fixture()?;
    cas.check_rational_enclosure(&contract, &mut ledger()?)?;
    let EnclosureOrigin::ScalarKernel(certificate) = &mut contract.origin else {
        return Err("scalar fixture".into());
    };
    let ScalarKernelLaw::Rational(value) = &mut certificate.records[0].law else {
        return Err("rational input".into());
    };
    let changed = Rational::from_ratio(3, 5, &mut ledger()?)?;
    assert_eq!(value.component_bits(), changed.component_bits());
    assert_eq!(value.is_negative(), changed.is_negative());
    assert_ne!(value, &changed);
    *value = changed;
    let changed_key = key(&cas, certificate, &mut ledger()?)?;
    assert_ne!(inventory(&cas)?.key.as_ref(), changed_key.as_slice());
    let Err(fault) = cas.check_rational_enclosure(&contract, &mut ledger()?) else {
        panic!("mutated exact record must refuse at original-target matching");
    };
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(fault.location, Location::new("scalar kernel coverage"));
    assert_eq!(cas.state.scalar_judgments.len(), 1);
    Ok(())
}

/// A different store and a rollback generation each require independent reconstruction.
/// ´test:unit:scalar-record-cache-owner-and-rollback´
/// ´claim:cas:scalar-record-cache-owner-and-rollback´
#[test]
fn scalar_record_cache_owner_and_rollback() -> TestResult {
    let (mut cas, contract) = fixture()?;
    let mut first = ledger()?;
    cas.check_rational_enclosure(&contract, &mut first)?;
    let mut fresh = cas.full_copy_for_test();
    let mut independent = ledger()?;
    fresh.check_rational_enclosure(&contract, &mut independent)?;
    assert_eq!(independent.usage().work, first.usage().work);
    let snapshot = cas.canonical_snapshot(&mut ledger()?)?;
    let Err(fault) = cas.transaction(&mut ledger()?, |cas, budget| {
        cas.check_rational_enclosure(&contract, budget)?;
        Err::<(), _>(Fault::invalid(InvalidCode::Reference, AT))
    }) else {
        panic!("deliberate transaction refusal");
    };
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Reference));
    assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, snapshot);
    assert_eq!(cas.state.scalar_judgments.len(), 1);
    let mut after = ledger()?;
    cas.check_rational_enclosure(&contract, &mut after)?;
    assert_eq!(after.usage().work, first.usage().work);
    cas.memo_epoch = u64::MAX;
    for _ in 0..2 {
        let mut saturated = ledger()?;
        cas.check_rational_enclosure(&contract, &mut saturated)?;
        assert_eq!(saturated.usage().work, first.usage().work);
        assert_eq!(cas.state.scalar_judgments.len(), 1);
    }
    Ok(())
}

/// Cold and warm N/N−1 boundaries preserve prior Work and never publish a failed receipt.
/// ´test:unit:scalar-record-cache-work-boundaries´
/// ´claim:cas:scalar-record-cache-work-boundaries´
#[test]
fn scalar_record_cache_work_boundaries() -> TestResult {
    for (warm, needed) in [(false, 22_577), (true, 22_033)] {
        for work in [needed, needed - 1] {
            let (mut cas, contract) = fixture()?;
            if warm {
                cas.check_rational_enclosure(&contract, &mut ledger()?)?;
            }
            let before = cas.canonical_snapshot(&mut ledger()?)?;
            let limits = Limits {
                work,
                ..Limits::default()
            };
            let mut budget = Budget::new(limits.clone())?;
            budget.work(17, AT)?;
            let result = cas.check_rational_enclosure(&contract, &mut budget);
            if work == needed {
                let receipt = result?;
                assert_eq!(receipt.retained_bytes(), 1_720);
                assert_eq!(budget.usage().work, needed);
                assert_eq!(cas.state.scalar_judgments.len(), 1);
            } else {
                let Err(fault) = result else {
                    panic!("receipt copy must refuse below its complete Work boundary");
                };
                assert!(matches!(fault.kind, FaultKind::Exhausted {
                    budget: BudgetKind::Work, limit, needed: actual, ..
                } if limit == work && actual == needed));
                assert_eq!(fault.location, Location::new("exact rational enclosure"));
                assert_eq!(budget.usage().work, needed - 1_720);
                assert_eq!(cas.state.scalar_judgments.len(), usize::from(warm));
                assert_eq!(budget.checked_records.bytes, 0);
            }
            assert_eq!(budget.limits(), &limits);
            if warm || work != needed {
                assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, before);
            }
        }
    }
    Ok(())
}

/// Retained judgments and registration metadata remain reserved beside subsequent caller payload.
/// ´test:unit:scalar-record-cache-storage-boundaries´
/// ´claim:cas:scalar-record-cache-storage-boundaries´
#[test]
fn scalar_record_cache_storage_boundaries() -> TestResult {
    let (mut cas, contract) = fixture()?;
    cas.check_rational_enclosure(&contract, &mut ledger()?)?;
    for bytes in [1_489, 1_488] {
        let limits = Limits {
            staged_payload_bytes: bytes,
            ..Limits::default()
        };
        let mut budget = Budget::new(limits.clone())?;
        let result = retained(&cas, &mut budget, |budget| {
            budget.with_retained_payload(1, AT, |budget| {
                budget.check(BudgetKind::StagedPayloadBytes, 0, AT)
            })
        });
        if bytes == 1_489 {
            result?;
        } else {
            let Err(fault) = result else {
                panic!("retained exact records must reserve the caller's extra byte");
            };
            assert!(matches!(
                fault.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::StagedPayloadBytes,
                    limit: 1_488,
                    needed: 1_489,
                    ..
                }
            ));
        }
        assert_eq!(budget.usage().work, 0);
        assert_eq!(budget.limits(), &limits);
        assert_eq!(cas.retained_judgment_bytes(), 1_424);
        assert_eq!(cas.state.scalar_judgments.len(), 1);
    }
    Ok(())
}

/// One retained inventory pays exactly its complete 104-byte comparison at N and refuses at N−1.
/// ´test:unit:scalar-record-cache-lookup-work´
/// ´claim:cas:scalar-record-cache-lookup-work´
#[test]
fn scalar_record_cache_lookup_work() -> TestResult {
    let (mut cas, contract) = fixture()?;
    cas.check_rational_enclosure(&contract, &mut ledger()?)?;
    let key = inventory(&cas)?.key.clone();
    for work in [121, 120] {
        let mut budget = Budget::new(Limits {
            work,
            ..Limits::default()
        })?;
        budget.work(17, AT)?;
        let result = lookup(&cas, &key, &mut budget);
        if work == 121 {
            assert!(result?.is_some());
            assert_eq!(budget.usage().work, 121);
        } else {
            let Err(fault) = result else {
                panic!("complete key comparison must refuse one below its Work charge");
            };
            assert!(matches!(
                fault.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::Work,
                    limit: 120,
                    needed: 121,
                    ..
                }
            ));
            assert_eq!(budget.usage().work, 17);
        }
    }
    Ok(())
}

/// Concurrent callers share one reconstruction while the store remains Send and Sync.
/// ´test:unit:scalar-record-cache-concurrent-store´
/// ´claim:cas:scalar-record-cache-concurrent-store´
#[test]
fn scalar_record_cache_concurrent_store() -> TestResult {
    let (cas, contract) = fixture()?;
    let _: &dyn Send = &cas;
    let _: &dyn Sync = &cas;
    let cas = std::sync::Mutex::new(cas);
    let mut costs = std::thread::scope(|scope| -> TestResult<Vec<u64>> {
        let jobs = (0..2)
            .map(|_| {
                scope.spawn(|| {
                    let mut budget = ledger()?;
                    cas.lock()
                        .map_err(|_| Fault::invalid(InvalidCode::Reference, AT))?
                        .check_rational_enclosure(&contract, &mut budget)?;
                    Ok::<_, Fault>(budget.usage().work)
                })
            })
            .collect::<Vec<_>>();
        jobs.into_iter()
            .map(|job| {
                let result = job.join().map_err(|_| "concurrent receipt panic")?;
                Ok(result?)
            })
            .collect()
    })?;
    costs.sort_unstable();
    assert_eq!(costs, [22_016, 22_560]);
    assert_eq!(
        cas.lock()
            .map_err(|_| "graph owner poisoned")?
            .state
            .scalar_judgments
            .len(),
        1
    );
    Ok(())
}

/// A fresh cold N−1 refusal leaves every graph byte unchanged and releases the staged suffix.
/// ´test:unit:scalar-graph-cold-receipt-boundary´
/// ´claim:cas:scalar-graph-cold-receipt-boundary´
#[test]
fn scalar_graph_cold_receipt_boundary() -> TestResult {
    for work in [22_560, 22_559] {
        let (mut cas, contract) = fixture()?;
        let before = cas.canonical_snapshot(&mut ledger()?)?;
        let mut budget = Budget::new(Limits {
            work,
            ..Limits::default()
        })?;
        let result = cas.check_rational_enclosure(&contract, &mut budget);
        if work == 22_560 {
            result?;
            assert_eq!(budget.usage().work, work);
            assert_eq!(cas.state.scalar_judgments.len(), 1);
        } else {
            let Err(fault) = result else {
                panic!("one-below cold receipt must refuse its final copy");
            };
            assert_eq!(fault.location, Location::new("exact rational enclosure"));
            assert!(matches!(
                fault.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::Work,
                    limit: 22_559,
                    needed: 22_560,
                    ..
                }
            ));
            assert_eq!(budget.usage().work, 20_840);
            assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, before);
            assert_eq!(cas.retained_judgment_count(), 0);
            assert_eq!(budget.checked_records.bytes, 0);
        }
        let cache = records(&cas)?;
        assert_eq!(cache.pending.len(), 0);
        assert_eq!(cache.pending.capacity(), 0);
    }
    Ok(())
}

/// A consumer refusal after the scalar receipt copy discards its staged node without refunding Work.
/// ´test:unit:scalar-graph-enclosing-receipt-refusal´
/// ´claim:cas:scalar-graph-enclosing-receipt-refusal´
#[test]
fn scalar_graph_enclosing_receipt_refusal() -> TestResult {
    let (mut cas, contract) = fixture()?;
    let before = cas.canonical_snapshot(&mut ledger()?)?;
    let mut budget = ledger()?;
    let result: Result<(), Fault> = cas.with_enclosure_attempt(&mut budget, |attempt, budget| {
        attempt.check_rational_enclosure(&contract, budget)?;
        Err(Fault::invalid(
            InvalidCode::EndpointMismatch,
            Location::new("enclosing receipt"),
        ))
    });
    let Err(fault) = result else {
        panic!("enclosing consumer must refuse after the inner receipt copy");
    };
    assert_eq!(fault.location, Location::new("enclosing receipt"));
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(budget.usage().work, 22_560);
    assert_eq!(budget.checked_records.bytes, 0);
    assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, before);
    let cache = records(&cas)?;
    assert_eq!(cache.pending.len(), 0);
    assert_eq!(cache.pending.capacity(), 0);
    Ok(())
}

/// A structural index cannot authorize a wrong graph node or a missing checked-evidence edge.
/// ´test:unit:scalar-graph-evidence-authentication´
/// ´claim:cas:scalar-graph-evidence-authentication´
#[test]
fn scalar_graph_evidence_authentication() -> TestResult {
    for missing_edge in [false, true] {
        let (mut cas, contract) = fixture()?;
        cas.check_rational_enclosure(&contract, &mut ledger()?)?;
        let inventory = inventory(&cas)?;
        let key = Arc::clone(&inventory.key);
        let original = inventory.scope[0];
        let id = cas.state.scalar_judgments[&key];
        if missing_edge {
            let edge = cas
                .state
                .graph
                .find_edge(cas.state.indices[&original], cas.state.indices[&id])
                .ok_or("missing original checked edge")?;
            cas.state.graph.remove_edge(edge);
        } else {
            cas.state.scalar_judgments.insert(key, original);
        }
        let before = cas.canonical_snapshot(&mut ledger()?)?;
        let Err(fault) = cas.check_rational_enclosure(&contract, &mut ledger()?) else {
            panic!("a forged scalar graph citation must refuse");
        };
        assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Reference));
        assert_eq!(fault.location, AT);
        assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    Ok(())
}

/// Nested proof refusal discards a cold staged inventory while preserving all earlier graph bytes.
/// ´test:unit:scalar-graph-nested-refusal´
/// ´claim:cas:scalar-graph-nested-refusal´
#[test]
fn scalar_graph_nested_refusal() -> TestResult {
    let (mut cas, contract) = fixture()?;
    let before = cas.canonical_snapshot(&mut ledger()?)?;
    let mut budget = ledger()?;
    let Err(fault) = cas.transaction(&mut budget, |cas, budget| {
        cas.transaction(budget, |cas, budget| {
            cas.check_rational_enclosure(&contract, budget)
        })?;
        Err::<(), _>(Fault::invalid(InvalidCode::Reference, AT))
    }) else {
        panic!("the enclosing proof transaction must refuse");
    };
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Reference));
    assert_eq!(fault.location, AT);
    assert_eq!(budget.usage().work, 23_072);
    assert_eq!(budget.checked_records.bytes, 0);
    assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, before);
    assert_eq!(cas.retained_judgment_count(), 0);
    assert_eq!(cas.state.scalar_judgments.len(), 0);
    let cache = records(&cas)?;
    assert_eq!(cache.pending.len(), 0);
    assert_eq!(cache.pending.capacity(), 0);
    assert_eq!(cache.depth, 0);
    drop(cache);
    Ok(())
}
