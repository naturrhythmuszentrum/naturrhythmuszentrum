// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit Integer floor frames admit syntax without supplying a projected family cover.

use crate::testing::builders::{admit, environment, expression_draft};
use crate::testing::common::{TestResult, real};
use crate::testing::readings::ledger;
use nrz_cas::domain::{
    AmbientDomain, CaseArmDraft, CasesDraft, ContextDraft, HeaderDraft, RegionDraft, RegionExpr,
};
use nrz_cas::limits::{Budget, Limits};
use nrz_cas::syntax::{AtomicSort, Draft, Field, Operator, PredicateKind, Sort};
use nrz_cas::{Cas, Handle, Rational, Reference};

struct Frame {
    cas: Cas,
    header: Handle,
    ambient: Handle,
    budget: Budget,
}

impl Frame {
    fn new() -> TestResult<Self> {
        let mut cas = Cas::new(Limits::default())?;
        let mut budget = ledger()?;
        let mut declarations = environment(Field::Real, &["q", "n"]);
        declarations.declarations[1].sort = Sort::Value(AtomicSort::Integer);
        let header = admit(
            &mut cas,
            Draft::Header(HeaderDraft::Core(declarations)),
            &mut budget,
        )?;
        let ambient = admit(
            &mut cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Assignments,
                expression: RegionExpr::Ambient,
            }),
            &mut budget,
        )?;
        Ok(Self {
            cas,
            header,
            ambient,
            budget,
        })
    }

    fn term(&mut self, op: Operator, sort: Sort, args: &[Handle]) -> TestResult<Handle> {
        admit(
            &mut self.cas,
            expression_draft(
                op,
                sort,
                Reference::Existing(self.header.clone()),
                Vec::new(),
                None,
                args,
            ),
            &mut self.budget,
        )
    }

    fn region(&mut self, expression: RegionExpr) -> TestResult<Handle> {
        admit(
            &mut self.cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Assignments,
                expression,
            }),
            &mut self.budget,
        )
    }

    fn guard(&mut self, kind: PredicateKind, args: &[Handle]) -> TestResult<Handle> {
        let predicate = admit(
            &mut self.cas,
            Draft::Condition {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                kind,
                arguments: args.iter().cloned().map(Reference::Existing).collect(),
            },
            &mut self.budget,
        )?;
        self.region(RegionExpr::GuardTrue {
            base: Reference::Existing(self.ambient.clone()),
            predicate: Reference::Existing(predicate),
        })
    }

    fn relation(&mut self, bounded: bool) -> TestResult<(Handle, [Handle; 2])> {
        let q = self.term(Operator::Free("q".into()), real(), &[])?;
        let n = self.term(
            Operator::Free("n".into()),
            Sort::Value(AtomicSort::Integer),
            &[],
        )?;
        let embedded = self.term(Operator::IntegerEmbed, real(), std::slice::from_ref(&n))?;
        let one = self.term(Operator::Rational(Rational::one()), real(), &[])?;
        let end = self.term(Operator::Add, real(), &[embedded.clone(), one])?;
        let lower = self.term(Operator::Sub, real(), &[q.clone(), embedded])?;
        let upper = self.term(Operator::Sub, real(), &[end, q.clone()])?;
        let lower = self.guard(PredicateKind::NonNegative, &[lower])?;
        let upper = self.guard(PredicateKind::Positive, &[upper])?;
        let mut regions = vec![lower, upper];
        if bounded {
            let zero = self.term(
                Operator::Rational(Rational::zero()),
                Sort::Value(AtomicSort::Integer),
                &[],
            )?;
            let five = Rational::from_ratio(5, 1, &mut self.budget)?;
            let five = self.term(
                Operator::Rational(five),
                Sort::Value(AtomicSort::Integer),
                &[],
            )?;
            regions.push(self.guard(PredicateKind::IntegerRange, &[n.clone(), zero, five])?);
        } else {
            regions.push(self.guard(PredicateKind::NonNegative, std::slice::from_ref(&n))?);
        }
        let relation = self.region(RegionExpr::Meet(
            regions.into_iter().map(Reference::Existing).collect(),
        ))?;
        Ok((relation, [q, n]))
    }
}

impl Frame {
    fn cases(&mut self, region: &Handle, value: Handle) -> TestResult<Handle> {
        let admitted = self.cas.admit(
            &[
                Draft::CaseArm(CaseArmDraft {
                    header: Reference::Existing(self.header.clone()),
                    scope: Vec::new(),
                    placements: Vec::new(),
                    region: Reference::Existing(region.clone()),
                    value: Reference::Existing(value),
                }),
                Draft::Cases(CasesDraft {
                    sort: Sort::Value(AtomicSort::Integer),
                    header: Reference::Existing(self.header.clone()),
                    scope: Vec::new(),
                    domain: Reference::Existing(region.clone()),
                    arms: vec![Reference::Local(0)],
                }),
            ],
            &mut self.budget,
        )?;
        admitted
            .last()
            .cloned()
            .ok_or_else(|| "missing floor case".into())
    }

    fn select(&mut self, region: Handle, function: &Handle) -> TestResult<u64> {
        use nrz_cas::limits::LimitsOverride;
        use nrz_cas::request::{CoveredInput, Outcome, Request, Task};
        let context = admit(
            &mut self.cas,
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(self.header.clone()),
                admissible: Reference::Existing(region.clone()),
                assumptions: Vec::new(),
            }),
            &mut ledger()?,
        )?;
        let root = self
            .cas
            .materialize_derived(function, function, &mut ledger()?)?;
        let receipt = self.cas.walk_derived(&root, &mut ledger()?)?.receipt;
        let supplied = Request {
            task: Task::Select { region },
            context,
            inputs: vec![CoveredInput { root, receipt }],
            premises: Vec::new(),
            limits: LimitsOverride::default(),
        };
        let Outcome::PendingPremise { blocker, usage, .. } = self.cas.run(&supplied) else {
            return Err("unproved floor selection escaped the partition boundary".into());
        };
        assert_eq!(
            blocker.kind,
            nrz_cas::FaultKind::Invalid(nrz_cas::InvalidCode::PendingPremise)
        );
        assert_eq!(blocker.location.operation, "exact region partition");
        Ok(usage.work)
    }
}

/// Both exact ranges admit Integer-valued syntax but still refuse an unproved floor cover.
/// ´test:integration:integer-case-family-boundary´
#[test]
fn integer_case_family_boundary() -> TestResult {
    for (bounded, declaration, admission, partition, selection) in [
        (false, 16_227, 6_913, 530, 646_328),
        (true, 18_427, 7_549, 1_390, 701_891),
    ] {
        let mut frame = Frame::new()?;
        let (region, [coordinate, value]) = frame.relation(bounded)?;
        assert_eq!(frame.budget.usage().work, declaration);
        let before = frame.cas.counts();
        let function = frame.cases(&region, value)?;
        assert_eq!(frame.budget.usage().work, declaration + admission);
        assert_eq!(frame.cas.counts().payload_bytes - before.payload_bytes, 965);
        assert_eq!(
            frame.cas.expression(&function)?.1,
            &Sort::Value(AtomicSort::Integer)
        );
        let contract = nrz_cas::decision::CasePartition {
            function: function.clone(),
            coordinates: vec![coordinate],
            integer: None,
        };
        let before = frame.cas.counts();
        let mut budget = ledger()?;
        let fault = frame
            .cas
            .check_case_partition(&contract, &mut budget)
            .err()
            .ok_or("unproved floor cover accepted")?;
        assert_eq!(
            fault.kind,
            nrz_cas::FaultKind::Invalid(nrz_cas::InvalidCode::UnsupportedCertificate)
        );
        assert_eq!(fault.location.operation, "exact case box cut");
        assert_eq!(budget.usage().work, partition);
        assert_eq!(frame.cas.counts(), before);
        assert_eq!(frame.select(region, &function)?, selection);
    }
    Ok(())
}
