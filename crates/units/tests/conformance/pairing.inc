<!-- SPDX-License-Identifier: AGPL-3.0-only -->

A sensitivity is obtained through checked descriptor derivation:

```
use nrz_units::{Descriptor, PairingWitness};
let ratio = Descriptor::ratio();
let pairing = PairingWitness::sensitivity(&ratio, &ratio)?;
let retained: PairingWitness = pairing;
assert_eq!(retained.target(), &ratio);
# Ok::<(), nrz_units::PairingError>(())
```

Replacing the move with a struct literal cannot forge the pairing:

```compile_fail,E0451
use nrz_units::{Descriptor, PairingWitness};
let ratio = Descriptor::ratio();
let pairing = PairingWitness::sensitivity(&ratio, &ratio)?;
let retained: PairingWitness = PairingWitness { ..pairing };
assert_eq!(retained.target(), &ratio);
# Ok::<(), nrz_units::PairingError>(())
```

A physical radius may participate in a descriptor-only bound contraction:

```
use nrz_units::{Descriptor, EvaluationDescriptor, PairingWitness, UncertaintyMode};
let ratio = Descriptor::ratio();
let map = PairingWitness::sensitivity(&ratio, &ratio)?;
let physical = PairingWitness::uncertainty(UncertaintyMode::Radius, &ratio, None, None)?;
let evaluation = EvaluationDescriptor::new(&physical);
assert_eq!(evaluation.target(), &physical);
map.contract_radius(&physical)?;
# Ok::<(), nrz_units::PairingError>(())
```

Its numerical evaluation error is a different target and cannot substitute for that radius:

```compile_fail,E0308
use nrz_units::{Descriptor, EvaluationDescriptor, PairingWitness, UncertaintyMode};
let ratio = Descriptor::ratio();
let map = PairingWitness::sensitivity(&ratio, &ratio)?;
let physical = PairingWitness::uncertainty(UncertaintyMode::Radius, &ratio, None, None)?;
let evaluation = EvaluationDescriptor::new(&physical);
assert_eq!(evaluation.target(), &physical);
map.contract_radius(&evaluation)?;
# Ok::<(), nrz_units::PairingError>(())
```

The evaluation descriptor can also be retained through its checked constructor:

```
use nrz_units::{Descriptor, EvaluationDescriptor, PairingWitness};
let ratio = Descriptor::ratio();
let pairing = PairingWitness::sensitivity(&ratio, &ratio)?;
let evaluation = EvaluationDescriptor::new(&pairing);
let retained = evaluation;
assert_eq!(retained.target(), &pairing);
# Ok::<(), nrz_units::PairingError>(())
```

Its target and evaluation kind cannot be supplied by a struct literal:

```compile_fail,E0451
use nrz_units::{Descriptor, EvaluationDescriptor, PairingWitness};
let ratio = Descriptor::ratio();
let pairing = PairingWitness::sensitivity(&ratio, &ratio)?;
let evaluation = EvaluationDescriptor::new(&pairing);
let retained = EvaluationDescriptor { ..evaluation };
assert_eq!(retained.target(), &pairing);
# Ok::<(), nrz_units::PairingError>(())
```
