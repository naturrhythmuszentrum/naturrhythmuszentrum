<!-- SPDX-License-Identifier: CC0-1.0 -->

# Engineering ownership and capability · `spec:nrz:engineering-overview`

**Definition (Evidence boundary)** · `def:nrz:engineering-record-boundary`

This set records the adopted scalar, accuracy and documentation-generator surfaces through test claims, constant pins, reviewed catalogues and dated observations. Bindings, matrix and CAS retain design-level descriptions while their evidence and constant adoption remains an open intent; a successful fixture does not establish every intended LC capability.

**Table (Declared workspace graph)** · `tab:nrz:engineering-workspace-graph`

|Package|Manifest responsibility|Normal dependencies|Development dependencies|Build dependencies|
|-------|-----------------------|-------------------|------------------------|------------------|
|`nrz-docgen`|Deterministic documentation datasets, rendering, and fixed-point checks|clap, minijinja, nrz-bindings, nrz-cas, serde, serde_json, thiserror, toml|tempfile||
|`nrz-scalar`|Finite binary element arithmetic with outward enclosures|thiserror|num-bigint, num-rational, num-traits||
|`nrz-accuracy`|Dimension-neutral nonnegative bounds and conservative accuracy policies|nrz-scalar|num-bigint, num-rational, num-traits||
|`nrz-matrix`|Typed descriptor heap with owned numerical regions and bound companions|faer, nrz-accuracy, nrz-bindings, nrz-scalar, thiserror|||
|`nrz-bindings`|Data-time quantity descriptors and checks over original symbolic expressions|nrz-cas, thiserror|||
|`nrz-cas`|Exact graph construction and checked symbolic derivations|num-bigint, num-complex, num-integer, num-rational, num-traits, petgraph|||

**Argument (Graph currency)** · `arg:nrz:engineering-graph-currency`

The census reads explicit Cargo membership and direct dependency declarations, resolves package aliases and workspace inheritance, and keeps normal, development and build edges separate. Conditional dependency declarations refuse instead of producing an invented unconditional graph (`[DOCGEN-claim:docgen:census-dependency-identities-and-refusals]`), (`[DOCGEN-claim:docgen:refuses-invalid-member-paths]`).

**Argument (Adopted numerical ownership)** · `arg:nrz:engineering-adopted-ownership`

Scalar admits finite arithmetic and encloses its exact targets; accuracy composes nonnegative bounds and applies dimension-neutral tolerance and recovery policies. Independent rational-oracle tests and retained retry allowances establish their named boundaries without granting physical model validity (`[SCALAR-claim:scalar:primitive-domain-and-range-refusals]`), (`[SCALAR-claim:scalar:literal-precision-change-lifecycle]`), (`[ACCURACY-claim:accuracy:budget-decisions-rational-oracle]`), (`[ACCURACY-claim:accuracy:precision-retry-cannot-reset-limits]`).

**Definition (LC route)** · `def:nrz:engineering-lc-route`

The design records describe the implemented homogeneous binding-admission subset and distinguish it from graph descriptors, total entry coverage, mixed restoration, cumulative symbolic lowering, phasor execution, spectral certification and integrated acceptance. Exact scale admission does not supply numerical constant enclosures, phase branches or reusable physical plans (`step:nrz:lc-resonance-binding-admission`), (`step:nrz:lc-resonance-descriptor-graph`), (`prin:nrz:descriptor-binding-total`), (`rule:nrz:descriptor-binding-checked-overlap`), (`step:nrz:lc-resonance-region-bindings`), (`step:nrz:lc-resonance-coordinate-lowering`), (`step:nrz:lc-resonance-symbolic-lowering`), (`step:nrz:lc-resonance-phasor-lowering`), (`step:nrz:lc-resonance-spectral-lowering`), (`step:nrz:lc-resonance-numerical-acceptance`).

**Definition (Delivery contract)** · `def:nrz:engineering-delivery-contract`

The first deliverable records platform and implementation provenance without an additional target-environment or latency/memory-envelope gate. Recorded costs describe their observed executions and do not become release thresholds without a separate product contract (`dec:nrz:lc-resonance-delivery-contract`).

## Open intents · `sec:nrz:engineering-overview-intents`

**Claim (Complete evidence adoption)** · `claim:nrz:engineering-later-wave-ownership`

Later documentation waves will register bindings, matrix and CAS constants, test claims and projections under the same profiles; their LC-facing acceptance evidence belongs to the numerical-acceptance stage (`step:nrz:lc-resonance-numerical-acceptance`).

**Claim (Graph ownership and mixed execution)** · `claim:nrz:engineering-graph-and-mixed-execution`

The descriptor-graph and region-binding stages will establish total entry coverage, checked overlap among bindings and optional representations, followed by original mixed restoration and the admitted LC numerical operations through their separate stage exits (`step:nrz:lc-resonance-descriptor-graph`), (`prin:nrz:descriptor-binding-total`), (`rule:nrz:descriptor-binding-checked-overlap`), (`step:nrz:lc-resonance-region-bindings`), (`step:nrz:lc-resonance-coordinate-lowering`), (`step:nrz:lc-resonance-numerical-acceptance`).

**Claim (Physical quantity and rerun surfaces)** · `claim:nrz:engineering-physical-surface`

The quantity, physics and record bites will supply bounded constant/phase numerics, versioned quantity recipes, reusable LC plans with common-source batches and fresh-process/build agreement within tracked error (`step:nrz:lc-resonance-quantity-bite`), (`step:nrz:lc-resonance-physics-bite`), (`step:nrz:lc-resonance-record-bite`).
