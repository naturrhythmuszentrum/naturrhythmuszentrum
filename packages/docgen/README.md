<!-- SPDX-License-Identifier: CC0-1.0 -->

# nrz-docgen · `guide:docgen:overview`

## Purpose · `sec:docgen:purpose`

`nrz-docgen` turns committed project inputs and workspace manifests into reviewable datasets and Markdown. Its fixed-point check makes documentation changes visible alongside their causes (`sec:docgen:fixed-point`).

## Stages · `sec:docgen:stages`

- Inputs: edit `docs/inputs/project.toml`, the files under `problems/`, and the workspace manifests that describe packages.
- Data: derive `docs/datasets/project.json`, `docs/datasets/census.json`, and `docs/datasets/problems.json` in stable order.
- Render: evaluate the document manifest's adjacent templates over their declared datasets.
- Check: compare two in-memory generations with each other and with committed output.

## Commands · `sec:docgen:commands`

- `nrz-docgen data [--root DIR]` writes derived datasets.
- `nrz-docgen render [--root DIR]` writes Markdown from the committed datasets.
- `nrz-docgen check [--root DIR]` writes nothing and reports drift.
- Exit status 0 means success, 1 means a runtime failure, 2 means incorrect usage, and 3 means documentation drift.

## Envelope · `sec:docgen:envelope`

Inputs, the document manifest, and datasets identify themselves with `namespace`, `version`, `owner`, `license`, `sources`, and `labels`, in that order. Readers parse the complete syntax, validate the envelope, and then decode and validate the typed payload.

## Fixed point · `sec:docgen:fixed-point`

`check` derives datasets and renders every document in memory, repeats the derivation over the resulting tree, and compares the two generations byte for byte. It then compares expected bytes and the managed file inventory with disk, reporting missing, changed, stale, and unstable paths (`sec:docgen:commands`).

## Workspace · `sec:docgen:workspace`

|Package|Path|Description|
|-------|----|-----------|
|`nrz-docgen`|`packages/docgen`|Deterministic documentation datasets, rendering, and fixed-point checks|
|`nrz-scalar`|`packages/scalar`|Validated scalar arithmetic prototype for Naturrhythmuszentrum|
|`nrz-matrix`|`packages/matrix`|Safe dense solve candidates with independent numerical certificates|
|`nrz-algebra`|`packages/algebra`|Exact symbolic expressions and deterministic collection normal forms|
|`nrz-units`|`packages/units`|Data-time quantity descriptors and checks over original symbolic expressions|

Naturrhythmuszentrum has 9 planned bites.
