<!-- SPDX-License-Identifier: AGPL-3.0-only -->

A checked map may be retained without exposing mutable coefficient definitions:

```
use nrz_cas::{limits::Limits, syntax::Environment};
use nrz_units::{CheckedMap, Context, Descriptor, ExponentPolicy, Registry};
let frozen = Context::new(Registry::new(ExponentPolicy::Rational), Limits::default(), Environment::default())?.freeze()?;
let map = frozen.conversion_map(&Descriptor::ratio(), &Descriptor::ratio())?;
let retained: CheckedMap = map;
retained.validate_snapshot(&frozen)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Changing only the move to a struct literal cannot manufacture a checked map:

```compile_fail,E0451
use nrz_cas::{limits::Limits, syntax::Environment};
use nrz_units::{CheckedMap, Context, Descriptor, ExponentPolicy, Registry};
let frozen = Context::new(Registry::new(ExponentPolicy::Rational), Limits::default(), Environment::default())?.freeze()?;
let map = frozen.conversion_map(&Descriptor::ratio(), &Descriptor::ratio())?;
let retained: CheckedMap = CheckedMap { ..map };
retained.validate_snapshot(&frozen)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```
