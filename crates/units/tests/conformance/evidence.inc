<!-- SPDX-License-Identifier: AGPL-3.0-only -->

The public construction and ownership boundary is checked by these rustdoc twins.
A valid builder produces a checked snapshot:

```
use nrz_cas::{limits::Limits, syntax::Environment};
use nrz_units::{Context, ExponentPolicy, Registry};
let builder = Context::new(
    Registry::new(ExponentPolicy::Rational), Limits::default(), Environment::default(),
)?;
let frozen = builder.freeze()?;
assert!(frozen.checks().is_empty());
# Ok::<(), Box<dyn std::error::Error>>(())
```

The same sequence cannot reuse the consumed builder:

```compile_fail,E0382
use nrz_cas::{limits::Limits, syntax::Environment};
use nrz_units::{Context, ExponentPolicy, Registry};
let builder = Context::new(
    Registry::new(ExponentPolicy::Rational), Limits::default(), Environment::default(),
)?;
let frozen = builder.freeze()?;
assert!(builder.check_all()?.is_empty());
# Ok::<(), Box<dyn std::error::Error>>(())
```

A frozen value may be moved without exposing its fields:

```
use nrz_cas::{limits::Limits, syntax::Environment};
use nrz_units::{CheckedDeclarations, Context, ExponentPolicy, Registry};
let frozen = Context::new(
    Registry::new(ExponentPolicy::Rational), Limits::default(), Environment::default(),
)?.freeze()?;
let retained: CheckedDeclarations = frozen;
assert!(retained.checks().is_empty());
# Ok::<(), Box<dyn std::error::Error>>(())
```

Replacing that move with a struct literal cannot manufacture checked state:

```compile_fail,E0451
use nrz_cas::{limits::Limits, syntax::Environment};
use nrz_units::{CheckedDeclarations, Context, ExponentPolicy, Registry};
let frozen = Context::new(
    Registry::new(ExponentPolicy::Rational), Limits::default(), Environment::default(),
)?.freeze()?;
let retained: CheckedDeclarations = CheckedDeclarations { ..frozen };
assert!(retained.checks().is_empty());
# Ok::<(), Box<dyn std::error::Error>>(())
```

The registry is available for read-only descriptor operations:

```
use nrz_cas::{limits::Limits, syntax::Environment};
use nrz_units::{Context, Descriptor, ExponentPolicy, Registry};
let frozen = Context::new(
    Registry::new(ExponentPolicy::Rational), Limits::default(), Environment::default(),
)?.freeze()?;
let registry = frozen.registry();
registry.negate(&Descriptor::ratio())?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Changing that operation to a rule-table mutation is refused:

```compile_fail,E0596
use nrz_cas::{limits::Limits, syntax::Environment};
use nrz_units::{Context, Descriptor, ExponentPolicy, Registry};
let frozen = Context::new(
    Registry::new(ExponentPolicy::Rational), Limits::default(), Environment::default(),
)?.freeze()?;
let registry = frozen.registry();
registry.admit(&Descriptor::ratio())?;
# Ok::<(), Box<dyn std::error::Error>>(())
```
