<!-- SPDX-License-Identifier: AGPL-3.0-only -->

A checked native recipe may be retained and inspected through the public facade:

```
use nrz_cas::{limits::{Limits, LimitsOverride}, syntax::{AtomicSort, Declaration as Mathematical, Environment, Field, Sort}};
use nrz_bindings::{CheckedSensitivity, Context, Declaration, Descriptor, Evidence, ExponentPolicy, Registry, SensitivityRecipe};
let mut builder = Context::new(Registry::new(ExponentPolicy::Rational), Limits::default(), Environment {
    declarations: vec![Mathematical { key: "x".into(), display_name: "x".into(), sort: Sort::Value(AtomicSort::Scalar(Field::Real)) }],
    ..Environment::default()
})?;
builder.declare("x".into(), Declaration::Input { descriptor: Descriptor::ratio(), nonzero: false, evidence: Evidence::default() })?;
let original = builder.parse("square", "x^2")?;
let checked = builder.check_sensitivity(SensitivityRecipe::Square, &[original], &["x".into()], &LimitsOverride::default())?;
let retained: CheckedSensitivity = checked;
assert_eq!(retained.coordinates(), ["x"]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Changing only the move to a struct literal cannot manufacture checked evidence:

```compile_fail,E0451
use nrz_cas::{limits::{Limits, LimitsOverride}, syntax::{AtomicSort, Declaration as Mathematical, Environment, Field, Sort}};
use nrz_bindings::{CheckedSensitivity, Context, Declaration, Descriptor, Evidence, ExponentPolicy, Registry, SensitivityRecipe};
let mut builder = Context::new(Registry::new(ExponentPolicy::Rational), Limits::default(), Environment {
    declarations: vec![Mathematical { key: "x".into(), display_name: "x".into(), sort: Sort::Value(AtomicSort::Scalar(Field::Real)) }],
    ..Environment::default()
})?;
builder.declare("x".into(), Declaration::Input { descriptor: Descriptor::ratio(), nonzero: false, evidence: Evidence::default() })?;
let original = builder.parse("square", "x^2")?;
let checked = builder.check_sensitivity(SensitivityRecipe::Square, &[original], &["x".into()], &LimitsOverride::default())?;
let retained: CheckedSensitivity = CheckedSensitivity { ..checked };
assert_eq!(retained.coordinates(), ["x"]);
# Ok::<(), Box<dyn std::error::Error>>(())
```
