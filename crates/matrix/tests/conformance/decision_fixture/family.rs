// SPDX-License-Identifier: AGPL-3.0-only
//! The original physical duration remains free under two explicit source-box assumptions.

use super::{
    AssumptionDraft, Certificate, ContextDraft, Declaration, Draft, Frame, Handle, HeaderDraft,
    HypothesisRef, Operator, PredicateKind, Rational, Reference, RegionExpr, SelectionFixture,
    TestResult, admit, applications, cover_in, exact, ledger, real,
};
use nrz_bindings::readings::algebra::decision::{
    IntegerCellRange, IntegerFamily, ScalarKernelFamily,
};

/// Add one Real source to the unchanged Integer decision declaration vocabulary.
fn frame() -> TestResult<Frame> {
    let mut frame = Frame::new()?;
    frame.header = admit(
        &mut frame.cas,
        Draft::Header(HeaderDraft::Extend {
            parent: Reference::Existing(frame.header.clone()),
            declarations: vec![Declaration {
                key: "duration_input".into(),
                display_name: "duration input".into(),
                sort: real(),
            }],
            spaces: Vec::new(),
        }),
        &mut ledger()?,
    )?;
    frame.ambient = frame.region(RegionExpr::Ambient)?;
    Ok(frame)
}

/// Construct one exact source-minus-bound or bound-minus-source assumption.
fn endpoint(
    frame: &mut Frame,
    source: &Handle,
    bound: &Rational,
    lower: bool,
) -> TestResult<Handle> {
    let literal = frame.term(Operator::Rational(bound.clone()), real(), &[])?;
    let operands = if lower {
        [source.clone(), literal]
    } else {
        [literal, source.clone()]
    };
    let difference = frame.term(Operator::Sub, real(), &operands)?;
    admit(
        &mut frame.cas,
        Draft::Condition {
            header: Reference::Existing(frame.header.clone()),
            scope: Vec::new(),
            kind: PredicateKind::NonNegative,
            arguments: vec![Reference::Existing(difference)],
        },
        &mut ledger()?,
    )
}

/// Keep source-box assumptions in the context without making them floor-cover premises.
pub(super) fn fixture() -> TestResult<(SelectionFixture, ScalarKernelFamily)> {
    let (mut frame, mut family) = cover_in(frame()?, false)?;
    family.count = Some(Box::new(IntegerCellRange {
        coordinate: family.coordinate.clone(),
        lower: Some(Rational::zero()),
        upper: None,
    }));
    let input = frame.term(Operator::Free("duration_input".into()), real(), &[])?;
    let pi = frame.term(
        Operator::Constant(nrz_bindings::readings::algebra::syntax::Constant::Pi),
        real(),
        &[],
    )?;
    let thousand = exact(&mut frame, "1000", false)?;
    let period = frame.term(Operator::Div, real(), &[pi, thousand])?;
    let target = frame.term(Operator::Div, real(), &[input.clone(), period])?;
    finish(frame, family, input, target)
}

/// Both native and mapped questions retain the same exact source-box and quotient assumptions.
pub(super) fn finish(
    mut frame: Frame,
    family: IntegerFamily,
    input: Handle,
    target: Handle,
) -> TestResult<(SelectionFixture, ScalarKernelFamily)> {
    let lower = Rational::from_ratio(9999, 1_000_000, &mut ledger()?)?;
    let upper = Rational::from_ratio(10001, 1_000_000, &mut ledger()?)?;
    let low = endpoint(&mut frame, &input, &lower, true)?;
    let high = endpoint(&mut frame, &input, &upper, false)?;
    let correspondence = admit(
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
    let context = admit(
        &mut frame.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(frame.header.clone()),
            admissible: Reference::Existing(family.domain.clone()),
            assumptions: [
                ("a-physical-lower", low),
                ("b-physical-upper", high),
                ("z-original-quotient", correspondence.clone()),
            ]
            .into_iter()
            .map(|(key, condition)| AssumptionDraft {
                key: key.into(),
                condition: Reference::Existing(condition),
            })
            .collect(),
        }),
        &mut ledger()?,
    )?;
    let value = exact(&mut frame, "3", true)?;
    let mut steps = applications(
        &context,
        family,
        value,
        target,
        std::slice::from_ref(&correspondence),
        None,
    )?;
    let Certificate::Hypothesis(reference) = &mut steps[0].certificate else {
        return Err("source hypothesis".into());
    };
    reference.slot = 2;
    let input = ScalarKernelFamily {
        source: input,
        lower,
        upper,
        premises: [0, 1].map(|slot| HypothesisRef {
            context: context.clone(),
            slot,
        }),
    };
    Ok((
        SelectionFixture {
            frame,
            context,
            steps,
        },
        input,
    ))
}
