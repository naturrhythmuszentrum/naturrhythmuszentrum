// SPDX-License-Identifier: AGPL-3.0-only
//! Complete block reconstruction retains every repeated coordinate and original definition.

use crate::domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
use crate::graph::{Cas, Draft, ExpressionDraft, Handle, Reference};
use crate::limits::Limits;
use crate::proof::{DomainContract, Judgment, Statement};
use crate::readings::{AffineMap, SelectionSpec};
use crate::syntax::{AtomicSort, Declaration, Environment, Extent, Field, Operator, Sort};
use crate::testing::common::{TestResult, default_budget as ledger, real};

fn matrix(rows: u32, columns: u32) -> Sort {
    Sort::Value(AtomicSort::Matrix {
        field: Field::Real,
        rows: Extent::Literal(rows),
        columns: Extent::Literal(columns),
    })
}

fn declarations() -> Vec<(&'static str, Sort)> {
    let mut result = vec![
        ("z", matrix(2, 1)),
        ("block", matrix(2, 2)),
        ("c", Sort::Value(AtomicSort::Scalar(Field::Complex))),
        ("left", real()),
        ("right", real()),
    ];
    result.extend(
        [
            "p_left", "r_left", "e_left", "q_left", "p_right", "r_right", "e_right", "q_right",
        ]
        .map(|key| (key, matrix(1, 1))),
    );
    result
}

fn setup() -> TestResult<(Cas, Vec<Handle>)> {
    let mut cas = Cas::new(Limits::default())?;
    let declarations = declarations();
    let environment = Environment {
        declarations: declarations
            .iter()
            .map(|(key, sort)| Declaration {
                key: (*key).into(),
                display_name: "same display".into(),
                sort: sort.clone(),
            })
            .collect(),
        ..Environment::default()
    };
    let mut drafts = vec![
        Draft::Header(HeaderDraft::Core(environment)),
        Draft::Region(RegionDraft {
            header: Reference::Local(0),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::Ambient,
        }),
        Draft::Context(ContextDraft::Root {
            header: Reference::Local(0),
            admissible: Reference::Local(1),
            assumptions: Vec::new(),
        }),
    ];
    drafts.extend(declarations.into_iter().map(|(key, sort)| {
        Draft::Expression(ExpressionDraft {
            header: Reference::Local(0),
            operator: Operator::Free(key.into()),
            sort,
            scope: Vec::new(),
            operands: Vec::new(),
            space: None,
            binding: None,
        })
    }));
    let handles = cas.admit(&drafts, &mut ledger()?)?;
    Ok((cas, handles))
}

fn map(inputs: u32, outputs: u32, values: &[i64]) -> TestResult<AffineMap> {
    let coefficients = values
        .iter()
        .map(|value| crate::Rational::from_ratio(*value, 1, &mut ledger()?))
        .collect::<Result<Vec<_>, crate::Fault>>()?;
    Ok(AffineMap::new(
        inputs,
        outputs,
        coefficients,
        vec![crate::Rational::zero(); usize::try_from(outputs)?],
        &mut ledger()?,
    )?)
}

fn fixture() -> TestResult<(Cas, Judgment)> {
    let (mut cas, handles) = setup()?;
    let context = handles.get(2).ok_or("context")?.clone();
    let source = handles.get(3).ok_or("source")?;
    let symbol = handles.get(4).ok_or("block")?;
    let expansion = map(2, 4, &[1, 0, 0, 1, 0, -1, 1, 0])?;
    let block = cas.define_reading(symbol, source, &expansion, &mut ledger()?)?;
    let left = cas.select_entries(
        &block,
        &SelectionSpec::Block {
            row: 0,
            column: 0,
            rows: 2,
            columns: 2,
        },
        &mut ledger()?,
    )?;
    let value = cas.define_reading(
        handles.get(5).ok_or("complex")?,
        &left,
        &map(4, 2, &[1, 0, 0, 0, 0, 1, 0, 0])?,
        &mut ledger()?,
    )?;
    let right = cas.define_reading(symbol, &value, &expansion, &mut ledger()?)?;
    let domain = cas.admissible_region(&context)?;
    Ok((
        cas,
        Judgment {
            context,
            layer: crate::domain::Layer::Pointwise,
            space: None,
            residuals: Vec::new(),
            statement: Statement::Equality {
                left,
                right,
                domain,
                domain_contract: DomainContract::DefinedAndEqual,
            },
        },
    ))
}

/// Record separate exact proof, fresh replay and authenticated inventory costs.
/// ´test:crate:expanded-representation-costs´
#[test]
fn expanded_representation_costs() -> TestResult {
    let (mut cas, claim) = fixture()?;
    let Statement::Equality { left, right, .. } = &claim.statement else {
        return Err("representation equality".into());
    };
    let mut kernel = ledger()?;
    assert!(crate::readings::check::equal(
        &cas.state,
        left.logical_id(),
        right.logical_id(),
        &mut kernel,
    )?);
    let mut checking = ledger()?;
    let witness = cas.prove_reading_equality(&claim.context, left, right, &mut checking)?;
    let mut inventory = ledger()?;
    let contents = cas.witness_dependencies(&witness, &mut inventory)?;
    let bytes = cas.export_witness(&witness, &mut ledger()?)?;
    let (mut receiver, expected) = fixture()?;
    let mut replay = ledger()?;
    let (imported, obligations) = crate::testing::replay_answer::measured(|| {
        Ok(receiver.replay(&bytes, &expected, &mut replay)?)
    })?;
    let rules: Vec<_> = obligations
        .iter()
        .map(|row| {
            (
                row.rule.specification().key,
                row.after.work - row.before.work,
            )
        })
        .collect();
    assert_eq!(
        receiver
            .witness_dependencies(&imported, &mut ledger()?)?
            .contents(),
        contents.contents()
    );
    assert_eq!(
        (
            kernel.usage().work,
            checking.usage().work,
            replay.usage().work,
            inventory.usage().work,
            bytes.len(),
            rules
        ),
        (
            121_700,
            170_357,
            256_123,
            509_124,
            1_732,
            vec![("poly.jets-equal", 124_915)]
        )
    );
    Ok(())
}

fn limited(kind: crate::limits::BudgetKind, limit: u64) -> TestResult<crate::limits::Budget> {
    let mut limits = Limits::default();
    match kind {
        crate::limits::BudgetKind::Work => limits.work = limit,
        crate::limits::BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = limit,
        crate::limits::BudgetKind::OutputBytes => limits.output_bytes = limit,
        _ => return Err("expanded inventory boundary".into()),
    }
    Ok(crate::limits::Budget::new(limits)?)
}

/// Complete expanded inventories retain exact cumulative Work and simultaneous payload boundaries.
/// ´test:crate:expanded-representation-inventory-boundaries´
#[test]
fn expanded_representation_inventory_boundaries() -> TestResult {
    use crate::limits::BudgetKind;
    let (mut cas, claim) = fixture()?;
    let Statement::Equality { left, right, .. } = &claim.statement else {
        return Err("representation equality".into());
    };
    let witness = cas.prove_reading_equality(&claim.context, left, right, &mut ledger()?)?;
    let before = cas.counts();
    let mut measured = ledger()?;
    let expected = cas.witness_dependencies(&witness, &mut measured)?;
    for (kind, needed) in [
        (BudgetKind::Work, 509_124),
        (BudgetKind::StagedPayloadBytes, 61_028),
        (BudgetKind::OutputBytes, 23_380),
    ] {
        let actual = cas.witness_dependencies(&witness, &mut limited(kind, needed)?)?;
        assert_eq!(actual.contents(), expected.contents());
        let error = cas
            .witness_dependencies(&witness, &mut limited(kind, needed - 1)?)
            .err()
            .ok_or("short inventory budget accepted")?;
        assert!(matches!(error.kind, crate::FaultKind::Exhausted { budget, .. } if budget == kind));
        assert_eq!(cas.counts(), before);
    }
    let mut cumulative = limited(BudgetKind::Work, measured.usage().work)?;
    cumulative.work(1, crate::Location::new("prior inventory work"))?;
    assert!(cas.witness_dependencies(&witness, &mut cumulative).is_err());
    Ok(())
}
