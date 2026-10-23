// SPDX-License-Identifier: AGPL-3.0-only
//! Independent proof-plan extraction measures checking and fresh replay per witness family.

use super::{SourceCoordinates, SourceJacobian};
use crate::budget::{Budget, Limits};
use crate::domain::{
    AmbientDomain, AssumptionDraft, ContextDraft, HeaderDraft, RegionDraft, RegionExpr,
};
use crate::graph::{Cas, Draft, Handle, LogicalId, Reference};
use crate::model::{AtomicSort, Declaration, Environment, Field, Operator, PredicateKind, Sort};
use crate::proof_model::{ApplicationDraft, VerifiedWitness};
use std::collections::BTreeMap;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
type Measurement = ([u64; 3], Vec<(String, u64)>);

fn limits() -> Limits {
    Limits {
        work: 100_000_000,
        replay_work: 100_000_000,
        ..Limits::default()
    }
}

fn ledger() -> TestResult<Budget> {
    Ok(Budget::new(limits())?)
}

fn admit(cas: &mut Cas, draft: Draft) -> TestResult<Handle> {
    cas.admit(&[draft], &mut ledger()?)?
        .pop()
        .ok_or_else(|| "measurement fixture".into())
}

fn declaration_header(cas: &mut Cas) -> TestResult<Handle> {
    let declarations = ["L", "C", "x", "y"]
        .into_iter()
        .map(|key| Declaration {
            key: key.into(),
            display_name: key.into(),
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
        })
        .collect();
    admit(
        cas,
        Draft::Header(HeaderDraft::Core(Environment {
            declarations,
            ..Environment::default()
        })),
    )
}

fn scalar(
    cas: &mut Cas,
    header: &Handle,
    operator: Operator,
    operands: &[LogicalId],
) -> TestResult<LogicalId> {
    Ok(cas.generated(
        header.logical_id(),
        operator,
        Sort::Value(AtomicSort::Scalar(Field::Real)),
        operands,
        &mut ledger()?,
    )?)
}

fn fixture_context(cas: &mut Cas, header: &Handle, positive: bool) -> TestResult<Handle> {
    let region = admit(
        cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::Ambient,
        }),
    )?;
    let mut assumptions = Vec::new();
    if positive {
        for key in ["L", "C"] {
            let term = scalar(cas, header, Operator::Free(key.into()), &[])?;
            let condition = admit(
                cas,
                Draft::Condition {
                    header: Reference::Existing(header.clone()),
                    scope: Vec::new(),
                    kind: PredicateKind::Positive,
                    arguments: vec![Reference::Existing(cas.handle(term))],
                },
            )?;
            assumptions.push(AssumptionDraft {
                key: key.into(),
                condition: Reference::Existing(condition),
            });
        }
    }
    admit(
        cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header.clone()),
            admissible: Reference::Existing(region),
            assumptions,
        }),
    )
}

fn matrix(cas: &mut Cas, context: &Handle, values: &[&str]) -> TestResult<Handle> {
    let header = cas.context_header(context)?;
    let mut entries = Vec::new();
    for text in values {
        let value = crate::Rational::parse(text, &mut ledger()?)?;
        entries.push(scalar(cas, &header, Operator::Rational(value), &[])?);
    }
    Ok(cas.jacobian_rectangle(context, 2, 2, &entries, &mut ledger()?)?)
}

fn lc(cas: &mut Cas, context: &Handle) -> TestResult<Handle> {
    let header = cas.context_header(context)?;
    let zero = scalar(
        cas,
        &header,
        Operator::Rational(crate::Rational::zero()),
        &[],
    )?;
    let one = scalar(
        cas,
        &header,
        Operator::Rational(crate::Rational::one()),
        &[],
    )?;
    let inductance = scalar(cas, &header, Operator::Free("L".into()), &[])?;
    let capacitance = scalar(cas, &header, Operator::Free("C".into()), &[])?;
    let upper = scalar(cas, &header, Operator::Div, &[one, capacitance])?;
    let reciprocal = scalar(cas, &header, Operator::Div, &[one, inductance])?;
    let lower = scalar(cas, &header, Operator::Neg, &[reciprocal])?;
    let physical =
        cas.jacobian_rectangle(context, 2, 2, &[zero, upper, lower, zero], &mut ledger()?)?;
    let state = matrix(cas, context, &["5", "0", "0", "1/10"])?;
    let inverse = matrix(cas, context, &["1/5", "0", "0", "10"])?;
    let time = matrix(cas, context, &["1/2000", "0", "0", "1/2000"])?;
    let right = product(cas, &physical, &state)?;
    let similar = product(cas, &inverse, &right)?;
    product(cas, &time, &similar)
}

fn product(cas: &mut Cas, left: &Handle, right: &Handle) -> TestResult<Handle> {
    let header = cas.state.expression(left.logical_id())?.header;
    let sort = cas.state.expression(left.logical_id())?.sort.clone();
    let id = cas.generated(
        header,
        Operator::MatrixMultiply,
        sort,
        &[left.logical_id(), right.logical_id()],
        &mut ledger()?,
    )?;
    Ok(cas.handle(id))
}

fn fixture(matrix: bool) -> TestResult<(Cas, SourceJacobian)> {
    let mut cas = Cas::new(limits())?;
    let header = declaration_header(&mut cas)?;
    let context = fixture_context(&mut cas, &header, matrix)?;
    let original = if matrix {
        lc(&mut cas, &context)?
    } else {
        let x = scalar(&mut cas, &header, Operator::Free("x".into()), &[])?;
        let y = scalar(&mut cas, &header, Operator::Free("y".into()), &[])?;
        let value = scalar(&mut cas, &header, Operator::Mul, &[x, y])?;
        cas.handle(value)
    };
    let sources = if matrix {
        vec!["L".into(), "C".into()]
    } else {
        vec!["x".into(), "y".into()]
    };
    let key = cas.source_jacobian_key(
        &context,
        &original,
        &SourceCoordinates {
            sources,
            fixed: Vec::new(),
        },
        &mut ledger()?,
    )?;
    let derivative = cas.source_jacobian(&key, &mut ledger()?)?;
    Ok((cas, derivative))
}

fn composition(cas: &mut Cas, original: &SourceJacobian) -> TestResult<super::ComposedJacobian> {
    let context = &original.key.context;
    let header = cas.context_header(context)?;
    let mut values = Vec::new();
    for text in ["2", "3", "5", "7", "11", "13"] {
        let value = crate::Rational::parse(text, &mut ledger()?)?;
        let id = scalar(cas, &header, Operator::Rational(value), &[])?;
        values.push(cas.handle(id));
    }
    let map = cas.exact_reading_map(context, (2, 2), &values[..4], &values[4..], &mut ledger()?)?;
    let readings = super::JacobianReadings {
        variables: vec!["x".into(), "y".into()],
        map,
    };
    let coordinates = SourceCoordinates {
        sources: vec!["L".into(), "C".into()],
        fixed: Vec::new(),
    };
    Ok(cas.compose_reading_jacobian(
        context,
        original.key.original(),
        &coordinates,
        &readings,
        &mut ledger()?,
    )?)
}

fn proposals(cas: &Cas, witness: &VerifiedWitness) -> TestResult<Vec<ApplicationDraft>> {
    let mut positions = BTreeMap::new();
    let mut pending = vec![(witness.proof().logical_id(), false)];
    let mut result = Vec::new();
    while let Some((id, ready)) = pending.pop() {
        if positions.contains_key(&id) {
            continue;
        }
        let application = crate::proof_view::application(cas, id, &mut ledger()?)?;
        if !ready {
            pending.push((id, true));
            pending.extend(
                application
                    .premises
                    .iter()
                    .rev()
                    .map(|child| (*child, false)),
            );
            continue;
        }
        let mut premises = Vec::new();
        for premise in application.premises {
            premises.push(*positions.get(&premise).ok_or("earlier proof")?);
        }
        positions.insert(id, u32::try_from(result.len())?);
        result.push(ApplicationDraft {
            rule: application.rule,
            context: application.judgment.context,
            conclusion: application.judgment.statement,
            premises,
            substitutions: application.substitutions,
            certificate: application.certificate,
        });
    }
    Ok(result)
}

fn measured(cas: &mut Cas, witness: &VerifiedWitness) -> TestResult<Measurement> {
    let steps = proposals(cas, witness)?;
    let mut counts = BTreeMap::new();
    for step in &steps {
        *counts.entry(format!("{:?}", step.rule)).or_insert(0) += 1;
    }
    let mut proof = ledger()?;
    cas.check_proof(&witness.judgment().context, &steps, &mut proof)?;
    let mut replay = ledger()?;
    cas.replay_jacobian_fact(witness, &mut replay)?;
    Ok((
        [
            proof.usage().work,
            replay.usage().work,
            u64::try_from(witness.bytes.len())?,
        ],
        counts.into_iter().collect(),
    ))
}

fn expected_rules(index: usize) -> Vec<(String, u64)> {
    let entries: &[(&str, u64)] = match index {
        0 => &[
            ("DerivativeTransport", 2),
            ("DifferentiateProduct", 2),
            ("DifferentiateVariable", 4),
            ("Jacobian", 1),
            ("RationalIdentity", 2),
        ],
        1 => &[
            ("DerivativeTransport", 4),
            ("DifferentiateConstant", 6),
            ("DifferentiateQuotient", 4),
            ("DifferentiateVariable", 4),
            ("Jacobian", 1),
            ("RationalIdentity", 4),
        ],
        3 => &[
            ("DerivativeTransport", 18),
            ("DifferentiateAdd", 8),
            ("DifferentiateConstant", 12),
            ("DifferentiateProduct", 10),
            ("DifferentiateVariable", 4),
            ("Jacobian", 1),
            ("RationalIdentity", 17),
        ],
        _ => &[("Jets", 1)],
    };
    entries
        .iter()
        .map(|(name, count)| ((*name).into(), *count))
        .collect()
}

fn expected_measurements() -> Vec<Measurement> {
    let costs = [
        [163_837, 300_597, 1_733],
        [538_827, 944_982, 3_494],
        [258_370, 503_985, 1_801],
        [1_724_796, 2_931_445, 8_042],
        [51_269, 144_846, 828],
        [287_456, 605_647, 2_137],
    ];
    costs
        .into_iter()
        .enumerate()
        .map(|(index, cost)| (cost, expected_rules(index)))
        .collect()
}

/// Every retained witness has separately measured kernel checking, fresh replay and per-rule counts.
/// ´test:unit:source-jacobian-proof-and-replay-measurements´
#[test]
fn source_jacobian_proof_and_replay_measurements() -> TestResult {
    let (mut scalar_store, first) = fixture(false)?;
    let (mut matrix_store, second) = fixture(true)?;
    let composed = composition(&mut scalar_store, &first)?;
    let rows = vec![
        measured(&mut scalar_store, first.witness())?,
        measured(&mut matrix_store, second.witness())?,
        measured(
            &mut matrix_store,
            second.representation().ok_or("matrix representation")?,
        )?,
        measured(&mut scalar_store, composed.direct().witness())?,
        measured(&mut scalar_store, composed.map_representation())?,
        measured(&mut scalar_store, composed.equality())?,
    ];
    assert_eq!(rows, expected_measurements());
    Ok(())
}
