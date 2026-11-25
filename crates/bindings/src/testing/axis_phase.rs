// SPDX-License-Identifier: AGPL-3.0-only
//! Independently declared nominal LC reactance questions for public consumer tests.

use nrz_bindings::{
    AxisPhaseCase, AxisPhaseSpec, Budget, Context, DecisionExpression, Declaration, Descriptor,
    Dimension, Evidence, ExponentPolicy, Kind, Limits, Measure, Observable, Registry, Role,
    readings::algebra::{
        Cas, Handle, Rational, Reference,
        decision::{EnclosureOrigin, RationalEnclosure},
        domain::{
            AmbientDomain, AssumptionDraft, ContextDraft, HeaderDraft, HypothesisRef, RegionDraft,
            RegionExpr,
        },
        syntax::{
            AtomicSort, Declaration as Symbol, Draft, Environment, ExpressionDraft, Field,
            Operator, PredicateKind, Sort,
        },
    },
};
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};

/// Test failures preserve their original typed cause.
pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
/// Unchanged default limits for every setup and cumulative consumer ledger.
pub(super) fn ledger() -> TestResult<Budget> {
    Ok(Budget::new(Limits::default())?)
}
/// Original declaration authority retained independently of the adapter.
pub(super) struct Fixture {
    /// Authoritative CAS graph, used to inspect original retained proof citations.
    pub(super) graph: Arc<Mutex<Cas>>,
    /// Complete physical and mathematical question.
    pub(super) case: AxisPhaseCase,
    /// Exact normalized reactance, computed from independent rational circuit values.
    pub(super) value: Rational,
}

/// Source-free inductive/capacitive reactances retain physical ohm meanings before normalization.
fn physical(inductive: &Rational, capacitive: &Rational) -> TestResult<DecisionExpression> {
    let resistance = Kind::Named(
        "resistance".into(),
        Box::new(Dimension::integers([2, 1, -3, -2, 0, 0, 0])?),
    );
    let ohm = Descriptor::new(resistance, Measure::coherent(), Role::Difference)?;
    let mut owner = Context::with_real_symbols(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        &["inductive", "capacitive", "ohm", "x"],
    )?;
    for (name, value) in [
        ("inductive", inductive.clone()),
        ("capacitive", capacitive.clone()),
        ("ohm", Rational::one()),
    ] {
        owner.declare(
            name.into(),
            Declaration::Constant {
                descriptor: ohm.clone(),
                value,
                evidence: Evidence {
                    source: "fixed LC original reactances".into(),
                    assurance: "exact nominal".into(),
                },
            },
        )?;
    }
    let expression = owner.parse(
        "original normalized LC reactance",
        "(inductive-capacitive)/ohm",
    )?;
    owner.declare(
        "x".into(),
        Declaration::Derived(Observable {
            descriptor: Descriptor::ratio(),
            expression,
            evidence: Evidence::default(),
        }),
    )?;
    Ok(DecisionExpression::declared(
        Rc::new(owner.freeze()?),
        "x",
        &mut ledger()?,
    )?)
}

/// Build the complete original a*L-1/(a*C) target without asking the implementation for a sign.
pub(super) fn fixture(frequency: i64) -> TestResult<Fixture> {
    fixture_with_budget(frequency, &mut ledger()?)
}

/// Measure the new case declaration separately from existing original-source construction.
pub(super) fn fixture_with_budget(frequency: i64, caller: &mut Budget) -> TestResult<Fixture> {
    let mut budget = ledger()?;
    let a = Rational::from_ratio(frequency, 1, &mut budget)?;
    let l = Rational::from_ratio(1, 40, &mut budget)?;
    let c = Rational::from_ratio(1, 100_000, &mut budget)?;
    let inductive = a.mul(&l, &mut budget)?;
    let capacitive = Rational::one().div(&a.mul(&c, &mut budget)?, &mut budget)?;
    let value = inductive.sub(&capacitive, &mut budget)?;
    let source = physical(&inductive, &capacitive)?;
    let mut cas = Cas::new(Limits::default())?;
    let header = admit(
        &mut cas,
        Draft::Header(HeaderDraft::Core(Environment {
            declarations: vec![Symbol {
                key: "x".into(),
                display_name: "original reactance coordinate".into(),
                sort: real(),
            }],
            ..Environment::default()
        })),
    )?;
    let domain = admit(
        &mut cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Real("x".into()),
            expression: RegionExpr::Ambient,
        }),
    )?;
    let coordinate = term(&mut cas, &header, Operator::Free("x".into()), &[])?;
    let a = term(&mut cas, &header, Operator::Rational(a), &[])?;
    let l = term(&mut cas, &header, Operator::Rational(l), &[])?;
    let c = term(&mut cas, &header, Operator::Rational(c), &[])?;
    let one = term(&mut cas, &header, Operator::Rational(Rational::one()), &[])?;
    let first = term(&mut cas, &header, Operator::Mul, &[a.clone(), l])?;
    let denominator = term(&mut cas, &header, Operator::Mul, &[a, c])?;
    let second = term(&mut cas, &header, Operator::Div, &[one, denominator])?;
    let target = term(&mut cas, &header, Operator::Sub, &[first, second])?;
    let equality = admit(
        &mut cas,
        Draft::Condition {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            kind: PredicateKind::Equal,
            arguments: vec![
                Reference::Existing(coordinate.clone()),
                Reference::Existing(target.clone()),
            ],
        },
    )?;
    let context = admit(
        &mut cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header),
            admissible: Reference::Existing(domain),
            assumptions: vec![AssumptionDraft {
                key: "original-reactance".into(),
                condition: Reference::Existing(equality),
            }],
        }),
    )?;
    let spec = AxisPhaseSpec {
        context: context.clone(),
        coordinate,
        target,
        correspondence: HypothesisRef { context, slot: 0 },
    };
    let graph = Arc::new(Mutex::new(cas));
    let case = AxisPhaseCase::declare(Arc::clone(&graph), source, spec, caller)?;
    Ok(Fixture { graph, case, value })
}

/// Explicit rational boxes are independent of the adapter's sign and floating calculations.
pub(super) fn enclosure(
    fixture: &Fixture,
    lower: &str,
    upper: &str,
) -> TestResult<RationalEnclosure> {
    let spec = fixture.case.specification();
    let cas = fixture.graph.lock().map_err(|_| "graph poisoned")?;
    let input_family = cas.admissible_region(
        &spec.context,
        &mut nrz_cas::limits::Budget::new(nrz_cas::limits::Limits::default())?,
    )?;
    drop(cas);
    Ok(RationalEnclosure {
        target: spec.target.clone(),
        context: spec.context.clone(),
        input_family,
        lower: Rational::parse(lower, &mut ledger()?)?,
        upper: Rational::parse(upper, &mut ledger()?)?,
        origin: EnclosureOrigin::ExactRational {
            expression: spec.target.clone(),
            value: fixture.value.clone(),
        },
    })
}

/// A single public checked admission, without unchecked native graph access.
fn admit(cas: &mut Cas, draft: Draft) -> TestResult<Handle> {
    Ok(cas
        .admit(&[draft], &mut ledger()?)?
        .pop()
        .ok_or("missing syntax")?)
}
/// Original scalar syntax retains ordered operands.
fn term(
    cas: &mut Cas,
    header: &Handle,
    operator: Operator,
    operands: &[Handle],
) -> TestResult<Handle> {
    admit(
        cas,
        Draft::Expression(ExpressionDraft {
            operator,
            sort: real(),
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            binding: None,
            space: None,
            operands: operands.iter().cloned().map(Reference::Existing).collect(),
        }),
    )
}
/// One Real scalar coordinate.
const fn real() -> Sort {
    Sort::Value(AtomicSort::Scalar(Field::Real))
}
