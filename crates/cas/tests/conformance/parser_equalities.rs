// SPDX-License-Identifier: AGPL-3.0-only
//! Proved-equality requirements for the later units migration, using production limits.

use nrz_cas::limits::{Limits, LimitsOverride};
use nrz_cas::proof::{DomainContract, Judgment, Statement};
use nrz_cas::request::{CoveredInput, Request, SearchResult, Task};
use nrz_cas::source::{OriginalForm, parse_original};
use nrz_cas::syntax::{Field, Operator, PredicateKind};
use nrz_cas::{Cas, Handle};

use crate::testing::builders::{core_context, environment};
use crate::testing::common::{TestResult, default_budget};
use crate::testing::witness::completed;

/// One default-bounded context containing exactly the requested real coordinates.
fn setup(keys: &[&str]) -> TestResult<(Cas, Handle, Handle)> {
    let mut cas = Cas::new(Limits::default())?;
    let context = core_context(
        &mut cas,
        environment(Field::Real, keys),
        Vec::new(),
        &mut default_budget()?,
    )?;
    let header = cas.context_header(&context)?;
    Ok((cas, context, header))
}

/// Form an equality request only from the parser's original roots and complete receipts.
fn request(cas: &Cas, context: &Handle, forms: &[OriginalForm]) -> TestResult<Request> {
    let inputs = forms
        .iter()
        .map(|form| {
            Ok(CoveredInput {
                root: form.root.clone(),
                receipt: cas
                    .walk_original(&form.root, &mut default_budget()?)?
                    .receipt,
            })
        })
        .collect::<TestResult<_>>()?;
    Ok(Request {
        task: Task::Equal {
            domain: cas.admissible_region(context)?,
            domain_contract: DomainContract::DefinedAndEqual,
        },
        context: context.clone(),
        inputs,
        premises: Vec::new(),
        limits: LimitsOverride::default(),
    })
}

/// Parse both complete sides and check the default simplifier's exact target and obligation.
fn prove(keys: &[&str], left: &str, right: &str, nonzero: Option<&str>) -> TestResult {
    let (mut cas, context, header) = setup(keys)?;
    let left_form = parse_original(&mut cas, "left", left, &header, &mut default_budget()?)
        .map_err(|error| error.to_string())?;
    let right_form = parse_original(&mut cas, "right", right, &header, &mut default_budget()?)
        .map_err(|error| error.to_string())?;
    let forms = [left_form, right_form];
    let operation = request(&cas, &context, &forms)?;
    let result = completed(cas.run(&operation))?;
    let claim = result.witness.judgment();
    let Statement::Equality { left, right, .. } = &claim.statement else {
        return Err("expected a proved equality".into());
    };
    assert_eq!(left, &forms.first().ok_or("left form")?.syntax);
    assert_eq!(right, &forms.get(1).ok_or("right form")?.syntax);
    match nonzero {
        None => assert_eq!(claim.residuals, []),
        Some(key) => assert_nonzero(&cas, claim, key)?,
    }
    replay_result(&cas, &result)?;
    Ok(())
}

/// A conditional quotient theorem must name the actual declared denominator in its guard.
fn assert_nonzero(cas: &Cas, claim: &Judgment, key: &str) -> TestResult {
    assert_eq!(claim.residuals.len(), 1);
    let guard = claim.residuals.first().ok_or("nonzero guard missing")?;
    assert_eq!(
        cas.predicate_kind(&guard.predicate)?,
        &PredicateKind::NonZero
    );
    let arguments = cas.predicate_arguments(&guard.predicate, &mut default_budget()?)?;
    assert_eq!(arguments.len(), 1);
    assert_eq!(
        cas.expression(arguments.first().ok_or("denominator missing")?)?
            .0,
        &Operator::Free(key.into())
    );
    Ok(())
}

/// Canonical witness bytes replay in a fresh store with independent default limits.
fn replay_result(cas: &Cas, result: &SearchResult) -> TestResult {
    let bytes = cas.export_witness(&result.witness, &mut default_budget()?)?;
    let mut fresh = Cas::new(Limits::default())?;
    let checked = fresh.inspect(&bytes, &mut default_budget()?)?;
    assert_eq!(
        fresh.export_witness(&checked, &mut default_budget()?)?,
        bytes
    );
    Ok(())
}

/// Cancellation is a requested theorem after visiting both written occurrences.
/// ´test:integration:parsed-self-subtraction-equality´
#[test]
fn parsed_self_subtraction_equality() -> TestResult {
    prove(&["x"], "x - x", "0", None)
}

/// Quotient cancellation retains the nonzero obligation under default limits.
/// ´test:integration:parsed-self-division-equality´
#[test]
fn parsed_self_division_equality() -> TestResult {
    prove(&["x"], "x / x", "1", Some("x"))
}

/// Decimal and division spellings reach the same exact scalar coefficient by proof.
/// ´test:integration:parsed-half-coefficient-equality´
#[test]
fn parsed_half_coefficient_equality() -> TestResult {
    prove(&["sigma"], "sigma / 2", "0.5 * sigma", None)
}

/// Symbolic coefficient collection must reach the explicitly requested factored endpoint.
/// ´test:integration:parsed-symbolic-collection-equality´
#[test]
fn parsed_symbolic_collection_equality() -> TestResult {
    prove(&["a", "b", "x"], "a*x + b*x", "(a + b)*x", None)
}

/// Affine offsets cancel by a proved scalar equality, with original additions retained.
/// ´test:integration:parsed-affine-cancellation-equality´
#[test]
fn parsed_affine_cancellation_equality() -> TestResult {
    prove(&["T", "T0", "c"], "(T + c) - (T0 + c)", "T - T0", None)
}
