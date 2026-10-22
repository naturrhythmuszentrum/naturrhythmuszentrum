<!-- SPDX-License-Identifier: CC0-1.0 -->

# nrz-docgen · `guide:docgen:overview`

## Purpose · `sec:docgen:purpose`

`nrz-docgen` turns committed project inputs and workspace manifests into reviewable datasets and Markdown. It keeps the engine's documentation tied to its declared sources through deterministic generation and a fixed-point check. Its physical expression validation concerns the exact descriptor side of the engine; it produces no floating computation or numerical certificate. Documentation can therefore describe both an implemented contract and a later design without treating the latter as executable evidence.

## Place in the engine · `sec:docgen:engine`

Docgen is a workspace tool that uses bindings and CAS to check original physical declarations. The numerical dependency direction remains matrix → bindings → CAS, with scalar and accuracy beneath matrix. Docgen owns input decoding, dataset derivation, template rendering and drift reporting. It owns neither the exact proof kernel nor float regions, numerical error bounds or a physics scheduler.

The architecture in `docs/design/descriptor-binding.md` has two graphs joined by bindings: exact descriptor structure and a computational graph of float-region versions and kernel applications. Meanings come from bindings, not the floats. Docgen checks the supported declared meanings and records named limitations; it does not execute those numerical bindings or upgrade a units check into an accuracy claim.

## In the LC target · `sec:docgen:lc-target`

The LC reference inputs are `L=25 mH` and `C=10 µF`, with independent answers `ω₀=2000 rad/s` and `f₀=1000/π Hz`. Docgen validates the scalar descriptor expressions admitted by schema v2 and preserves unsupported Complex phase and mixed-operator spectral expressions as explicit limitations. The corpus's hand-derived answers are reference data, not numerical results certified by the generator.

Homogeneous symbol admission is implemented in `crates/bindings/src/admission/` and `crates/matrix/src/binding/`. The descriptor-graph step, the region-bindings step, the coordinate-lowering step, the symbolic-lowering step, the phasor-lowering step, the spectral-lowering step and the numerical-acceptance step in `docs/design/lc-resonance.md` add the remaining LC numerical route. The LC-quantities step will add the minimal schema/recipe binding needed for supported shaped quantities and certified constant/phase evaluation; the LC-plans step will assemble reusable plans. Exact π measure scales already exist in bindings, but rational-only schema and numerical export boundaries still refuse non-rational coefficients. Scalar must enclose an exported constant before matrix can use it with tracked error. Docgen supplies no π evaluator or heap-to-CAS conversion.

## Main concepts and types · `sec:docgen:concepts`

The library functions `data`, `render` and `check` take a repository root. `Finding` and `Class` identify drift, while `Error` retains a path-bearing failure. The executable exposes the same three stages:

- Inputs: edit `data/inputs/project.toml`, the files under `data/problems/`, and the workspace manifests that describe packages.
- Data: derive `data/datasets/project.json`, `data/datasets/census.json`, and `data/datasets/problems.json` in stable order.
- Render: evaluate `data/documents.toml` using `templates/{output}.j2` sources over their declared datasets.
- Check: compare two in-memory generations with each other and with committed output.

Inputs, the document manifest, and datasets identify themselves with `namespace`, `version`, `owner`, `license`, `sources`, and `labels`, in that order. Readers parse the complete syntax, validate the envelope, and then decode and validate the typed payload.

## Guarantees and refusals · `sec:docgen:guarantees`

`check` first scans Git-tracked paths and current UTF-8 text for ordinal schedule codes, then derives datasets and renders every document in memory twice from the committed inputs. Generated paths are disjoint from those inputs and templates, so the second generation reads the same sources; no generated tree is written or overlaid. It compares the generations byte for byte, then compares expected bytes and the managed file inventory with disk, reporting missing, changed, stale and unstable paths. This is documentation reproducibility, separate from the engine's numerical agreement within tracked error.

The named refusal `ordinal-step-code` reports a root-relative path, a one-based line and the matched spelling; line zero identifies a filename. The guard recognizes ordering words with the recorded single-digit plan ordinals, numeric step heads, CAS slice markers and contextual LC substeps. Mathematical symbols, exponents, types, versions and multi-digit algorithm positions retain their meanings. It uses no file allowlist. Untracked files, binary payloads and symbolic-link targets are not scanned; an unavailable Git inventory refuses explicitly. Tests build forbidden examples from separate words so the tracked test sources obey the same rule.

Problem and fixture schema v2 is specified in `docs/design/problem-data-schema.md`. Its strict records declare quantities, counts, metadata, independent uncertainties and executable expressions; bindings checks CAS-owned originals using semantic-header parsing, complete occurrence walks and syntax inspection. `render` reconstructs the context and compares regenerated evidence with committed validation records. Unsupported records retain named limitations and cannot become substitute inputs. Problem/fixture inputs and committed problem datasets require v2.0; v1 is refused with the migration diagnostic, and unknown minors require an explicit compatibility decision. A units check establishes declared consistency, not numerical accuracy, model validity or statistical assurance.

## Current scope and limits · `sec:docgen:scope`

The census lists current workspace members in manifest order; retired packages remain in repository history. The table reproduces package metadata. Its manifest descriptions are not the full architectural contract: the crate READMEs and `docs/design/descriptor-binding.md` distinguish current adapters from the target ownership of meaning.

|Package|Path|Manifest description|
|-------|----|--------------------|
|`nrz-docgen`|`tools/docgen`|Deterministic documentation datasets, rendering, and fixed-point checks|
|`nrz-scalar`|`crates/scalar`|Finite binary element arithmetic with outward enclosures|
|`nrz-accuracy`|`crates/accuracy`|Dimension-neutral nonnegative bounds and conservative accuracy policies|
|`nrz-matrix`|`crates/matrix`|Typed descriptor heap with owned numerical regions and bound companions|
|`nrz-bindings`|`crates/bindings`|Data-time quantity descriptors and checks over original symbolic expressions|
|`nrz-cas`|`crates/cas`|Exact graph construction and checked symbolic derivations|

The project dataset records 9 entries in the broader engine plan. The LC implementation route is specified separately in `docs/design/lc-resonance.md`; a plan entry or generated description does not establish an implemented numerical domain.

## Testing · `sec:docgen:testing`

- `nrz-docgen data [--root DIR]` writes derived datasets.
- `nrz-docgen render [--root DIR]` writes Markdown from the committed datasets.
- `nrz-docgen check [--root DIR]` writes nothing, refuses ordinal schedule codes and reports drift.
- Exit status 0 means success, 1 means a runtime failure, 2 means incorrect usage, and 3 means documentation drift.

The tests in `tests/fixed_point.rs` and `tests/preservation.rs` cover generation stability, drift and preservation behavior. Run `cargo test -p nrz-docgen` for the tool's tests and `cargo run -p nrz-docgen -- check` for the committed workspace fixed point. The tool's checks complement crate doctests and numerical/symbolic oracles; they do not replace them.
