// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit mathematical fixtures and independently requested equality witnesses.

use nrz_bindings::{CheckedExpression, Context, Diagnostic, Registry};
use nrz_cas::proof::Statement;
use nrz_cas::request::Outcome;
use nrz_cas::source::OriginalForm;
use nrz_cas::syntax::{AtomicSort, Declaration, Environment, Field, Operator, PredicateKind, Sort};
use nrz_cas::{
    Fault,
    limits::{Budget, Limits, LimitsOverride},
};

/// Declare mathematical keys independently of the physical declarations under test.
pub(crate) fn fixture(registry: Registry, limits: Limits, keys: &[&str]) -> Result<Context, Fault> {
    let environment = Environment {
        declarations: keys
            .iter()
            .map(|key| Declaration {
                key: (*key).into(),
                display_name: (*key).into(),
                sort: if *key == "project" {
                    Sort::Function {
                        inputs: vec![AtomicSort::Scalar(Field::Real); 2],
                        output: AtomicSort::Scalar(Field::Real),
                    }
                } else {
                    Sort::Value(AtomicSort::Scalar(Field::Real))
                },
            })
            .collect(),
        ..Environment::default()
    };
    Context::new(registry, limits, environment)
}

/// Parse a fixture with the store's semantic header and independent parsing budget.
pub(crate) fn original(context: &Context, text: &str) -> Result<OriginalForm, Diagnostic> {
    context.parse(text, text)
}

/// Check an explicit equality target and every expected original domain obligation.
pub(crate) fn prove(
    context: &Context,
    checked: &CheckedExpression,
    text: &str,
    nonzero: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let target = context.parse(text, text)?;
    let result = match context.prove_equal(checked, &target, LimitsOverride::default())? {
        Outcome::Proved { result, .. } | Outcome::Conditional { result, .. } => result,
        outcome => return Err(format!("expected proved coordinate equality: {outcome:?}").into()),
    };
    let judgment = result.witness.judgment();
    let Statement::Equality { right, .. } = &judgment.statement else {
        return Err("missing equality statement".into());
    };
    assert_eq!(right, &target.syntax);
    if let Some(key) = nonzero {
        assert_eq!(judgment.residuals.len(), 1);
        let guard = judgment.residuals.first().ok_or("missing domain guard")?;
        let cas = context.cas()?;
        assert_eq!(
            cas.predicate_kind(&guard.predicate)?,
            &PredicateKind::NonZero
        );
        let arguments =
            cas.predicate_arguments(&guard.predicate, &mut Budget::new(Limits::default())?)?;
        assert_eq!(arguments.len(), 1);
        assert_eq!(
            cas.expression(arguments.first().ok_or("missing guarded value")?)?
                .0,
            &Operator::Free(key.into())
        );
    } else {
        assert_eq!(judgment.residuals, []);
    }
    Ok(())
}
