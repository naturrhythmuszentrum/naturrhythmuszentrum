// SPDX-License-Identifier: AGPL-3.0-only
//! Native owners supply the consuming carrier and the original structured variable payloads.

use super::{CAPACITY, Encoding, NODES, Sink, TestResult};
use crate::{ExecutionAccount, LcOmegaOutput, LcShapeError};
use nrz_bindings::{AdmissionError, UnitError, readings::algebra as cas};
use nrz_scalar::Scalar;
use std::sync::Arc;

/// Verify actual declaration identity and error allocation identity across the consuming return.
pub(in crate::loan) fn returned<P: Scalar + 'static, const N: usize>(
    output: LcOmegaOutput<'_, P, N>,
    variant: usize,
    account: &mut ExecutionAccount,
) -> TestResult {
    let key = output.specialization_key();
    let original = std::ptr::from_ref(output.inventory());
    let sink = Sink::admit(&NODES, CAPACITY, &[], account)?;
    let before = account.usage().work;
    let prepared = sink.prepare(0, Encoding::Refusal, account)?;
    // H1 M7 preflight + T1 M4 native inspection + M6 encoded fields I6 M5 commit.
    assert_eq!(account.usage().work - before, 30);
    // Fixed sink encoding/commit is prepaid before the native operation creates its cause.
    let error = cause(variant, account)?;
    let string = match &error {
        LcShapeError::Declaration(AdmissionError::Units(
            UnitError::UnsupportedMeasureScale(text) | UnitError::ConflictingKind(text),
        )) => Some((text.as_ptr(), text.len(), text.capacity())),
        _ => None,
    };
    let series = match &error {
        LcShapeError::Declaration(
            AdmissionError::Algebra(fault) | AdmissionError::Units(UnitError::Arithmetic(fault)),
        ) => fault.series_evidence.clone(),
        _ => None,
    };
    let failure = output.sink_refusal(error);
    let (output, error) = failure.into_parts();
    let before = account.usage().work;
    let returned = prepared.return_refusal(output, error);
    let handle = returned.receipt?;
    let output = returned.capability;
    let error = returned.error;
    assert_eq!(account.usage().work, before);
    assert_eq!(output.specialization_key(), key);
    assert_eq!(std::ptr::from_ref(output.inventory()), original);
    match &error {
        LcShapeError::Declaration(AdmissionError::Units(
            UnitError::UnsupportedMeasureScale(text) | UnitError::ConflictingKind(text),
        )) => assert_eq!(Some((text.as_ptr(), text.len(), text.capacity())), string),
        LcShapeError::Declaration(
            AdmissionError::Algebra(fault) | AdmissionError::Units(UnitError::Arithmetic(fault)),
        ) => {
            let evidence = fault
                .series_evidence
                .as_ref()
                .ok_or("original series evidence")?;
            assert!(Arc::ptr_eq(
                evidence,
                series.as_ref().ok_or("outside witness")?
            ));
            assert_eq!(Arc::strong_count(evidence), 2);
            assert_eq!(
                fault.series_constant().ok_or("original Rational")?.value,
                cas::Rational::parse("2/3", &mut account.ledger)?
            );
        }
        LcShapeError::Loan(crate::LoanError::UnsupportedContract) => assert_eq!(variant, 4),
        LcShapeError::Declaration(AdmissionError::UnsupportedRecipe) => assert_eq!(variant, 5),
        _ => return Err("wrong consuming refusal".into()),
    }
    let pull = sink.pull(handle, account)?;
    assert_eq!(pull.bytes(), 41);
    let tag = [0, 1, 2, 2, 3, 4][variant];
    assert_eq!(pull.words(), [42, 7, 19, 3, tag, 2]);
    drop(pull);
    sink.acknowledge(handle, account)?;
    overlength(output, account)
}

/// Both native series branches and the remaining closed tags have independent input causes.
fn cause(variant: usize, account: &mut ExecutionAccount) -> TestResult<LcShapeError> {
    Ok(match variant {
        0 => AdmissionError::Units(UnitError::UnsupportedMeasureScale("scale".repeat(257))).into(),
        1 => AdmissionError::Units(UnitError::ConflictingKind("kind".repeat(257))).into(),
        2 => AdmissionError::Algebra(series_fault(account)?).into(),
        3 => AdmissionError::Units(UnitError::Arithmetic(series_fault(account)?)).into(),
        4 => LcShapeError::Loan(crate::LoanError::UnsupportedContract),
        _ => AdmissionError::UnsupportedRecipe.into(),
    })
}

/// A corrupt admitted bound returns the identical real declaration and leaves evidence empty.
fn overlength<P: Scalar, const N: usize>(
    output: LcOmegaOutput<'_, P, N>,
    account: &mut ExecutionAccount,
) -> TestResult {
    let key = output.specialization_key();
    let original = std::ptr::from_ref(output.inventory());
    let mut stopped = Sink::admit(&NODES, CAPACITY, &[], account)?;
    std::rc::Rc::get_mut(&mut stopped.backing)
        .ok_or("exclusive native stop fixture")?
        .slots[0]
        .bound = 40;
    let returned = stopped.deliver(
        0,
        output,
        super::Evidence::Outcome(super::Outcome::Published),
        account,
    );
    assert!(matches!(returned.receipt, Err(super::Error::Overlength)));
    assert_eq!(returned.capability.specialization_key(), key);
    assert_eq!(
        std::ptr::from_ref(returned.capability.inventory()),
        original
    );
    assert_eq!(stopped.backing.slots[0].state.get(), 1);
    assert!(stopped.backing.words.borrow().iter().all(|word| *word == 0));
    Ok(())
}
/// Obtain a real independently owned Rational refusal through the public CAS producer.
fn series_fault(account: &mut ExecutionAccount) -> TestResult<cas::Fault> {
    use cas::{
        Reference as R,
        series::{SeriesObject, SeriesOperation},
        solving::{CoefficientDomainData, SolveDraft},
        syntax::Draft,
    };
    let mut store = cas::Cas::new(account.limits().clone())?;
    let (header, context) = roots(&mut store, account)?;
    let field = store
        .admit(
            &[Draft::Solve(SolveDraft::CoefficientDomain(
                CoefficientDomainData::Rational,
            ))],
            &mut account.ledger,
        )?
        .pop()
        .ok_or("rational coefficient field")?;
    let zero = expression(
        &mut store,
        &header,
        cas::syntax::Operator::Rational(cas::Rational::zero()),
        account,
    )?;
    let number = cas::Rational::parse("2/3", &mut account.ledger)?;
    let coefficient = expression(
        &mut store,
        &header,
        cas::syntax::Operator::Rational(number),
        account,
    )?;
    let variable = expression(
        &mut store,
        &header,
        cas::syntax::Operator::Free("x".into()),
        account,
    )?;
    let object = store
        .admit(
            &[Draft::SeriesObject(SeriesObject {
                field: R::Existing(field),
                centre: R::Existing(zero),
                variable: R::Existing(variable),
                scope: Vec::new(),
                context: R::Existing(context),
                order: 1,
                coefficients: vec![R::Existing(coefficient)],
            })],
            &mut account.ledger,
        )?
        .pop()
        .ok_or("series object")?;
    let fault = store
        .compute_series(
            &SeriesOperation::Compose {
                left: object.clone(),
                right: object,
                order: None,
            },
            &[],
            &mut account.ledger,
        )
        .expect_err("nonzero inner constant");
    assert!(fault.series_constant().is_some());
    Ok(fault)
}
/// One real coordinate and explicit root context are admitted without producer expectations.
fn roots(
    store: &mut cas::Cas,
    account: &mut ExecutionAccount,
) -> TestResult<(cas::Handle, cas::Handle)> {
    use cas::{
        Reference as R,
        domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr},
        syntax::{AtomicSort, Declaration, Draft, Environment, Field, Sort},
    };
    let handles = store.admit(
        &[
            Draft::Header(HeaderDraft::Core(Environment {
                field: Field::Real,
                declarations: vec![Declaration {
                    key: "x".into(),
                    display_name: "x".into(),
                    sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
                }],
                ..Environment::default()
            })),
            Draft::Region(RegionDraft {
                header: R::Local(0),
                scope: Vec::new(),
                ambient: AmbientDomain::Real("x".into()),
                expression: RegionExpr::Ambient,
            }),
            Draft::Context(ContextDraft::Root {
                header: R::Local(0),
                admissible: R::Local(1),
                assumptions: Vec::new(),
            }),
        ],
        &mut account.ledger,
    )?;
    Ok((handles[0].clone(), handles[2].clone()))
}
/// Literal syntax uses the same original account; no fresh budget hides native Work.
fn expression(
    store: &mut cas::Cas,
    header: &cas::Handle,
    operator: cas::syntax::Operator,
    account: &mut ExecutionAccount,
) -> TestResult<cas::Handle> {
    use cas::{
        Reference,
        syntax::{AtomicSort, Draft, ExpressionDraft, Field, Sort},
    };
    Ok(store
        .admit(
            &[Draft::Expression(ExpressionDraft {
                space: None,
                operator,
                sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                binding: None,
                operands: Vec::new(),
            })],
            &mut account.ledger,
        )?
        .pop()
        .ok_or("original expression")?)
}
