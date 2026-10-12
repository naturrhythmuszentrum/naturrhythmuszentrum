<!-- SPDX-License-Identifier: CC0-1.0 -->

# Naturrhythmuszentrum · `guide:nrz:overview`

## Purpose · `sec:nrz:overview-purpose`

Naturrhythmuszentrum is a matrix-based physics calculation engine for researchers, engineers, and developers who need inspectable dimensions, accuracy, and assumptions. It represents physical relationships and computational dependencies with petgraph, uses faer for linear-algebra candidates, and expresses quantities through dimensions, measures, and semantic kinds, keeping discrete physical counts distinct from ratios, angles, amount of substance, and structural metadata (`def:vision:dimensions`), (`def:vision:measures`), (`def:vision:counts`), (`rule:vision:semantic-kinds-and-roles`), (`rule:vision:counts-and-metadata`). Its result carries evidence about numerical reliability under a stated mathematical model and declared input uncertainty (`def:vision:purpose-and-audience`).

Accuracy governs acceptance before speed; dimensions, measures, kinds and affine roles survive scalar, matrix, and graph abstractions; and every public physical scalar and matrix carries evidence (`prin:vision:accuracy-before-speed`), (`prin:vision:dimensions-survive-abstraction`), (`prin:vision:values-carry-evidence`). Guarantees identify their scope, workspace code uses safe Rust, documentation is reproducible, and package and computational boundaries remain independently reviewable (`prin:vision:guarantees-name-scope`), (`prin:vision:safety-defines-boundaries`), (`prin:vision:documentation-is-reproducible`), (`prin:vision:reviewable-boundaries`). Algebraic unit checks cover equations, blocks, observables and uncertainty expressions through types and data validation (`req:vision:algebraic-unit-checks`). Measurement and computational uncertainties belong to the matrix’s contents and each has its own derived dimension, measure and kind (`def:vision:intrinsic-uncertainty-companions`), (`rule:vision:uncertainty-unit-algebra`).

## Workspace · `sec:nrz:overview-workspace`

The baseline is a virtual nightly Cargo workspace. The packages below are the target structure and join the workspace member list as coherent implementations land (`def:vision:baseline-and-proposed-tree`). Dependencies follow the declared graph, including normal, build, development, and feature-dependent edges (`rule:vision:layer-enforcement`).

**Table (Workspace packages)** · `tab:nrz:workspace-packages`

|Directory|Crate|Responsibility|Engine dependencies|Architecture|
|---------|-----|--------------|-------------------|------------|
|`packages/algebra`|`nrz-algebra`|Exact symbolic scalar expressions and collection normal forms|None of the other engine packages|(`tab:vision:package-structure`)|
|`packages/scalar`|`nrz-scalar`|Supported scalar contracts, conversions, and validated primitive numerical operations|None of the other engine packages|(`tab:vision:package-structure`)|
|`packages/accuracy`|`nrz-accuracy`|Dimension-neutral envelope primitives, assurance, policy, and compact provenance|`nrz-scalar`|(`tab:vision:package-structure`)|
|`packages/units`|`nrz-units`|Dimensions, measures, counts and kinds, typed quantities, conversions and algebraic checks|`nrz-scalar`, `nrz-accuracy`|(`tab:vision:package-structure`)|
|`packages/matrix`|`nrz-matrix`|Matrices with checked block descriptors, intrinsic uncertainties, faer adapters and certificates|`nrz-scalar`, `nrz-accuracy`, `nrz-units`|(`tab:vision:package-structure`)|
|`packages/physics`|`nrz-physics`|Physical structures, constraint assembly, and computation graphs using petgraph|`nrz-units`, `nrz-matrix`, `nrz-accuracy`|(`tab:vision:package-structure`)|
|`packages/engine`|`nrz`|Curated application-facing exports and entry points|Public interfaces of the engine packages above|(`tab:vision:package-structure`)|
|`packages/docgen`|`nrz-docgen`|TOML ingestion, deterministic datasets, minijinja rendering, and fixed-point verification|No runtime engine dependency|(`tab:vision:package-structure`)|

Only the matrix package integrates faer, and only the physics package integrates petgraph. Engine interfaces check and preserve dimensions, measures, kinds, normalization maps, and intrinsic uncertainty companions across those boundaries (`rule:vision:backend-boundaries`), (`rule:vision:heterogeneous-assembly`), (`def:vision:intrinsic-uncertainty-companions`).

## Documentation · `sec:nrz:overview-documentation`

Every repository Markdown file is a committed render of a minijinja template. Edit the template and its owning TOML inputs; derive JSON datasets and regenerate the render in the same meaningful commit. Templates sit beside outputs, with central `docs/` inputs and datasets for root and cross-cutting documents (`prin:vision:documentation-is-reproducible`), (`dec:vision:documentation-layout`), (`rule:vision:documentation-envelope-and-derivation`).

The generator interface is established by (`step:nrz:bite-documentation-generator`):

```sh
nrz-docgen data
nrz-docgen render
nrz-docgen check
```

The commands derive datasets, render documents, and verify the committed generation fixed point. `check` regenerates in an isolated output area, compares bytes and output inventory, verifies that another generation changes nothing, and exits 3 on drift (`rule:vision:fixed-point-gate-and-bootstrap`), (`step:nrz:bite-documentation-generator`).

**Table (Documentation set)** · `tab:nrz:documentation-set`

|Document|Title label|Role|
|--------|-----------|----|
|Conceptual architecture of the Naturrhythmuszentrum engine|(`spec:vision:conceptual-architecture`)|Defines purpose, guarantees, dimensions, measures, counts, algebraic unit checks, uncertainty contracts, numerical boundaries, repository structure, and completion obligations.|
|Exemplars to design against|(`reg:exemplars:reference-register`)|Records the systems, problems, and adversarial fixtures that inform and test those decisions.|
|The engine's plan|(`plan:nrz:engine-plan`)|Orders implementation bites and states their deliverables, verification, exclusions, import procedure, and gate.|
|Naturrhythmuszentrum|(`guide:nrz:overview`)|Provides the workspace and documentation entry point.|
|Contributing to Naturrhythmuszentrum|(`guide:nrz:contributing`)|Defines meaningful commits, documentation changes, verification handoff, and licensing.|

## Plan · `sec:nrz:overview-plan`

The plan gives the dependencies and completion evidence for these bites (`plan:nrz:engine-plan`):

- Bite 1, documentation generator: establish reproducible datasets, templates, and the five renders (`step:nrz:bite-documentation-generator`).
- Bite 1b, exemplar register: seed the seven reference problems and bind adversarial fixtures to feasibility tests (`step:nrz:bite-exemplar-register`).
- Bite 2, numerical feasibility: establish safe backend and validated-arithmetic feasibility before broad APIs (`step:nrz:bite-numerical-feasibility`).
- Bite 3, units design: decide dimensions, measures, counts and uncertainty descriptors through static and data-time prototypes over the agreed generic expression contract (`step:nrz:bite-units-design`), (`req:vision:expression-and-symbol-contract`).
- Bite 4, units and numerical foundations: implement the quantity and intrinsic uncertainty contracts, conversions, algebraic checks and planned data migration around scalar and accuracy foundations (`step:nrz:bite-units-and-foundations`), (`req:vision:unit-data-migration`).
- Bite 5, certified matrix core: establish supported matrix storage, arithmetic, and square solve guarantees (`step:nrz:bite-certified-matrix-core`).
- Bite 6, initial physics core: assemble and evaluate supported physical problems with explicit error coverage (`step:nrz:bite-physics-core`).
- Bite 7, advanced matrix domains: admit further operations through their own domain checks and certificates (`step:nrz:bite-advanced-matrix-domains`).
- Bite 8, accuracy and benchmarks: consolidate whole-problem accuracy, uncertainty comparisons, and recorded performance evidence (`step:nrz:bite-accuracy-and-benchmarks`).

Current bite status belongs to the superloop's records (`conv:nrz:status-records`).

## Licensing · `sec:nrz:overview-licensing`

Code is AGPL-3.0-only under `LICENSE-CODE`; data and documentation are CC0-1.0 under `LICENSE-DOCS`. Code sources carry `SPDX-License-Identifier: AGPL-3.0-only`; documentation, data inputs, and narrative documentation templates carry `SPDX-License-Identifier: CC0-1.0`. Generated material receives its own explicit classification, and third-party material retains its applicable attribution (`rule:vision:metadata-and-licenses`). Contribution requirements are in (`rule:nrz:source-license-classification`).
