// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit Integer floor frames admit syntax without supplying a projected family cover.

#[path = "integer_family/cover.rs"]
mod cover;

use crate::testing::builders::{admit, environment, expression_draft};
use crate::testing::common::{TestResult, real};
use crate::testing::readings::ledger;
use nrz_cas::domain::{
    AmbientDomain, CaseArmDraft, CasesDraft, ContextDraft, HeaderDraft, RegionDraft, RegionExpr,
};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::syntax::{AtomicSort, Draft, Field, Operator, PredicateKind, Sort};
use nrz_cas::{Cas, FaultKind, Handle, InvalidCode, Location, Rational, Reference};

struct Frame {
    cas: Cas,
    header: Handle,
    ambient: Handle,
    budget: Budget,
}

impl Frame {
    fn new() -> TestResult<Self> {
        Self::with_parameters(false)
    }

    fn with_parameters(parameters: bool) -> TestResult<Self> {
        let mut cas = Cas::new(Limits::default())?;
        let mut budget = ledger()?;
        let names = if parameters {
            &["q", "n", "a", "b"][..]
        } else {
            &["q", "n"][..]
        };
        let mut declarations = environment(Field::Real, names);
        for declaration in declarations.declarations.iter_mut().skip(1) {
            declaration.sort = Sort::Value(AtomicSort::Integer);
        }
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
    fn cases(&mut self, region: &Handle, value: Handle) -> TestResult<(Handle, Handle)> {
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
        let arm = admitted.first().cloned().ok_or("missing floor arm")?;
        let function = admitted.last().cloned().ok_or("missing floor case")?;
        Ok((function, arm))
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
        let (function, _) = frame.cases(&region, value)?;
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

fn mixed_cut(bounded: bool) -> TestResult<(Frame, nrz_cas::decision::CaseCut)> {
    let mut frame = Frame::new()?;
    let (region, [q, n]) = frame.relation(bounded)?;
    let (function, association) = frame.cases(&region, n.clone())?;
    let cut = nrz_cas::decision::CaseCut {
        function,
        association,
        bounds: vec![
            super::interval(q, (203, 64), (205, 64))?,
            super::interval(n, (3, 1), (3, 1))?,
        ],
        integer: None,
    };
    Ok((frame, cut))
}

/// Literal Work and scratch boundaries are atomic, cumulative and independent of output capacity.
/// ´test:integration:integer-coordinate-cut-boundaries´
#[test]
fn integer_coordinate_cut_boundaries() -> TestResult {
    let measured = [(21_214, 59_680), (27_574, 61_500), (39_686, 74_532)];
    for ((_, frame, cut), (work, staged)) in cut_fixtures()?.into_iter().zip(measured) {
        let before = frame.cas.counts();
        let mut budget = ledger()?;
        frame.cas.check_case_cut(&cut, &mut budget)?;
        assert_eq!(budget.usage().work, work);
        assert_eq!(frame.cas.counts(), before);
        for (kind, amount) in [
            (BudgetKind::Work, work),
            (BudgetKind::StagedPayloadBytes, staged),
        ] {
            assert!(cut_attempt(&frame, &cut, kind, amount, 0)?);
            assert!(!cut_attempt(&frame, &cut, kind, amount - 1, 0)?);
        }
        assert!(!cut_attempt(&frame, &cut, BudgetKind::Work, work, 1)?);
        assert!(cut_attempt(&frame, &cut, BudgetKind::Work, work + 1, 1)?);
    }
    Ok(())
}

fn check_cut(
    frame: &Frame,
    cut: &nrz_cas::decision::CaseCut,
    expected: Option<InvalidCode>,
) -> TestResult {
    let before = frame.cas.counts();
    let mut budget = ledger()?;
    let result = frame.cas.check_case_cut(cut, &mut budget);
    if let Some(code) = expected {
        let fault = result.err().ok_or("false mixed cut accepted")?;
        assert_eq!(fault.kind, FaultKind::Invalid(code));
    } else {
        result?;
    }
    assert_eq!(frame.cas.counts(), before);
    assert!(budget.usage().work > 0);
    Ok(())
}

/// Exact Integer axes and explicit embeddings preserve both boundaries of each floor cell.
/// ´test:integration:integer-coordinate-cut´
#[test]
fn integer_coordinate_cut() -> TestResult {
    for bounded in [false, true] {
        let (frame, original) = mixed_cut(bounded)?;
        check_cut(&frame, &original, None)?;
        let mut cut = original.clone();
        cut.bounds[0].lower = super::rational(3, 1)?;
        cut.bounds[0].upper = super::rational(3, 1)?;
        check_cut(&frame, &cut, None)?;
        cut.bounds[0].upper = super::rational(4, 1)?;
        check_cut(
            &frame,
            &cut,
            Some(InvalidCode::DecisionBoundaryIntersection),
        )?;
        cut.clone_from(&original);
        cut.bounds[1].upper = super::rational(4, 1)?;
        check_cut(
            &frame,
            &cut,
            Some(InvalidCode::DecisionBoundaryIntersection),
        )?;
        cut.bounds[1].lower = super::rational(2, 1)?;
        cut.bounds[1].upper = super::rational(2, 1)?;
        check_cut(&frame, &cut, Some(InvalidCode::DecisionOutsideRegion))?;
        for index in [-1, 0, 5] {
            cut.bounds[0].lower = super::rational(index, 1)?;
            cut.bounds[0].upper = super::rational(index, 1)?;
            cut.bounds[1].lower = super::rational(index, 1)?;
            cut.bounds[1].upper = super::rational(index, 1)?;
            let outside = index < 0 || (bounded && index == 5);
            let expected = outside.then_some(InvalidCode::DecisionOutsideRegion);
            check_cut(&frame, &cut, expected)?;
        }
    }
    Ok(())
}

/// Every Integer coordinate is supplied, ordered and integral, so an empty lattice cannot prove a cut.
/// ´test:integration:integer-coordinate-cut-boxes´
#[test]
fn integer_coordinate_cut_boxes() -> TestResult {
    let (frame, original) = mixed_cut(false)?;
    for (lower, upper) in [((13, 4), (7, 2)), ((3, 1), (7, 2)), ((4, 1), (3, 1))] {
        let mut cut = original.clone();
        cut.bounds[1].lower = super::rational(lower.0, lower.1)?;
        cut.bounds[1].upper = super::rational(upper.0, upper.1)?;
        check_cut(&frame, &cut, Some(InvalidCode::DecisionBox))?;
    }
    let mut cut = original.clone();
    cut.bounds.pop();
    check_cut(&frame, &cut, Some(InvalidCode::DecisionBox))?;
    cut.clone_from(&original);
    cut.bounds.push(original.bounds[1].clone());
    check_cut(&frame, &cut, Some(InvalidCode::DecisionBox))?;
    let contract = nrz_cas::decision::CasePartition {
        function: original.function,
        coordinates: original
            .bounds
            .into_iter()
            .map(|interval| interval.coordinate)
            .collect(),
        integer: None,
    };
    let before = frame.cas.counts();
    let fault = frame
        .cas
        .check_case_partition(&contract, &mut ledger()?)
        .err()
        .ok_or("Integer cut implied a projected cover")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Scope));
    assert_eq!(frame.cas.counts(), before);
    Ok(())
}

/// Original nonlinear Integer operands remain unsupported even inside an explicit embedding.
/// ´test:integration:integer-coordinate-cut-nonlinear´
#[test]
fn integer_coordinate_cut_nonlinear() -> TestResult {
    let (mut frame, mut cut) = mixed_cut(false)?;
    let n = cut.bounds[1].coordinate.clone();
    let square = frame.term(
        Operator::Mul,
        Sort::Value(AtomicSort::Integer),
        &[n.clone(), n.clone()],
    )?;
    let embedded = frame.term(Operator::IntegerEmbed, real(), &[square])?;
    let region = frame.guard(PredicateKind::Positive, &[embedded])?;
    (cut.function, cut.association) = frame.cases(&region, n)?;
    check_cut(&frame, &cut, Some(InvalidCode::UnsupportedCertificate))
}

fn cut_attempt(
    frame: &Frame,
    cut: &nrz_cas::decision::CaseCut,
    kind: BudgetKind,
    amount: u64,
    prior: u64,
) -> TestResult<bool> {
    let mut limits = Limits {
        output_bytes: 0,
        payload_bytes: 0,
        ..Limits::default()
    };
    match kind {
        BudgetKind::Work => limits.work = amount,
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = amount,
        _ => return Err("unsupported mixed cut resource".into()),
    }
    let mut budget = Budget::new(limits.clone())?;
    budget.work(prior, Location::new("prior mixed cut work"))?;
    let before = frame.cas.counts();
    let result = frame.cas.check_case_cut(cut, &mut budget);
    assert_eq!(frame.cas.counts(), before);
    assert_eq!(budget.limits(), &limits);
    match result {
        Ok(()) => Ok(true),
        Err(fault) => {
            assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind));
            Ok(false)
        }
    }
}

fn parametric_cut() -> TestResult<(Frame, nrz_cas::decision::CaseCut)> {
    let mut frame = Frame::with_parameters(true)?;
    let (relation, [q, n]) = frame.relation(false)?;
    let a = frame.term(
        Operator::Free("a".into()),
        Sort::Value(AtomicSort::Integer),
        &[],
    )?;
    let b = frame.term(
        Operator::Free("b".into()),
        Sort::Value(AtomicSort::Integer),
        &[],
    )?;
    let range = frame.guard(
        PredicateKind::IntegerRange,
        &[n.clone(), a.clone(), b.clone()],
    )?;
    let region = frame.region(RegionExpr::Meet(vec![
        Reference::Existing(relation),
        Reference::Existing(range),
    ]))?;
    let (function, association) = frame.cases(&region, n.clone())?;
    let cut = nrz_cas::decision::CaseCut {
        function,
        association,
        bounds: vec![
            super::interval(q, (203, 64), (205, 64))?,
            super::interval(n, (3, 1), (3, 1))?,
            super::interval(a, (0, 1), (2, 1))?,
            super::interval(b, (4, 1), (5, 1))?,
        ],
        integer: None,
    };
    Ok((frame, cut))
}

type CutFixture = (&'static str, Frame, nrz_cas::decision::CaseCut);

fn cut_fixtures() -> TestResult<[CutFixture; 3]> {
    let (nonnegative, first) = mixed_cut(false)?;
    let (bounded, second) = mixed_cut(true)?;
    let (parametric, third) = parametric_cut()?;
    Ok([
        ("nonnegative", nonnegative, first),
        ("bounded", bounded, second),
        ("parametric", parametric, third),
    ])
}

/// Variable Integer endpoints are checked over every parameter assignment in the supplied box.
/// ´test:integration:integer-coordinate-cut-parameters´
#[test]
fn integer_coordinate_cut_parameters() -> TestResult {
    let (frame, original) = parametric_cut()?;
    check_cut(&frame, &original, None)?;
    let mut cut = original.clone();
    cut.bounds[2].upper = super::rational(4, 1)?;
    check_cut(
        &frame,
        &cut,
        Some(InvalidCode::DecisionBoundaryIntersection),
    )?;
    cut.bounds[2].lower = super::rational(4, 1)?;
    check_cut(&frame, &cut, Some(InvalidCode::DecisionOutsideRegion))?;
    cut.clone_from(&original);
    cut.bounds[3].lower = super::rational(3, 1)?;
    check_cut(
        &frame,
        &cut,
        Some(InvalidCode::DecisionBoundaryIntersection),
    )?;
    cut.bounds[3].upper = super::rational(3, 1)?;
    check_cut(&frame, &cut, Some(InvalidCode::DecisionOutsideRegion))?;
    cut.clone_from(&original);
    cut.bounds.pop();
    check_cut(&frame, &cut, Some(InvalidCode::DecisionBox))
}
