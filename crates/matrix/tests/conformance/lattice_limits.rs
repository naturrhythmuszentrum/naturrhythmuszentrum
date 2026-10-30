// SPDX-License-Identifier: AGPL-3.0-only
//! Live lattice receiver owners have independent literal resource boundaries.

use super::{decision_fixture::ledger, lattice_live, lattice_selection};
use crate::binding_testing::TestResult;
use nrz_bindings::readings::algebra::{
    FaultKind, Location, lattice::RationalBoxEnclosure, limits::BudgetKind,
};
use nrz_bindings::{Budget, LatticeInputs, Limits, SelectedLattice};
use nrz_matrix::{
    DecisionSnapshotError, FrozenLatticeEnclosure, Heap, HeapLimits, LiveDecisionError,
    PreparedLatticeDecision, ValueBinding,
};
use nrz_scalar::ArithmeticContext;

/// One already checked mathematical question and its current native inputs.
struct Ready {
    /// Original numerical authority.
    heap: Heap,
    /// Complete retained physical association.
    association: LatticeInputs,
    /// Every designated current value and companion binding.
    inputs: Vec<ValueBinding<f64>>,
    /// Immutable full mathematical and physical tuple.
    selected: SelectedLattice,
    /// Independently certified original target intervals.
    contract: RationalBoxEnclosure,
}

impl Ready {
    /// Keep mathematical prerequisite production outside the measured live owner.
    fn new(kind: u8) -> TestResult<Self> {
        let source = lattice_selection::prepared(kind, false)?;
        let association = source.case.associate_inputs(&mut ledger()?)?;
        let contract = lattice_selection::original_box(&source)?.clone();
        let mut heap = Heap::new(HeapLimits::default())?;
        let inputs = lattice_live::inputs(&mut heap, &association, kind)?;
        let selected = lattice_selection::select(source)?;
        Ok(Self {
            heap,
            association,
            inputs,
            selected,
            contract,
        })
    }

    /// Spend only containment on the caller's measured ledger.
    fn enclose(&self, budget: &mut Budget) -> TestResult<FrozenLatticeEnclosure<f64>> {
        let frozen =
            self.heap
                .freeze_lattice_inputs(&self.association, &self.inputs, &mut ledger()?)?;
        Ok(self.heap.enclose_lattice_inputs(
            frozen,
            self.selected.cas(),
            &self.contract,
            &ArithmeticContext::new()?,
            lattice_live::literals(),
            budget,
        )?)
    }

    /// Prepare an independent fresh publication candidate using the unchanged checked theorem.
    fn prepared(&self) -> TestResult<PreparedLatticeDecision<f64>> {
        Ok(self.heap.prepare_lattice_decision(
            self.enclose(&mut ledger()?)?,
            self.selected.clone(),
            &mut ledger()?,
        )?)
    }
}

/// Every test starts with caller Work already spent.
fn budget(limits: Limits) -> TestResult<Budget> {
    let mut budget = Budget::new(limits)?;
    budget.work(17, Location::new("prior lattice owner"))?;
    Ok(budget)
}

/// Refuse specifically at the requested resource checker without inspecting a success value.
fn exhausted(error: &LiveDecisionError, expected: BudgetKind) -> TestResult {
    let (LiveDecisionError::Algebra(fault)
    | LiveDecisionError::Snapshot(DecisionSnapshotError::Resources(fault))) = error
    else {
        return Err("live resource refusal must retain its resource checker".into());
    };
    let FaultKind::Exhausted { budget: actual, .. } = fault.kind else {
        return Err("live owner must report resource exhaustion".into());
    };
    assert_eq!(actual, expected);
    Ok(())
}

/// Complete original-box checking and numerical containment have a literal Work boundary.
/// ´test:integration:lattice-enclosure-work-boundaries´
/// ´claim:matrix:lattice-enclosure-work-boundaries´
#[test]
fn lattice_enclosure_work_boundaries() -> TestResult {
    for (kind, limits) in [(0, [59_481, 59_480]), (1, [21_299, 21_298])] {
        let ready = Ready::new(kind)?;
        let graph = ready.selected.cas().counts();
        let records = ready.heap.provenance_usage();
        for (position, work) in limits.into_iter().enumerate() {
            let frozen = ready.heap.freeze_lattice_inputs(
                &ready.association,
                &ready.inputs,
                &mut ledger()?,
            )?;
            let mut budget = budget(Limits {
                work,
                ..Limits::default()
            })?;
            let result = ready.heap.enclose_lattice_inputs(
                frozen,
                ready.selected.cas(),
                &ready.contract,
                &ArithmeticContext::new()?,
                lattice_live::literals(),
                &mut budget,
            );
            if position == 0 {
                assert_eq!(
                    result?.retained_payload_bytes(),
                    if kind == 0 { 5848 } else { 8480 }
                );
                assert_eq!(budget.usage().work, work);
            } else {
                let Err(fault) = result else {
                    panic!("one-below containment Work must refuse the whole vector");
                };
                exhausted(&fault, BudgetKind::Work)?;
            }
            assert!(budget.usage().work > 17);
            assert!(budget.usage().work <= work);
            assert_eq!(ready.selected.cas().counts(), graph);
            assert_eq!(ready.heap.provenance_usage(), records);
        }
    }
    Ok(())
}

/// Matching a selected theorem to the complete checked original box has its own Work boundary.
/// ´test:integration:lattice-preparation-work-boundaries´
/// ´claim:matrix:lattice-preparation-work-boundaries´
#[test]
fn lattice_preparation_work_boundaries() -> TestResult {
    for (kind, limits) in [(0, [1711, 1710]), (1, [2039, 2038])] {
        let ready = Ready::new(kind)?;
        let graph = ready.selected.cas().counts();
        let records = ready.heap.provenance_usage();
        for (position, work) in limits.into_iter().enumerate() {
            let enclosed = ready.enclose(&mut ledger()?)?;
            let mut budget = budget(Limits {
                work,
                ..Limits::default()
            })?;
            let result =
                ready
                    .heap
                    .prepare_lattice_decision(enclosed, ready.selected.clone(), &mut budget);
            if position == 0 {
                let prepared = result?;
                ready
                    .heap
                    .publish_lattice_decision(prepared, &mut ledger()?)?;
                assert_eq!(budget.usage().work, work);
            } else {
                let Err(fault) = result else {
                    panic!("one-below selected-box matching Work must refuse");
                };
                exhausted(&fault, BudgetKind::Work)?;
            }
            assert!(budget.usage().work > 17);
            assert!(budget.usage().work <= work);
            assert_eq!(ready.selected.cas().counts(), graph);
            assert_eq!(ready.heap.provenance_usage(), records);
        }
    }
    Ok(())
}

/// Publication has independent Work, staged-payload and returned-storage refusal boundaries.
/// ´test:integration:lattice-publication-boundaries´
/// ´claim:matrix:lattice-publication-boundaries´
#[test]
fn lattice_publication_boundaries() -> TestResult {
    for (kind, work_limits, byte_limits) in [
        (0, [242, 241], [41_628, 41_627]),
        (1, [306, 305], [65_790, 65_789]),
    ] {
        let ready = Ready::new(kind)?;
        let graph = ready.selected.cas().counts();
        let records = ready.heap.provenance_usage();
        for (work, staged_payload_bytes, output_bytes, refusal) in [
            (work_limits[0], byte_limits[0] + 13, byte_limits[0], None),
            (
                work_limits[1],
                byte_limits[0] + 13,
                byte_limits[0],
                Some(BudgetKind::Work),
            ),
            (
                work_limits[0],
                byte_limits[1] + 13,
                byte_limits[0],
                Some(BudgetKind::StagedPayloadBytes),
            ),
            (
                work_limits[0],
                byte_limits[0] + 13,
                byte_limits[1],
                Some(BudgetKind::OutputBytes),
            ),
        ] {
            let prepared = ready.prepared()?;
            let mut budget = budget(Limits {
                work,
                staged_payload_bytes,
                output_bytes,
                ..Limits::default()
            })?;
            let result = budget.with_retained_payload(
                13,
                Location::new("retained lattice caller"),
                |budget| ready.heap.publish_lattice_decision(prepared, budget),
            );
            if let Some(expected) = refusal {
                let Err(fault) = result else {
                    panic!("one-below publication resources must return no attachment");
                };
                exhausted(&fault, expected)?;
            } else {
                result?.current(&ready.heap, &mut ledger()?)?;
                assert_eq!(budget.usage().work, work);
            }
            assert!(budget.usage().work >= 17);
            assert!(budget.usage().work <= work);
            assert_eq!(ready.selected.cas().counts(), graph);
            assert_eq!(ready.heap.provenance_usage(), records);
        }
    }
    Ok(())
}

/// Distinct optional consumers retain their own Work charges.
#[derive(Clone, Copy)]
enum Access {
    /// Recheck the native lease only.
    Current,
    /// Recheck the requested physical association as well.
    Requested,
    /// Create a dependent physical representative.
    Representative,
    /// Materialize one separately capacity-checked executable index.
    Index,
}

/// Current access and each dependent constructor refuse exactly one below their own owner cost.
/// ´test:integration:lattice-access-work-boundaries´
/// ´claim:matrix:lattice-access-work-boundaries´
#[test]
fn lattice_access_work_boundaries() -> TestResult {
    for kind in [0, 1] {
        let ready = Ready::new(kind)?;
        let published = ready
            .heap
            .publish_lattice_decision(ready.prepared()?, &mut ledger()?)?;
        let limits = if kind == 0 {
            [
                (Access::Current, [115, 114]),
                (Access::Requested, [129, 128]),
                (Access::Representative, [115, 114]),
                (Access::Index, [152, 151]),
            ]
        } else {
            [
                (Access::Current, [179, 178]),
                (Access::Requested, [201, 200]),
                (Access::Representative, [179, 178]),
                (Access::Index, [216, 215]),
            ]
        };
        for (access, limits) in limits {
            for (position, work) in limits.into_iter().enumerate() {
                let mut budget = budget(Limits {
                    work,
                    ..Limits::default()
                })?;
                let result = match access {
                    Access::Current => published.current(&ready.heap, &mut budget).map(|_| ()),
                    Access::Requested => published
                        .current_for(&ready.heap, &ready.association, &mut budget)
                        .map(|_| ()),
                    Access::Representative => published
                        .representative(&ready.heap, 0, &mut budget)
                        .map(|_| ()),
                    Access::Index => published
                        .index(&ready.heap, 0, (0, 3), &mut budget)
                        .map(|_| ()),
                };
                if position == 0 {
                    result?;
                    assert_eq!(budget.usage().work, work);
                } else {
                    let Err(fault) = result else {
                        panic!("one-below live consumer Work must refuse");
                    };
                    exhausted(&fault, BudgetKind::Work)?;
                }
                assert!(budget.usage().work > 17);
                assert!(budget.usage().work <= work);
                published.current(&ready.heap, &mut ledger()?)?;
            }
        }
    }
    Ok(())
}

/// A checked vector's returned-storage boundary includes the full native snapshot and receipt.
/// ´test:integration:lattice-enclosure-output-boundaries´
/// ´claim:matrix:lattice-enclosure-output-boundaries´
#[test]
fn lattice_enclosure_output_boundaries() -> TestResult {
    for (kind, limits) in [(0, [5848, 5847]), (1, [8480, 8479])] {
        let ready = Ready::new(kind)?;
        let graph = ready.selected.cas().counts();
        let records = ready.heap.provenance_usage();
        for (position, output_bytes) in limits.into_iter().enumerate() {
            let frozen = ready.heap.freeze_lattice_inputs(
                &ready.association,
                &ready.inputs,
                &mut ledger()?,
            )?;
            let mut budget = budget(Limits {
                output_bytes,
                ..Limits::default()
            })?;
            let result = ready.heap.enclose_lattice_inputs(
                frozen,
                ready.selected.cas(),
                &ready.contract,
                &ArithmeticContext::new()?,
                lattice_live::literals(),
                &mut budget,
            );
            if position == 0 {
                assert_eq!(result?.retained_payload_bytes(), output_bytes);
            } else {
                let Err(fault) = result else {
                    panic!("one-below complete enclosure output storage must refuse");
                };
                exhausted(&fault, BudgetKind::OutputBytes)?;
            }
            assert!(budget.usage().work > 17);
            assert_eq!(ready.selected.cas().counts(), graph);
            assert_eq!(ready.heap.provenance_usage(), records);
        }
    }
    Ok(())
}
