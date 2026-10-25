// SPDX-License-Identifier: AGPL-3.0-only
//! Ordinary local membership and native selection before certified decision admission.

use crate::testing::builders::{admit, environment, expression};
use crate::testing::common::{TestResult, real};
use crate::testing::readings::ledger;
use nrz_cas::domain::{
    AmbientDomain, CaseArmDraft, CasesDraft, ContextDraft, HeaderDraft, HypothesisRef, RegionDraft,
    RegionExpr,
};
use nrz_cas::limits::{Budget, BudgetKind, Limits, LimitsOverride};
use nrz_cas::proof::{ApplicationDraft, Certificate, Primitive, Statement, VerifiedWitness};
use nrz_cas::request::{CoveredInput, Outcome, Request, Task};
use nrz_cas::syntax::{Draft, Field, Operator, PredicateKind};
use nrz_cas::{Cas, FaultKind, Handle, InvalidCode, Location, Rational, Reference};

#[derive(Clone, Copy, Debug)]
pub(super) enum Example {
    Integer,
    Sign,
    Phase,
}

pub(super) struct Fixture {
    pub(super) cas: Cas,
    pub(super) header: Handle,
    pub(super) context: Handle,
    pub(super) ambient: Handle,
    pub(super) kind: AmbientDomain,
}

impl Fixture {
    pub(super) fn new(example: Example) -> TestResult<Self> {
        let (keys, kind) = match example {
            Example::Integer | Example::Sign => (vec!["x"], AmbientDomain::Real("x".into())),
            Example::Phase => (vec!["a", "b"], AmbientDomain::Assignments),
        };
        let mut cas = Cas::new(Limits::default())?;
        let header = admit(
            &mut cas,
            Draft::Header(HeaderDraft::Core(environment(Field::Real, &keys))),
            &mut ledger()?,
        )?;
        let ambient = admit(
            &mut cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                ambient: kind.clone(),
                expression: RegionExpr::Ambient,
            }),
            &mut ledger()?,
        )?;
        let context = admit(
            &mut cas,
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(header.clone()),
                admissible: Reference::Existing(ambient.clone()),
                assumptions: Vec::new(),
            }),
            &mut ledger()?,
        )?;
        Ok(Self {
            cas,
            header,
            context,
            ambient,
            kind,
        })
    }

    pub(super) fn term(&mut self, op: Operator, operands: &[Handle]) -> TestResult<Handle> {
        expression(
            &mut self.cas,
            &self.context,
            op,
            real(),
            operands,
            &mut ledger()?,
        )
    }

    pub(super) fn number(&mut self, n: i64, d: u64) -> TestResult<Handle> {
        self.term(
            Operator::Rational(Rational::from_ratio(n, d, &mut ledger()?)?),
            &[],
        )
    }

    pub(super) fn region(&mut self, value: RegionExpr) -> TestResult<Handle> {
        admit(
            &mut self.cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                ambient: self.kind.clone(),
                expression: value,
            }),
            &mut ledger()?,
        )
    }

    pub(super) fn guard(
        &mut self,
        kind: PredicateKind,
        arguments: &[Handle],
    ) -> TestResult<Handle> {
        let predicate = admit(
            &mut self.cas,
            Draft::Condition {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                kind,
                arguments: arguments.iter().cloned().map(Reference::Existing).collect(),
            },
            &mut ledger()?,
        )?;
        self.region(RegionExpr::GuardTrue {
            base: Reference::Existing(self.ambient.clone()),
            predicate: Reference::Existing(predicate),
        })
    }

    pub(super) fn meet(&mut self, values: Vec<Handle>) -> TestResult<Handle> {
        self.region(RegionExpr::Meet(
            values.into_iter().map(Reference::Existing).collect(),
        ))
    }

    pub(super) fn interval(
        &mut self,
        x: &Handle,
        lower: (i64, u64),
        upper: (i64, u64),
        strict: bool,
    ) -> TestResult<Handle> {
        let lower = self.number(lower.0, lower.1)?;
        let upper = self.number(upper.0, upper.1)?;
        let lo = self.term(Operator::Sub, &[x.clone(), lower])?;
        let hi = self.term(Operator::Sub, &[upper, x.clone()])?;
        let lo = self.guard(PredicateKind::NonNegative, &[lo])?;
        let hi = self.guard(
            if strict {
                PredicateKind::Positive
            } else {
                PredicateKind::NonNegative
            },
            &[hi],
        )?;
        self.meet(vec![lo, hi])
    }

    pub(super) fn regions(&mut self, example: Example) -> TestResult<(Handle, Handle)> {
        match example {
            Example::Integer => {
                let x = self.term(Operator::Free("x".into()), &[])?;
                let enclosure = self.interval(&x, (203, 64), (205, 64), false)?;
                let option = self.interval(&x, (3, 1), (4, 1), true)?;
                Ok((enclosure, option))
            }
            Example::Sign => {
                let x = self.term(Operator::Free("x".into()), &[])?;
                let enclosure = self.interval(&x, (-301, 4), (-299, 4), false)?;
                let negative = self.term(Operator::Neg, &[x])?;
                let option = self.guard(PredicateKind::Positive, &[negative])?;
                Ok((enclosure, option))
            }
            Example::Phase => self.phase(),
        }
    }

    pub(super) fn phase(&mut self) -> TestResult<(Handle, Handle)> {
        let a = self.term(Operator::Free("a".into()), &[])?;
        let b = self.term(Operator::Free("b".into()), &[])?;
        let horizontal = self.interval(&a, (-7, 4), (-5, 4), false)?;
        let vertical = self.interval(&b, (1, 4), (1, 2), false)?;
        let enclosure = self.meet(vec![horizontal, vertical])?;
        let negative = self.term(Operator::Neg, &[a])?;
        let left = self.guard(PredicateKind::Positive, &[negative])?;
        let upper = self.guard(PredicateKind::Positive, &[b])?;
        Ok((enclosure, self.meet(vec![left, upper])?))
    }

    fn cases(&mut self, option: Handle) -> TestResult<Handle> {
        let value = self.number(3, 1)?;
        let arm = admit(
            &mut self.cas,
            Draft::CaseArm(CaseArmDraft {
                placements: Vec::new(),
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                region: Reference::Existing(option),
                value: Reference::Existing(value),
            }),
            &mut ledger()?,
        )?;
        admit(
            &mut self.cas,
            Draft::Cases(CasesDraft {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                domain: Reference::Existing(self.ambient.clone()),
                arms: vec![Reference::Existing(arm)],
            }),
            &mut ledger()?,
        )
    }

    fn membership(
        &mut self,
        enclosure: &Handle,
        budget: &mut Budget,
    ) -> TestResult<(Handle, VerifiedWitness)> {
        let before = self.cas.counts();
        let admitted = admit(
            &mut self.cas,
            Draft::Context(ContextDraft::Local {
                parent: Reference::Existing(self.context.clone()),
                region: Reference::Existing(enclosure.clone()),
                hypotheses: Vec::new(),
            }),
            budget,
        );
        if admitted.is_err() {
            assert_eq!(self.cas.counts(), before);
        }
        let local = admitted?;
        let step = ApplicationDraft {
            context: local.clone(),
            rule: Primitive::ContextHypothesis,
            conclusion: Statement::Subset {
                subset: self.cas.admissible_region(&local)?,
                superset: enclosure.clone(),
            },
            certificate: Certificate::Hypothesis(HypothesisRef {
                context: local.clone(),
                slot: 0,
            }),
            premises: Vec::new(),
            substitutions: Vec::new(),
        };
        let before = self.cas.counts();
        let checked = self.cas.check_proof(&local, &[step], budget);
        if checked.is_err() {
            assert_eq!(self.cas.counts(), before);
        }
        Ok((local, checked?))
    }

    fn request(
        &mut self,
        cases: &Handle,
        local: Handle,
        member: VerifiedWitness,
    ) -> TestResult<Request> {
        let root = self.cas.materialize_derived(cases, cases, &mut ledger()?)?;
        let receipt = self.cas.walk_derived(&root, &mut ledger()?)?.receipt;
        Ok(Request {
            task: Task::Select {
                region: self.cas.admissible_region(&local)?,
            },
            context: local,
            inputs: vec![CoveredInput { root, receipt }],
            premises: vec![member],
            limits: LimitsOverride::default(),
        })
    }
}

/// Independent records distinguish membership admission from producer and checker work.
#[derive(Clone, Copy)]
struct Costs {
    example: Example,
    admission: u64,
    generation: u64,
    checking: u64,
    staged: [u64; 2],
    payload: [u64; 2],
    added: [u64; 2],
    witness: [usize; 2],
}

const COSTS: [Costs; 3] = [
    Costs {
        example: Example::Integer,
        admission: 29_300,
        generation: 956_428,
        checking: 1_715_691,
        staged: [20_816, 169_741],
        payload: [14_095, 36_455],
        added: [2_055, 14_446],
        witness: [783, 1_610],
    },
    Costs {
        example: Example::Sign,
        admission: 29_340,
        generation: 742_533,
        checking: 1_336_868,
        staged: [20_848, 168_496],
        payload: [11_645, 30_199],
        added: [2_055, 14_392],
        witness: [785, 1_391],
    },
    Costs {
        example: Example::Phase,
        admission: 43_786,
        generation: 208_643,
        checking: 0,
        staged: [33_120, 37_817],
        payload: [18_188, 34_357],
        added: [2_079, 9_662],
        witness: [1_193, 0],
    },
];

fn prepared(example: Example) -> TestResult<(Fixture, Handle, Handle)> {
    let mut fixture = Fixture::new(example)?;
    let (enclosure, option) = fixture.regions(example)?;
    let cases = fixture.cases(option)?;
    Ok((fixture, enclosure, cases))
}

/// Exact default-profile costs are independent baseline data, never certified origin evidence.
/// ´test:integration:decision-baseline-costs´
#[test]
fn decision_baseline_costs() -> TestResult {
    for costs in COSTS {
        let (mut fixture, enclosure, cases) = prepared(costs.example)?;
        let before = fixture.cas.counts();
        let mut budget = ledger()?;
        let (local, member) = fixture.membership(&enclosure, &mut budget)?;
        assert_eq!(budget.usage().work, costs.admission);
        assert_eq!(
            fixture.cas.counts().payload_bytes - before.payload_bytes,
            costs.added[0]
        );
        assert_eq!(member.exported_byte_len(), costs.witness[0]);
        assert_eq!(
            member.hypotheses(),
            &[HypothesisRef {
                context: local.clone(),
                slot: 0
            }]
        );
        let request = fixture.request(&cases, local, member)?;
        let before = fixture.cas.counts();
        check_selection(&fixture.cas.run(&request), &request, &costs)?;
        assert_eq!(
            fixture.cas.counts().payload_bytes - before.payload_bytes,
            costs.added[1]
        );
    }
    Ok(())
}

fn check_selection(outcome: &Outcome, request: &Request, costs: &Costs) -> TestResult {
    match outcome {
        Outcome::Proved { result, usage, .. } => {
            assert_eq!(usage.work, costs.generation);
            assert_eq!(result.replay_usage.work, costs.checking);
            assert_eq!(result.witness.exported_byte_len(), costs.witness[1]);
            assert_eq!(result.witness.judgment().context, request.context);
            let Statement::Equality { domain, .. } = &result.witness.judgment().statement else {
                return Err("selection did not prove a local equality".into());
            };
            let Task::Select { region } = &request.task else {
                return Err("fixture lost its selection request".into());
            };
            assert_eq!(domain, region);
        }
        Outcome::PendingPremise {
            partial,
            blocker,
            usage,
            ..
        } => {
            assert!(matches!(costs.example, Example::Phase));
            assert!(partial.is_none());
            assert_eq!(
                blocker.kind,
                FaultKind::Invalid(InvalidCode::PendingPremise)
            );
            assert_eq!(blocker.location.operation, "exact region partition");
            assert_eq!(usage.work, costs.generation);
        }
        other => return Err(format!("unexpected baseline outcome: {other:?}").into()),
    }
    Ok(())
}

/// Configure a measured caller allowance without changing any production default.
fn bounded(kind: BudgetKind, limit: u64) -> Limits {
    let mut limits = Limits::default();
    match kind {
        BudgetKind::Work => limits.work = limit,
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = limit,
        BudgetKind::PayloadBytes => limits.payload_bytes = limit,
        _ => {}
    }
    limits
}

/// A failed public admission call rolls back its own graph changes; earlier calls remain committed.
fn admission_attempt(costs: &Costs, kind: BudgetKind, limit: u64, prior: u64) -> TestResult<bool> {
    let (mut fixture, enclosure, _) = prepared(costs.example)?;
    let mut budget = Budget::new(bounded(kind, limit))?;
    budget.work(prior, Location::new("prior ordinary membership work"))?;
    let result = fixture.membership(&enclosure, &mut budget);
    if let Err(error) = &result {
        let fault = error
            .downcast_ref::<nrz_cas::Fault>()
            .ok_or("unexpected baseline error")?;
        assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind));
    }
    Ok(result.is_ok())
}

/// Ordinary frame admission and hypothesis proof share the exact cumulative Work allowance.
/// ´test:integration:decision-baseline-admission-boundaries´
#[test]
fn decision_baseline_admission_boundaries() -> TestResult {
    for costs in COSTS {
        for (kind, needed) in [
            (BudgetKind::Work, costs.admission),
            (BudgetKind::StagedPayloadBytes, costs.staged[0]),
            (BudgetKind::PayloadBytes, costs.payload[0]),
        ] {
            assert!(admission_attempt(&costs, kind, needed, 0)?);
            assert!(!admission_attempt(&costs, kind, needed - 1, 0)?);
        }
        assert!(!admission_attempt(
            &costs,
            BudgetKind::Work,
            costs.admission,
            1
        )?);
    }
    Ok(())
}

fn selection_attempt(costs: &Costs, kind: BudgetKind, limit: u64) -> TestResult<bool> {
    let (mut fixture, enclosure, cases) = prepared(costs.example)?;
    let (local, member) = fixture.membership(&enclosure, &mut ledger()?)?;
    let mut request = fixture.request(&cases, local, member)?;
    match kind {
        BudgetKind::Work => request.limits.work = Some(limit),
        BudgetKind::ReplayWork => request.limits.replay_work = Some(limit),
        BudgetKind::StagedPayloadBytes => request.limits.staged_payload_bytes = Some(limit),
        BudgetKind::PayloadBytes => request.limits.payload_bytes = Some(limit),
        _ => return Err("unsupported baseline ceiling".into()),
    }
    match fixture.cas.run(&request) {
        Outcome::Proved { result, .. } => Ok(kind != BudgetKind::Work || result.response.is_some()),
        Outcome::PendingPremise { .. } => Ok(matches!(costs.example, Example::Phase)),
        Outcome::Exhausted { fault, .. } => {
            assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. }
                if budget == kind || (kind == BudgetKind::ReplayWork && budget == BudgetKind::Work)));
            Ok(false)
        }
        other => Err(format!("unexpected baseline boundary: {other:?}").into()),
    }
}

/// Selection's producer, checker and payload owners have independent literal boundaries.
/// ´test:integration:decision-baseline-selection-boundaries´
#[test]
fn decision_baseline_selection_boundaries() -> TestResult {
    for costs in COSTS {
        for (kind, needed) in [
            (BudgetKind::Work, costs.generation),
            (BudgetKind::StagedPayloadBytes, costs.staged[1]),
            (BudgetKind::PayloadBytes, costs.payload[1]),
        ] {
            assert!(selection_attempt(&costs, kind, needed)?);
            assert!(!selection_attempt(&costs, kind, needed - 1)?);
        }
        if costs.checking != 0 {
            assert!(selection_attempt(
                &costs,
                BudgetKind::ReplayWork,
                costs.checking
            )?);
            assert!(!selection_attempt(
                &costs,
                BudgetKind::ReplayWork,
                costs.checking - 1
            )?);
        }
    }
    Ok(())
}

/// Export refuses one missing byte without changing either graph occupancy or caller usage.
/// ´test:integration:decision-baseline-export-boundaries´
#[test]
fn decision_baseline_export_boundaries() -> TestResult {
    for costs in COSTS {
        let (mut fixture, enclosure, _) = prepared(costs.example)?;
        let (_, member) = fixture.membership(&enclosure, &mut ledger()?)?;
        let count = u64::try_from(costs.witness[0])?;
        let before = fixture.cas.counts();
        let mut exact = Budget::new(Limits {
            output_bytes: count,
            ..Limits::default()
        })?;
        assert_eq!(
            fixture.cas.export_witness(&member, &mut exact)?.len(),
            costs.witness[0]
        );
        let mut short = Budget::new(Limits {
            output_bytes: count - 1,
            ..Limits::default()
        })?;
        let usage = short.usage();
        assert!(fixture.cas.export_witness(&member, &mut short).is_err());
        assert_eq!(short.usage(), usage);
        assert_eq!(fixture.cas.counts(), before);
    }
    Ok(())
}
