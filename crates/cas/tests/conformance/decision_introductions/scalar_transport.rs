// SPDX-License-Identifier: AGPL-3.0-only
//! Certified restriction edges use the same guarded-region inclusion argument.

use super::{Example, frame_draft, prepared_introduction};
use crate::testing::builders::admit;
use crate::testing::common::TestResult;
use crate::testing::readings::ledger;
use crate::testing::witness::equality;
use nrz_cas::Reference;
use nrz_cas::domain::{RegionDraft, RegionExpr};
use nrz_cas::proof::{ApplicationDraft, Certificate, Primitive, Statement};
use nrz_cas::syntax::{Draft, Operator};

/// A retained decision introduction does not exclude its ordinary canonical restriction edge.
/// ´test:integration:scalar-transport-certified-edge´
#[test]
fn scalar_transport_certified_edge() -> TestResult {
    crossing(false)?;
    crossing(true)
}

fn crossing(conditional: bool) -> TestResult {
    let mut f = prepared_introduction(Example::Sign, true)?;
    let intro = f.partition.source.cas.check_proof(
        &f.partition.source.context,
        &f.steps,
        &mut ledger()?,
    )?;
    let draft = frame_draft(&f, &intro);
    let coordinate = f.partition.question.coordinates[0].clone();
    let source = &mut f.partition.source;
    let frame = admit(&mut source.cas, draft, &mut ledger()?)?;
    let domain = source
        .cas
        .admissible_region(&frame, &mut crate::testing::budgets::graph_budget()?)?;
    let original = source.cas.admissible_region(
        &source.context,
        &mut crate::testing::budgets::graph_budget()?,
    )?;
    let one = source.number(1, 1)?;
    let two = source.number(2, 1)?;
    let (quotient, half, rule) = if conditional {
        (
            source.term(Operator::Div, &[coordinate.clone(), coordinate])?,
            one,
            Primitive::Cancel,
        )
    } else {
        (
            source.term(Operator::Div, &[one, two])?,
            source.number(1, 2)?,
            Primitive::Exact,
        )
    };
    let mut weaken = equality(&frame, &domain, Primitive::ContextWeaken, &quotient, &half);
    weaken.premises = vec![0, 1];
    let mut consumer = equality(&frame, &domain, Primitive::Symmetry, &half, &quotient);
    consumer.premises = vec![2];
    let steps = [
        equality(&source.context, &original, rule, &quotient, &half),
        ApplicationDraft {
            context: frame.clone(),
            rule: Primitive::RegionConstruct,
            conclusion: Statement::Subset {
                subset: domain.clone(),
                superset: original.clone(),
            },
            premises: vec![],
            substitutions: vec![],
            certificate: Certificate::None,
        },
        weaken,
        consumer,
    ];
    if conditional {
        let expected = admit(
            &mut source.cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(source.header.clone()),
                scope: vec![],
                ambient: source.kind.clone(),
                expression: RegionExpr::Meet(vec![
                    Reference::Existing(original),
                    Reference::Existing(domain.clone()),
                ]),
            }),
            &mut ledger()?,
        )?;
        let weakened = source
            .cas
            .check_proof(&frame, &steps[..3], &mut ledger()?)?;
        let [required] = weakened.judgment().residuals.as_slice() else {
            return Err("missing weakened nonzero requirement".into());
        };
        assert_eq!(required.region, expected);
        assert_ne!(required.region, domain);
    }
    let witness = source.cas.check_proof(&frame, &steps, &mut ledger()?)?;
    assert_eq!(witness.judgment().residuals.is_empty(), !conditional);
    Ok(())
}
