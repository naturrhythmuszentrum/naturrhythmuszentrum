// SPDX-License-Identifier: AGPL-3.0-only
//! Original LC floor selection assembled solely through public CAS constructors.

mod family;

use crate::binding_testing::TestResult;
use nrz_bindings::readings::algebra::decision::{
    EnclosureOrigin, IntegerFamily, IntegerSelection, RationalEnclosure,
};
use nrz_bindings::readings::algebra::domain::{
    AmbientDomain, AssumptionDraft, CaseArmDraft, CasesDraft, ContextDraft, HeaderDraft,
    HypothesisRef, RegionDraft, RegionExpr,
};
use nrz_bindings::readings::algebra::limits::{Budget, Limits};
use nrz_bindings::readings::algebra::pi_bounds::PiBoundCertificate;
use nrz_bindings::readings::algebra::proof::{ApplicationDraft, Certificate, Primitive, Statement};
use nrz_bindings::readings::algebra::syntax::{
    AtomicSort, Declaration, Draft, Environment, ExpressionDraft, Field, Operator, PredicateKind,
    Sort,
};
use nrz_bindings::readings::algebra::{Cas, Handle, Rational, Reference};

/// Unchanged production limits for independent fixture setup.
pub(super) fn ledger() -> TestResult<Budget> {
    Ok(Budget::new(Limits::default())?)
}
/// Exact Real scalar sort.
const fn real() -> Sort {
    Sort::Value(AtomicSort::Scalar(Field::Real))
}
/// One checked draft without hidden graph access.
fn admit(cas: &mut Cas, draft: Draft, budget: &mut Budget) -> TestResult<Handle> {
    cas.admit(&[draft], budget)?
        .pop()
        .ok_or_else(|| "missing admitted handle".into())
}
/// Original mixed Real/Integer source frame.
pub(super) struct Frame {
    /// Retained exact graph.
    pub(super) cas: Cas,
    /// Original declaration identity.
    header: Handle,
    /// Full assignment domain.
    ambient: Handle,
    /// Fixture syntax ledger.
    budget: Budget,
}
impl Frame {
    /// Declare q and its bound Integer index.
    fn new() -> TestResult<Self> {
        let mut cas = Cas::new(Limits::default())?;
        let mut budget = ledger()?;
        let header = admit(
            &mut cas,
            Draft::Header(HeaderDraft::Core(Environment {
                declarations: [("q", real()), ("n", Sort::Value(AtomicSort::Integer))]
                    .into_iter()
                    .map(|(name, sort)| Declaration {
                        key: name.into(),
                        display_name: name.into(),
                        sort,
                    })
                    .collect(),
                ..Environment::default()
            })),
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
    /// Preserve all ordered original operands.
    fn term(&mut self, operator: Operator, sort: Sort, args: &[Handle]) -> TestResult<Handle> {
        admit(
            &mut self.cas,
            Draft::Expression(ExpressionDraft {
                space: None,
                operator,
                sort,
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                binding: None,
                operands: args.iter().cloned().map(Reference::Existing).collect(),
            }),
            &mut self.budget,
        )
    }
    /// Construct an exact assignment region.
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
    /// Give a predicate its exact ambient ownership.
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
    /// Declare the half-open floor relation and exact Integer range.
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
    /// Retain the native case and its original association.
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
}

/// Declare projected coverage without asserting its proof.
fn cover(bounded: bool) -> TestResult<(Frame, IntegerFamily)> {
    cover_in(Frame::new()?, bounded)
}

/// A family fixture adds its physical declaration before constructing the unchanged floor relation.
fn cover_in(mut frame: Frame, bounded: bool) -> TestResult<(Frame, IntegerFamily)> {
    let (relation, [q, n]) = frame.relation(bounded)?;
    let (function, _) = frame.cases(&relation, n.clone())?;
    let zero = frame.term(
        Operator::Rational(Rational::zero()),
        Sort::Value(AtomicSort::Integer),
        &[],
    )?;
    let mut domain = frame.guard(PredicateKind::NonNegative, std::slice::from_ref(&q))?;
    let upper = if bounded {
        let five = Rational::from_ratio(5, 1, &mut ledger()?)?;
        let bound = frame.term(
            Operator::Rational(five.clone()),
            Sort::Value(AtomicSort::Integer),
            &[],
        )?;
        let constant = frame.term(Operator::Rational(five), real(), &[])?;
        let difference = frame.term(Operator::Sub, real(), &[constant, q.clone()])?;
        let high = frame.guard(PredicateKind::Positive, &[difference])?;
        domain = frame.region(RegionExpr::Meet(vec![
            Reference::Existing(domain),
            Reference::Existing(high),
        ]))?;
        Some(bound)
    } else {
        None
    };
    Ok((
        frame,
        IntegerFamily {
            function,
            coordinate: q,
            index: n,
            domain,
            lower: Some(zero),
            upper,
            count: None,
        },
    ))
}

/// Complete original target, family and proof proposal.
pub(super) struct SelectionFixture {
    /// Original graph and syntax.
    pub(super) frame: Frame,
    /// Source correspondence context.
    pub(super) context: Handle,
    /// Independently checked proof proposal.
    pub(super) steps: Vec<ApplicationDraft>,
}

/// The original varying LC duration and its retained exact source-box premises.
pub(super) fn family_fixture() -> TestResult<(
    SelectionFixture,
    nrz_bindings::readings::algebra::decision::ScalarKernelFamily,
)> {
    family::fixture()
}

/// Admit an exact rational with explicit mathematical sort.
fn exact(frame: &mut Frame, text: &str, integer: bool) -> TestResult<Handle> {
    let number = Rational::parse(text, &mut ledger()?)?;
    let sort = if integer {
        Sort::Value(AtomicSort::Integer)
    } else {
        real()
    };
    frame.term(Operator::Rational(number), sort, &[])
}

/// Build the original LC target or an exact rational refusal twin.
pub(super) fn fixture(bounded: bool, selected: &str, target: &str) -> TestResult<SelectionFixture> {
    let (frame, family) = cover(bounded)?;
    assemble(frame, family, Vec::new(), selected, target)
}

/// Keep original source equality as an explicit qualified assumption.
fn assemble(
    mut frame: Frame,
    family: IntegerFamily,
    mut bindings: Vec<Handle>,
    selected: &str,
    target: &str,
) -> TestResult<SelectionFixture> {
    let value = exact(&mut frame, selected, true)?;
    let (target, number) = if target == "10/Pi" {
        let pi = frame.term(
            Operator::Constant(nrz_bindings::readings::algebra::syntax::Constant::Pi),
            real(),
            &[],
        )?;
        let ten = exact(&mut frame, "10", false)?;
        (frame.term(Operator::Div, real(), &[ten, pi])?, None)
    } else {
        let number = Rational::parse(target, &mut ledger()?)?;
        (exact(&mut frame, target, false)?, Some(number))
    };
    let predicate = admit(
        &mut frame.cas,
        Draft::Condition {
            header: Reference::Existing(frame.header.clone()),
            scope: Vec::new(),
            kind: PredicateKind::Equal,
            arguments: vec![
                Reference::Existing(family.coordinate.clone()),
                Reference::Existing(target.clone()),
            ],
        },
        &mut ledger()?,
    )?;
    bindings.push(predicate);
    let context = admit(
        &mut frame.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(frame.header.clone()),
            admissible: Reference::Existing(family.domain.clone()),
            assumptions: bindings
                .iter()
                .enumerate()
                .map(|(index, condition)| AssumptionDraft {
                    key: format!("selection-source-{index}"),
                    condition: Reference::Existing(condition.clone()),
                })
                .collect(),
        }),
        &mut ledger()?,
    )?;
    let steps = applications(&context, family, value, target, &bindings, number)?;
    Ok(SelectionFixture {
        frame,
        context,
        steps,
    })
}

/// Reference the exact source slot and its predicate.
fn hypothesis(
    context: &Handle,
    domain: &Handle,
    predicate: &Handle,
    slot: u32,
) -> ApplicationDraft {
    ApplicationDraft {
        rule: Primitive::ContextHypothesis,
        context: context.clone(),
        conclusion: Statement::PredicateFact {
            region: domain.clone(),
            predicate: predicate.clone(),
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Hypothesis(HypothesisRef {
            context: context.clone(),
            slot,
        }),
    }
}

/// Declare cover, correspondence and the original-target selection certificate.
fn applications(
    context: &Handle,
    family: IntegerFamily,
    value: Handle,
    target: Handle,
    bindings: &[Handle],
    number: Option<Rational>,
) -> TestResult<Vec<ApplicationDraft>> {
    let mut steps = Vec::new();
    for (index, predicate) in bindings.iter().enumerate() {
        steps.push(hypothesis(
            context,
            &family.domain,
            predicate,
            u32::try_from(index)?,
        ));
    }
    let cover = u32::try_from(steps.len())?;
    let correspondence = cover.checked_sub(1).ok_or("missing coordinate binding")?;
    steps.push(ApplicationDraft {
        rule: Primitive::RegionDecide,
        context: context.clone(),
        conclusion: Statement::IntegerFamily(Box::new(family.clone())),
        premises: (0..correspondence).collect(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    });
    let (lower, upper, origin) = if let Some(number) = number {
        (
            number.clone(),
            number.clone(),
            EnclosureOrigin::ExactRational {
                expression: target.clone(),
                value: number,
            },
        )
    } else {
        (
            Rational::from_ratio(203, 64, &mut ledger()?)?,
            Rational::from_ratio(205, 64, &mut ledger()?)?,
            EnclosureOrigin::PiBound {
                expression: target.clone(),
                certificate: Box::new(cycle()?),
            },
        )
    };
    let enclosure = RationalEnclosure {
        target,
        context: context.clone(),
        input_family: family.domain.clone(),
        lower,
        upper,
        origin,
    };
    steps.push(ApplicationDraft {
        rule: Primitive::CaseSelect,
        context: context.clone(),
        conclusion: Statement::IntegerSelection(Box::new(IntegerSelection { family, value })),
        premises: vec![cover, correspondence],
        substitutions: Vec::new(),
        certificate: Certificate::IntegerSelection(Box::new(enclosure)),
    });
    Ok(steps)
}

/// Independent finite Machin certificate for the LC cycle box.
pub(super) fn cycle() -> TestResult<PiBoundCertificate> {
    let mut budget = ledger()?;
    Ok(PiBoundCertificate {
        version: 1,
        terms: [2, 1],
        sums: [
            Rational::parse("74/375", &mut budget)?,
            Rational::parse("1/239", &mut budget)?,
        ],
        lower: Rational::parse("640/205", &mut budget)?,
        upper: Rational::parse("640/203", &mut budget)?,
    })
}
