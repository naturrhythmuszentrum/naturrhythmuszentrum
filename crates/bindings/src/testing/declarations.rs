// SPDX-License-Identifier: AGPL-3.0-only
//! Shared declaration setup expressed entirely through the public facade.

use nrz_cas::limits::Limits;
use nrz_cas::syntax::{AtomicSort, Declaration as Mathematical, Environment, Field, Sort};

use crate::test_api::{
    BaseDimension, Context, Declaration, Descriptor, Dimension, Evidence, ExponentPolicy, Kind,
    Measure, Observable, Registry, Role,
};

/// Setup errors remain test errors instead of panics.
pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Predeclare real mathematical keys, with one optional unary function key.
pub(crate) fn context(keys: &[&str]) -> TestResult<Context> {
    context_with_registry(Registry::new(ExponentPolicy::Rational), keys)
}

/// Predeclare mathematical keys with an explicitly supplied physical rule registry.
pub(crate) fn context_with_registry(registry: Registry, keys: &[&str]) -> TestResult<Context> {
    let environment = Environment {
        declarations: keys
            .iter()
            .map(|key| Mathematical {
                key: (*key).into(),
                display_name: (*key).into(),
                sort: if *key == "project" {
                    Sort::Function {
                        inputs: vec![AtomicSort::Scalar(Field::Real)],
                        output: AtomicSort::Scalar(Field::Real),
                    }
                } else {
                    Sort::Value(AtomicSort::Scalar(Field::Real))
                },
            })
            .collect(),
        ..Environment::default()
    };
    Ok(Context::new(registry, Limits::default(), environment)?)
}

/// A declared length kind with coherent difference coordinates.
pub(crate) fn length() -> TestResult<Descriptor> {
    Ok(Descriptor::new(
        Kind::Named(
            "length".into(),
            Box::new(Dimension::base(BaseDimension::Length)),
        ),
        Measure::coherent(),
        Role::Difference,
    )?)
}

/// A physical input with no numerical nonzero assertion.
pub(crate) fn input(builder: &mut Context, name: &str, descriptor: &Descriptor) -> TestResult {
    builder.declare(
        name.into(),
        Declaration::Input {
            descriptor: descriptor.clone(),
            nonzero: false,
            evidence: Evidence {
                source: format!("source:{name}"),
                assurance: "declared".into(),
            },
        },
    )?;
    Ok(())
}

/// Parse and retain the original body of one derived declaration.
pub(crate) fn derived(
    builder: &mut Context,
    name: &str,
    body: &str,
    descriptor: &Descriptor,
) -> TestResult {
    let expression = builder.parse(name, body)?;
    builder.declare(
        name.into(),
        Declaration::Derived(Observable {
            descriptor: descriptor.clone(),
            expression,
            evidence: Evidence {
                source: format!("recipe:{name}"),
                assurance: "declared".into(),
            },
        }),
    )?;
    Ok(())
}
