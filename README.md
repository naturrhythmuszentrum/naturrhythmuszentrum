<!-- SPDX-License-Identifier: CC0-1.0 -->

# Naturrhythmuszentrum · `guide:nrz:overview`

## Purpose · `sec:nrz:overview-purpose`

Naturrhythmuszentrum is a physics calculation engine for results whose meaning, numerical error and assumptions remain inspectable. It combines exact symbolic structure with verified floating matrix computations. A result carries evidence for a stated mathematical model and admitted input family; that evidence does not certify the model's physical adequacy. Accuracy governs acceptance before speed, and deterministic bounds, statistical models and numerical evaluation errors retain their separate meanings.

The first reusable physical target is LC resonance: one small circuit exercises mixed voltage/current coordinates, a Complex spectrum, phasor response, frequency kinds and explicit undefined outcomes. The implemented foundations already support exact CAS proofs, physical descriptor checks, outward arithmetic and certified homogeneous matrix operations. The complete LC engine remains an implementation route, not an available end-to-end product.

## Two graphs joined by bindings · `sec:nrz:overview-architecture`

The descriptor graph is exact CAS structure: symbols, sorts, kinds, units, axes and checked relations. Its definitions and judgments have the same meaning on every machine. The computational graph consists of float-region versions and the kernel applications that produced them, with tracked error and dependencies. Bindings joins a symbol's exact meaning to its selected numerical representation, or eventually to an exact value requiring no numerical region. See `docs/design/descriptor-binding.md` for the architecture and its implementation boundary.

The heap knows only floats in that architecture. A region has no inherent unit, kind, sort or physical meaning. Several symbols may read the same allocation through checked relations: a Complex symbol can read a 2×2 block while a Real symbol reads its corner, or the state `(v,i)` and its voltage/current components can read one two-float buffer. Every overlap needs a checked relation between the readings. Binding admission in `crates/matrix/src/binding/symbols.rs` refuses a second symbol owner per migrated region. The region-bindings step replaces that blanket refusal with checked related readings and retains numerical lifetime protection (`step:nrz:lc-resonance-region-bindings`).

A kernel is a float procedure with an exact array law and an enclosure theorem. Meaning is checked on the descriptor side before lowering to a numerical schedule. Exact constants cross the boundary once: bindings exports their spelling, scalar encloses it in the requested format with import error, and matrix uses the enclosure. No heap value enters CAS arithmetic. Exact π-dependent descriptors already exist, but a non-rational coefficient refuses numerical export until the quantity stage supplies certified constant evaluation.

Successful executions of the same exact target agree within their tracked error under admitted arithmetic models. They need not produce identical centers or bit patterns. Domain, coverage, coordinates and source assumptions must agree before two bounds can support that comparison; covariance is not a deterministic tolerance.

## Workspace · `sec:nrz:overview-workspace`

The nightly Rust workspace contains six implemented packages. Dependencies flow from matrix through bindings to CAS, with scalar and accuracy beneath matrix. Matrix integrates faer for candidates and verifies the supported numerical laws independently. CAS owns its exact graph; future physics plans will assemble descriptor operations and schedule supported numerical subgraphs without introducing another authority for meaning. Package structure is discussed in `docs/design/architecture.md`, with the ownership refinements in `docs/design/descriptor-binding.md`.

**Table (Workspace packages)** · `tab:nrz:workspace-packages`

|Crate|Path|Responsibility|Engine dependencies|
|-----|----|--------------|-------------------|
|`nrz-cas`|`crates/cas`|Exact graph structure, original occurrences and independently checked symbolic judgments.|None.|
|`nrz-bindings`|`crates/bindings`|Physical meaning, exact maps and checked symbol/representation relations over CAS.|`nrz-cas`.|
|`nrz-scalar`|`crates/scalar`|Exact-spelling imports and finite binary arithmetic with outward enclosures.|None.|
|`nrz-accuracy`|`crates/accuracy`|Nonnegative bound composition, conservative budgets and finite recovery policies.|`nrz-scalar`.|
|`nrz-matrix`|`crates/matrix`|Versioned numerical regions, float kernels, companions and verified publication.|`nrz-bindings`, `nrz-scalar`, `nrz-accuracy`.|
|`nrz-docgen`|`tools/docgen`|Validated documentation inputs, deterministic datasets, rendering and fixed-point checks.|`nrz-bindings`, `nrz-cas` for tooling checks.|
|`nrz-physics` (planned)|`crates/physics`|Reusable physical descriptor plans, source models and numerical scheduling.|Planned bindings, matrix and accuracy clients.|
|`nrz` (planned)|`crates/engine`|Curated application-facing exports.|Planned public engine interfaces.|

Each implemented crate's README is its rustdoc front page, introducing purpose, dependencies, the LC contribution, public concepts, guarantees, limits and testing. Detailed contracts live beside the corresponding source modules and types. The planned physics and facade packages are not workspace members.

## LC target and plan · `sec:nrz:overview-plan`

For `L=25 mH` and `C=10 µF`, the independent reference answers include `ω₀=2000 rad/s`, `f₀=1000/π Hz`, characteristic impedance `50 ohm` and state roots `±2000i s⁻¹`. Impedance below, at and above resonance is `−75i`, `0` and `+75i ohm`. At resonance, impedance succeeds while phase is undefined and admittance/current cannot produce a finite result. The source-free energy check and driven phasor experiment are distinct uses of the same ideal elements.

`docs/design/lc-resonance.md` defines dependencies for the named steps. Homogeneous import/copy admission in `crates/bindings/src/admission/` checks homogeneous import/copy value, companion and planned result symbols, while matrix validates and atomically publishes their live numerical inventory. This implements the binding-admission step (`step:nrz:lc-resonance-binding-admission`). The remaining work comprises:

- the descriptor-graph step (`step:nrz:lc-resonance-descriptor-graph`) and the region-bindings step (`step:nrz:lc-resonance-region-bindings`): graph-backed descriptors and representations and general versioned bindings and meaning-free kernel contracts.
- the coordinate-lowering step (`step:nrz:lc-resonance-coordinate-lowering`) and the symbolic-lowering step (`step:nrz:lc-resonance-symbolic-lowering`): mixed-coordinate maps and complete restoration and cumulative symbolic policy and exact LC theorem transport.
- the phasor-lowering step (`step:nrz:lc-resonance-phasor-lowering`), the spectral-lowering step (`step:nrz:lc-resonance-spectral-lowering`) and the numerical-acceptance step (`step:nrz:lc-resonance-numerical-acceptance`): checked phasor blocks, the positive two-state spectral theorem and integrated LC numerical acceptance.
- LC quantities (`step:nrz:lc-resonance-quantity-bite`): quantities, certified π/phase evaluation and data-time binding for supported recipes.
- LC plans (`step:nrz:lc-resonance-physics-bite`): reusable LC plans and shared-source frequency batches.
- LC reproducibility (`step:nrz:lc-resonance-record-bite`): within-error reproducibility and environment provenance after the first physics slice.

The design route retains the separate earlier symbolic proof-cost work and does not imply a general eigensolver, Complex numerical library or time integrator. The broader engine plan remains in `docs/plan/plan.md`; the LC design refines the relevant implementation sequence.

## Documentation · `sec:nrz:overview-documentation`

Repository Markdown is committed output from minijinja templates. Edit `templates/{output}.j2` and the corresponding inputs, and keep rendered documents synchronized in the same commit. Inputs and derived datasets live under `data/`. `nrz-docgen data` derives datasets, `nrz-docgen render` renders documents, and `nrz-docgen check` checks generated bytes, inventory and repeat-generation stability without writing.

**Table (Documentation map)** · `tab:nrz:documentation-set`

|Path|Role|
|----|----|
|`docs/design/descriptor-binding.md`|Two graphs, exact meanings, float storage, bindings and current migration gaps.|
|`docs/design/lc-resonance.md`|LC model, independent answers, ownership and named implementation dependencies (`sec:nrz:lc-resonance-bites`).|
|`docs/design/architecture.md`|Project purpose, guarantees and package structure.|
|`docs/design/matrix-core-design.md`, `docs/design/shapes.md`|Numerical enclosure, shape, companion and operation contracts.|
|`docs/design/cas-design.md`, `docs/design/cas-solving-design.md`|Exact graph, proof, replay and solving contracts.|
|`docs/design/units-design.md`, `docs/design/problem-data-schema.md`|Physical checking and the versioned data boundary.|
|`docs/reference/`, `data/problems/`|Reference systems, fixtures and their generated problem documentation.|
|`docs/records/`, `docs/plan/plan.md`|Recorded evidence, admission limits and the broader implementation plan.|
|`CONTRIBUTING.md`, `tools/docgen/README.md`|Contribution rules and documentation commands.|

## Licensing · `sec:nrz:overview-licensing`

Code is AGPL-3.0-only under `LICENSE-CODE`; data and documentation are CC0-1.0 under `LICENSE-DOCS`. Code sources keep their `SPDX-License-Identifier: AGPL-3.0-only` line; documentation, inputs and narrative templates keep `SPDX-License-Identifier: CC0-1.0`. Generated material has an explicit classification, and third-party material retains its applicable attribution. See `CONTRIBUTING.md` for contribution requirements.
