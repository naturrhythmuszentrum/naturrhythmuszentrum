// SPDX-License-Identifier: AGPL-3.0-only
//! Exact broad-domain fixture built through public CAS preparation inputs.

use super::{RetainedExpm1, TestResult};
use crate::{ExecutionAccount, Heap, LoanError};
use nrz_bindings::{
    Budget, Limits,
    readings::algebra::{
        Cas, Exhaustion, FaultKind, Handle, Location, Rational, Reference,
        domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr},
        enclosure::{
            CoordinateBound, EnclosureGoal, EnclosureRequest, Interval,
            prepared_expm1::{Counts, Policy, Request},
        },
        limits::BudgetKind,
        syntax::{
            AtomicSort, Declaration, Draft, Environment, ExpressionDraft, Field, Operator, Sort,
        },
    },
};
use nrz_scalar::{ArithmeticContext, Expm1Input};

/// Original cumulative ledger has ample exact arithmetic allowance.
pub(super) fn account() -> TestResult<ExecutionAccount> {
    Ok(ExecutionAccount::new(Budget::new(limits())?)?)
}
/// Fixed original limits are chosen before any ownership lifecycle begins.
pub(super) fn limits() -> Limits {
    Limits {
        work: 2_000_000_000,
        integer_exponent: 4096,
        arithmetic_steps: 50_000_000,
        output_bytes: 4_194_304,
        staged_payload_bytes: 64_000_000,
        witness_bytes: 4_194_304,
        ..Limits::default()
    }
}
/// The complete original request claims one coordinate in [-2,0].
pub(super) fn request() -> TestResult<(Cas, Request)> {
    let mut budget = Budget::new(limits())?;
    let mut cas = Cas::new(limits())?;
    let sort = Sort::Value(AtomicSort::Scalar(Field::Real));
    let mut environment = Environment::default();
    environment.declarations.push(Declaration {
        key: "x".into(),
        display_name: "x".into(),
        sort: sort.clone(),
    });
    let drafts = [
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
    let [header, _, context]: [Handle; 3] = cas
        .admit(&drafts, &mut budget)?
        .try_into()
        .map_err(|_| "fixture")?;
    let operators = [
        Operator::Free("x".into()),
        Operator::Exponential,
        Operator::Rational(Rational::one()),
        Operator::Sub,
    ];
    let mut terms: Vec<Handle> = Vec::new();
    for (index, operator) in operators.into_iter().enumerate() {
        let operands = match index {
            1 => vec![Reference::Existing(terms[0].clone())],
            3 => vec![
                Reference::Existing(terms[1].clone()),
                Reference::Existing(terms[2].clone()),
            ],
            0 | 2 => Vec::new(),
            _ => return Err("fixture index".into()),
        };
        let draft = Draft::Expression(ExpressionDraft {
            space: None,
            operator,
            sort: sort.clone(),
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            binding: None,
            operands,
        });
        terms.push(
            cas.admit(std::slice::from_ref(&draft), &mut budget)?
                .remove(0),
        );
    }
    let original = EnclosureRequest {
        target: terms[3].clone(),
        context,
        coordinates: vec![CoordinateBound {
            coordinate: terms[0].clone(),
            interval: Interval::new(
                Rational::from_ratio(-2, 1, &mut budget)?,
                Rational::zero(),
                &mut budget,
            )?,
        }],
        goal: EnclosureGoal::Width,
    };
    Ok((
        cas,
        Request {
            original,
            bits: 40,
            environment: 1,
        },
    ))
}
/// Public ownership constructor provides the only law fixture admission.
pub(super) fn prepare(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> TestResult<RetainedExpm1> {
    let (cas, request) = request()?;
    heap.prepare_retained_expm1(
        cas,
        &request,
        [-2.0, 0.0],
        Policy::default(),
        &ArithmeticContext::new()?,
        &mut Counts::default(),
        account,
    )
    .map_err(|(error, _)| error.into())
}
/// Exact represented singleton input.
pub(super) const fn point(value: f64) -> Expm1Input {
    Expm1Input {
        endpoints: [value; 2],
        center: value,
        bits: 40,
    }
}

/// Closed pairing supplied to isolated shell tests, with no public authority construction.
pub(super) const fn seal() -> super::super::Seal {
    super::super::Seal {
        domain: [(-2.0_f64).to_bits(), 0],
        revisions: [1; 4],
    }
}

/// Spend the original cumulative ledger down to a boundary without replacing its fixed limits.
pub(super) fn leave_work(account: &mut ExecutionAccount, available: u64) -> TestResult<u64> {
    let prefix = account.limits().work - account.usage().work - available;
    account
        .ledger
        .work(prefix, Location::new("retained law caller prefix"))?;
    Ok(account.usage().work)
}

/// Check every field of a managed Work refusal, including its accounting location.
pub(super) fn work_refusal(error: LoanError, limit: u64, needed: u64) -> TestResult {
    let LoanError::Budget(fault) = error else {
        return Err("typed managed Work refusal".into());
    };
    assert_eq!(
        fault.kind,
        FaultKind::Exhausted {
            reason: Exhaustion::Limit,
            budget: BudgetKind::Work,
            limit,
            needed,
        }
    );
    assert_eq!(fault.location, Location::new("native owned host input"));
    assert!(fault.series_evidence.is_none());
    Ok(())
}
