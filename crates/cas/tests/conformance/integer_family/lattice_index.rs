// SPDX-License-Identifier: AGPL-3.0-only
//! Cartesian lattice ranges consume existing scalar family proofs in their original parent scope.

#[path = "lattice_cover.rs"]
mod lattice_cover;

use super::{Frame, admit, bound_guard, ledger, real};
use crate::testing::builders::environment;
use crate::testing::common::TestResult;
use crate::testing::readings::matrix;
use nrz_cas::decision::{IntegerCellRange, IntegerFamily};
use nrz_cas::domain::{
    AmbientDomain, AssumptionDraft, ContextDraft, HeaderDraft, HypothesisRef, RegionDraft,
    RegionExpr,
};
use nrz_cas::lattice::{
    CheckedLatticeBasis, LatticeBasisOutcome, LatticeBasisQuestion, LatticeIndexQuestion,
};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::proof::{ApplicationDraft, Certificate, Primitive, Statement, VerifiedWitness};
use nrz_cas::syntax::{AtomicSort, Draft, Field, Operator, PredicateKind, Sort};
use nrz_cas::{Cas, FaultKind, Handle, InvalidCode, Rational, Reference};

struct Product {
    frame: Frame,
    basis: Box<CheckedLatticeBasis>,
    question: LatticeIndexQuestion,
    covers: Vec<VerifiedWitness>,
}

fn integer(frame: &mut Frame, value: i64) -> TestResult<Handle> {
    frame.term(
        Operator::Rational(Rational::from_ratio(value, 1, &mut ledger()?)?),
        Sort::Value(AtomicSort::Integer),
        &[],
    )
}

fn frame(parameters: bool) -> TestResult<Frame> {
    let names = if parameters {
        &["q", "n", "a", "b"][..]
    } else {
        &["q", "n", "r", "m"][..]
    };
    named_frame(names)
}

fn named_frame(names: &[&str]) -> TestResult<Frame> {
    let mut cas = Cas::new(Limits::default())?;
    let mut budget = ledger()?;
    let mut declarations = environment(Field::Real, names);
    for declaration in &mut declarations.declarations {
        if !matches!(declaration.key.as_str(), "q" | "r" | "s") {
            declaration.sort = Sort::Value(AtomicSort::Integer);
        }
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
    Ok(Frame {
        cas,
        header,
        ambient,
        budget,
    })
}

fn parameter_bindings(frame: &mut Frame, lower: i64) -> TestResult<(Vec<Handle>, Vec<Handle>)> {
    let mut endpoints = Vec::new();
    let mut predicates = Vec::new();
    for (key, value) in [("a", lower), ("b", 5)] {
        let parameter = frame.term(
            Operator::Free(key.into()),
            Sort::Value(AtomicSort::Integer),
            &[],
        )?;
        let value = integer(frame, value)?;
        let predicate = admit(
            &mut frame.cas,
            Draft::Condition {
                header: Reference::Existing(frame.header.clone()),
                scope: Vec::new(),
                kind: PredicateKind::Equal,
                arguments: vec![
                    Reference::Existing(parameter.clone()),
                    Reference::Existing(value),
                ],
            },
            &mut ledger()?,
        )?;
        endpoints.push(parameter);
        predicates.push(predicate);
    }
    Ok((endpoints, predicates))
}

fn assumptions(predicates: &[Handle]) -> Vec<AssumptionDraft> {
    predicates
        .iter()
        .enumerate()
        .map(|(index, predicate)| AssumptionDraft {
            key: format!("endpoint-{index}"),
            condition: Reference::Existing(predicate.clone()),
        })
        .collect()
}

fn basis(
    frame: &mut Frame,
    context: &Handle,
    count: u32,
    dependent: bool,
) -> TestResult<Box<CheckedLatticeBasis>> {
    let zero = frame.term(Operator::Rational(Rational::zero()), real(), &[])?;
    let one = frame.term(Operator::Rational(Rational::one()), real(), &[])?;
    let mut entries = Vec::new();
    for row in 0..count {
        for column in 0..count {
            entries.push(if dependent && (row, column) == (0, 1) {
                frame.term(Operator::Free("q".into()), real(), &[])?
            } else if row == column {
                one.clone()
            } else {
                zero.clone()
            });
        }
    }
    let matrix = frame.term(
        Operator::Matrix {
            rows: count,
            columns: count,
        },
        matrix(count, count),
        &entries,
    )?;
    let offset = frame.term(
        Operator::Matrix {
            rows: count,
            columns: 1,
        },
        crate::testing::readings::matrix(count, 1),
        &vec![zero; usize::try_from(count)?],
    )?;
    let question = LatticeBasisQuestion {
        context: context.clone(),
        basis: matrix,
        offset,
    };
    match frame.cas.admit_lattice_basis(&question, &mut ledger()?)? {
        LatticeBasisOutcome::Admitted(value) => Ok(value),
        LatticeBasisOutcome::Unresolved => Err("fixture basis unresolved".into()),
    }
}

struct Factor<'a> {
    keys: (&'a str, &'a str),
    endpoints: [Option<Handle>; 2],
    nonnegative: bool,
    bindings: &'a [Handle],
    local: bool,
}

fn factor(
    frame: &mut Frame,
    parent: &Handle,
    spec: Factor<'_>,
) -> TestResult<(IntegerFamily, VerifiedWitness)> {
    let q = frame.term(Operator::Free(spec.keys.0.into()), real(), &[])?;
    let n = frame.term(
        Operator::Free(spec.keys.1.into()),
        Sort::Value(AtomicSort::Integer),
        &[],
    )?;
    let embedded = frame.term(Operator::IntegerEmbed, real(), std::slice::from_ref(&n))?;
    let one = frame.term(Operator::Rational(Rational::one()), real(), &[])?;
    let end = frame.term(Operator::Add, real(), &[embedded.clone(), one])?;
    let left = frame.term(Operator::Sub, real(), &[q.clone(), embedded])?;
    let right = frame.term(Operator::Sub, real(), &[end, q.clone()])?;
    let mut relation = vec![
        frame.guard(PredicateKind::NonNegative, &[left])?,
        frame.guard(PredicateKind::Positive, &[right])?,
    ];
    let mut projected = Vec::new();
    for (position, bound) in spec.endpoints.iter().enumerate() {
        if let Some(bound) = bound {
            relation.push(bound_guard(frame, &n, bound, position == 1, false)?);
            projected.push(bound_guard(frame, &q, bound, position == 1, true)?);
        }
    }
    if spec.nonnegative {
        let zero = integer(frame, 0)?;
        relation.push(bound_guard(frame, &n, &zero, false, false)?);
        projected.push(bound_guard(frame, &q, &zero, false, true)?);
    }
    let relation = frame.region(RegionExpr::Meet(
        relation.into_iter().map(Reference::Existing).collect(),
    ))?;
    let projected = frame.region(RegionExpr::Meet(
        projected.into_iter().map(Reference::Existing).collect(),
    ))?;
    let context = admit(
        &mut frame.cas,
        Draft::Context(ContextDraft::Local {
            parent: Reference::Existing(parent.clone()),
            region: Reference::Existing(projected),
            hypotheses: if spec.local {
                assumptions(spec.bindings)
            } else {
                Vec::new()
            },
        }),
        &mut ledger()?,
    )?;
    let domain = frame.cas.admissible_region(&context)?;
    let (function, _) = frame.cases(&relation, n.clone())?;
    let [lower, upper] = spec.endpoints;
    let family = IntegerFamily {
        function,
        coordinate: q.clone(),
        index: n,
        domain: domain.clone(),
        lower,
        upper,
        count: Some(Box::new(IntegerCellRange {
            coordinate: q,
            lower: spec.nonnegative.then(Rational::zero),
            upper: None,
        })),
    };
    let mut steps = Vec::new();
    for (index, predicate) in spec.bindings.iter().enumerate() {
        steps.push(ApplicationDraft {
            rule: Primitive::ContextHypothesis,
            context: context.clone(),
            conclusion: Statement::PredicateFact {
                region: domain.clone(),
                predicate: predicate.clone(),
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::Hypothesis(HypothesisRef {
                context: if spec.local {
                    context.clone()
                } else {
                    parent.clone()
                },
                slot: u32::try_from(index)? + u32::from(spec.local),
            }),
        });
    }
    steps.push(ApplicationDraft {
        rule: Primitive::RegionDecide,
        context: context.clone(),
        conclusion: Statement::IntegerFamily(Box::new(family.clone())),
        premises: (0..spec.bindings.len())
            .map(u32::try_from)
            .collect::<Result<Vec<_>, _>>()?,
        substitutions: Vec::new(),
        certificate: Certificate::None,
    });
    let proof = frame.cas.check_proof(&context, &steps, &mut ledger()?)?;
    Ok((family, proof))
}

fn fixture(kind: u32, lower: i64, local: bool, dependent: bool) -> TestResult<Product> {
    let parameters = kind == 2;
    let count = if parameters {
        1
    } else if kind == 3 {
        3
    } else {
        2
    };
    let mut frame = if kind == 3 {
        named_frame(&["q", "n", "r", "m", "s", "l"])?
    } else {
        frame(parameters)?
    };
    let (endpoints, predicates) = if parameters {
        parameter_bindings(&mut frame, lower)?
    } else {
        (Vec::new(), Vec::new())
    };
    let parent = admit(
        &mut frame.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(frame.header.clone()),
            admissible: Reference::Existing(frame.ambient.clone()),
            assumptions: if local {
                Vec::new()
            } else {
                assumptions(&predicates)
            },
        }),
        &mut ledger()?,
    )?;
    let basis = basis(&mut frame, &parent, count, dependent)?;
    let mut families = Vec::new();
    let mut covers = Vec::new();
    for position in 0..count {
        let bounds = match kind {
            0 => [None, None],
            1 => [
                Some(integer(&mut frame, if position == 0 { 0 } else { -2 })?),
                Some(integer(&mut frame, if position == 0 { 2 } else { 3 })?),
            ],
            _ => [endpoints.first().cloned(), endpoints.get(1).cloned()],
        };
        let spec = Factor {
            keys: match position {
                0 => ("q", "n"),
                1 => ("r", "m"),
                _ => ("s", "l"),
            },
            endpoints: bounds,
            nonnegative: parameters,
            bindings: &predicates,
            local,
        };
        let (family, proof) = factor(&mut frame, &parent, spec)?;
        families.push(family);
        covers.push(proof);
    }
    let coordinate_domain = frame.region(RegionExpr::Meet(
        families
            .iter()
            .map(|family| Reference::Existing(family.domain.clone()))
            .collect(),
    ))?;
    let question = LatticeIndexQuestion {
        basis: basis.question().clone(),
        families,
        coordinate_domain,
    };
    Ok(Product {
        frame,
        basis,
        question,
        covers,
    })
}

/// Unbounded products, signed Integer boxes and exact parameter/count intersections retain scalar proofs.
/// ´test:integration:lattice-index-domains´
#[test]
fn lattice_index_domains() -> TestResult {
    for kind in 0..3 {
        let source = fixture(kind, 2, false, false)?;
        let before = source.frame.cas.counts();
        let checked = source.frame.cas.admit_lattice_index_domain(
            &source.basis,
            &source.question,
            &source.covers,
            &mut ledger()?,
        )?;
        assert_eq!(checked.question(), &source.question);
        assert_eq!(checked.scalar_proofs().len(), source.covers.len());
        assert_eq!(source.frame.cas.counts(), before);
        for (proof, cover) in checked.scalar_proofs().iter().zip(&source.covers) {
            assert_eq!(proof, cover.proof());
            assert_eq!(cover.hypotheses().len(), if kind == 2 { 2 } else { 0 });
        }
    }
    Ok(())
}

fn refuses(
    source: &Product,
    question: &LatticeIndexQuestion,
    covers: &[VerifiedWitness],
) -> TestResult {
    let before = source.frame.cas.counts();
    let fault = source
        .frame
        .cas
        .admit_lattice_index_domain(&source.basis, question, covers, &mut ledger()?)
        .err()
        .ok_or("invalid product admitted")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::DecisionIntegerRange)
    );
    assert_eq!(source.frame.cas.counts(), before);
    Ok(())
}

/// Widened domains, omitted factors or proofs and repeated binders refuse instead of clamping ranges.
/// ´test:integration:lattice-index-refusals´
#[test]
fn lattice_index_refusals() -> TestResult {
    let mut source = fixture(1, 2, false, false)?;
    let mut widened = source.question.clone();
    widened.coordinate_domain.clone_from(&source.frame.ambient);
    refuses(&source, &widened, &source.covers)?;
    let mut omitted = source.question.clone();
    omitted.coordinate_domain =
        source
            .frame
            .region(RegionExpr::Meet(vec![Reference::Existing(
                source.question.families[0].domain.clone(),
            )]))?;
    refuses(&source, &omitted, &source.covers)?;
    refuses(&source, &source.question, &source.covers[..1])?;
    let mut changed = source.question.clone();
    changed.families[0].upper = None;
    refuses(&source, &changed, &source.covers)?;
    let mut repeated = source.question.clone();
    repeated.families[1] = repeated.families[0].clone();
    refuses(
        &source,
        &repeated,
        &[source.covers[0].clone(), source.covers[0].clone()],
    )?;
    Ok(())
}

/// Scalar range evidence cannot introduce local parameter assumptions or let a basis depend on its coordinates.
/// ´test:integration:lattice-index-scope´
#[test]
fn lattice_index_scope() -> TestResult {
    let local = fixture(2, 2, true, false)?;
    refuses(&local, &local.question, &local.covers)?;
    let coupled = fixture(1, 2, false, true)?;
    refuses(&coupled, &coupled.question, &coupled.covers)?;
    let mut source = fixture(2, 2, false, false)?;
    let context = source.covers[0].judgment().context.clone();
    let missing = ApplicationDraft {
        rule: Primitive::RegionDecide,
        context: context.clone(),
        conclusion: Statement::IntegerFamily(Box::new(source.question.families[0].clone())),
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    };
    let before = source.frame.cas.counts();
    let fault = source
        .frame
        .cas
        .check_proof(&context, &[missing], &mut ledger()?)
        .err()
        .ok_or("missing endpoint bindings accepted")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::DecisionIntegerRange)
    );
    assert_eq!(source.frame.cas.counts(), before);
    Ok(())
}

/// Fresh scalar replay and product reconstruction retain the original parameter context and endpoint bindings.
/// ´test:integration:lattice-index-replay´
#[test]
fn lattice_index_replay() -> TestResult {
    let source = fixture(2, 2, false, false)?;
    let bytes = source
        .frame
        .cas
        .export_witness(&source.covers[0], &mut ledger()?)?;
    let mut receiver = fixture(2, 2, false, false)?;
    let expected = receiver.covers[0].judgment().clone();
    let replayed = receiver
        .frame
        .cas
        .replay(&bytes, &expected, &mut ledger()?)?;
    assert_eq!(replayed.hypotheses().len(), 2);
    receiver.covers = vec![replayed];
    receiver.frame.cas.admit_lattice_index_domain(
        &receiver.basis,
        &receiver.question,
        &receiver.covers,
        &mut ledger()?,
    )?;
    let mut changed = fixture(2, 3, false, false)?;
    let expected = changed.covers[0].judgment().clone();
    let before = changed.frame.cas.counts();
    let fault = changed
        .frame
        .cas
        .replay(&bytes, &expected, &mut ledger()?)
        .err()
        .ok_or("changed parameter context accepted")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(changed.frame.cas.counts(), before);
    Ok(())
}

fn bounded(kind: BudgetKind, amount: u64) -> TestResult<Budget> {
    let mut limits = Limits::default();
    match kind {
        BudgetKind::Work => limits.work = amount,
        BudgetKind::OutputBytes => limits.output_bytes = amount,
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = amount,
        BudgetKind::ConditionAtoms => limits.condition_atoms = amount,
        _ => return Err("unsupported range budget fixture".into()),
    }
    Ok(Budget::new(limits)?)
}

fn boundary(
    source: &Product,
    kind: BudgetKind,
    amount: u64,
    prior: u64,
    retained: u64,
) -> TestResult<bool> {
    let before = source.frame.cas.counts();
    let snapshot = source.frame.cas.canonical_snapshot(&mut ledger()?)?;
    let mut budget = bounded(kind, amount)?;
    let limits = budget.limits().clone();
    budget.work(prior, nrz_cas::Location::new("earlier range work"))?;
    let result = budget.with_retained_payload(
        retained,
        nrz_cas::Location::new("range caller data"),
        |budget| {
            source.frame.cas.admit_lattice_index_domain(
                &source.basis,
                &source.question,
                &source.covers,
                budget,
            )
        },
    );
    assert_eq!(budget.limits(), &limits);
    assert!(budget.usage().work >= prior);
    assert_eq!(source.frame.cas.counts(), before);
    assert_eq!(
        source.frame.cas.canonical_snapshot(&mut ledger()?)?,
        snapshot
    );
    match result {
        Ok(_) => Ok(true),
        Err(fault) => {
            assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind));
            Ok(false)
        }
    }
}

/// Exact measured Work boundaries preserve prior usage and leave the graph unchanged on refusal.
/// ´test:integration:lattice-index-work´
#[test]
fn lattice_index_work() -> TestResult {
    for (kind, needed) in [(0, 1_356), (1, 1_425), (2, 921)] {
        let source = fixture(kind, 2, false, false)?;
        assert!(boundary(&source, BudgetKind::Work, needed, 0, 0)?);
        assert!(!boundary(&source, BudgetKind::Work, needed - 1, 0, 0)?);
        assert!(!boundary(&source, BudgetKind::Work, needed, 37, 0)?);
        assert!(boundary(&source, BudgetKind::Work, needed + 37, 37, 0)?);
    }
    Ok(())
}

/// Literal metadata, staged-payload and traversal boundaries include retained caller data.
/// ´test:integration:lattice-index-capacities´
#[test]
fn lattice_index_capacities() -> TestResult {
    let source = fixture(2, 2, false, false)?;
    for (kind, needed) in [
        (BudgetKind::OutputBytes, 769),
        (BudgetKind::StagedPayloadBytes, 3_633),
        (BudgetKind::ConditionAtoms, 10),
    ] {
        assert!(boundary(&source, kind, needed, 0, 0)?);
        assert!(!boundary(&source, kind, needed - 1, 0, 0)?);
    }
    assert!(boundary(
        &source,
        BudgetKind::StagedPayloadBytes,
        3_890,
        37,
        257
    )?);
    assert!(!boundary(
        &source,
        BudgetKind::StagedPayloadBytes,
        3_889,
        37,
        257
    )?);
    Ok(())
}
