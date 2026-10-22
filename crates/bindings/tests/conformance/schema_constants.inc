<!-- SPDX-License-Identifier: AGPL-3.0-only -->

A bindings-only client retains a checked exact schema definition:

```
use nrz_bindings::{Context, Descriptor, ExponentPolicy, Limits, LimitsOverride, ReferenceDefinition, Registry};
let owner = Context::with_real_symbols(Registry::new(ExponentPolicy::Rational), Limits::default(), &[])?.freeze()?;
let reference = ReferenceDefinition::unit("reference", &Descriptor::ratio())?;
let retained: ReferenceDefinition = reference;
owner.schema_normalization_map(&Descriptor::ratio(), &retained, None, &LimitsOverride::default())?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Changing only the move to a struct literal cannot manufacture reference authority:

```compile_fail,E0451
use nrz_bindings::{Context, Descriptor, ExponentPolicy, Limits, LimitsOverride, ReferenceDefinition, Registry};
let owner = Context::with_real_symbols(Registry::new(ExponentPolicy::Rational), Limits::default(), &[])?.freeze()?;
let reference = ReferenceDefinition::unit("reference", &Descriptor::ratio())?;
let retained: ReferenceDefinition = ReferenceDefinition { ..reference };
owner.schema_normalization_map(&Descriptor::ratio(), &retained, None, &LimitsOverride::default())?;
# Ok::<(), Box<dyn std::error::Error>>(())
```
