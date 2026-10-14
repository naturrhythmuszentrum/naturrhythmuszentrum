<!-- SPDX-License-Identifier: CC0-1.0 -->

# nrz-docgen · `guide:docgen:overview`

## Purpose · `sec:docgen:purpose`

`nrz-docgen` turns committed project inputs and workspace manifests into reviewable datasets and Markdown. Its fixed-point check makes documentation changes visible alongside their causes (`sec:docgen:fixed-point`).

## Stages · `sec:docgen:stages`

- Inputs: edit `data/inputs/project.toml`, the files under `data/problems/`, and the workspace manifests that describe packages.
- Data: derive `data/datasets/project.json`, `data/datasets/census.json`, and `data/datasets/problems.json` in stable order.
- Render: evaluate `data/documents.toml` using `templates/{output}.j2` sources over their declared datasets.
- Check: compare two in-memory generations with each other and with committed output.

## Commands · `sec:docgen:commands`

- `nrz-docgen data [--root DIR]` writes derived datasets.
- `nrz-docgen render [--root DIR]` writes Markdown from the committed datasets.
- `nrz-docgen check [--root DIR]` writes nothing and reports drift.
- Exit status 0 means success, 1 means a runtime failure, 2 means incorrect usage, and 3 means documentation drift.

## Envelope · `sec:docgen:envelope`

Inputs, the document manifest, and datasets identify themselves with `namespace`, `version`, `owner`, `license`, `sources`, and `labels`, in that order. Readers parse the complete syntax, validate the envelope, and then decode and validate the typed payload.

Problem and fixture schema v2 is specified in `docs/design/problem-data-schema.md`. Its strict records declare quantities, counts, metadata, independent uncertainties and executable expressions; `nrz-units` checks original operations over `nrz-algebra`. `render` reconstructs the context and compares regenerated evidence with committed validation records. Unsupported records retain named limitations and cannot become substitute inputs. Problem and fixture inputs and committed problem datasets require v2.0; v1 is refused with the migration diagnostic, and unknown minors require an explicit compatibility decision. A units check establishes declared consistency, not numerical accuracy, model validity or statistical assurance.

## Fixed point · `sec:docgen:fixed-point`

`check` derives datasets and renders every document in memory, repeats the derivation over the resulting tree, and compares the two generations byte for byte. It then compares expected bytes and the managed file inventory with disk, reporting missing, changed, stale, and unstable paths (`sec:docgen:commands`).

## Workspace · `sec:docgen:workspace`

|Package|Path|Description|
|-------|----|-----------|
|`nrz-docgen`|`tools/docgen`|Deterministic documentation datasets, rendering, and fixed-point checks|
|`nrz-scalar`|`crates/scalar`|Validated scalar arithmetic prototype for Naturrhythmuszentrum|
|`nrz-matrix`|`crates/matrix`|Safe dense solve candidates with independent numerical certificates|
|`nrz-algebra`|`crates/algebra`|Exact symbolic expressions and deterministic collection normal forms|
|`nrz-units`|`crates/units`|Data-time quantity descriptors and checks over original symbolic expressions|
|`nrz-descriptor-prototype`|`prototypes/descriptors`|Descriptor schema interning and checked computation boundary prototypes|
|`nrz-cas`|`crates/cas`|Exact graph construction and checked symbolic derivations|

Naturrhythmuszentrum has 9 planned bites.
