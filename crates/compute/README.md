<!-- SPDX-License-Identifier: CC0-1.0 -->

# Compute instrumentation · `guide:compute:overview`

## Purpose · `sec:compute:purpose`

The unpublished `nrz-compute` instrument provides collection and exact summary surfaces for (`[DOCS-spec:nrz:compute-benchmarks]`). Host wall-time observations describe a machine under a recorded load. They are neither a correctness oracle nor the theoretical reference of the LC overhead bound.

## Exact quantities · `sec:compute:exact-quantities`

The decimal surface decodes finite binary64 bits and declared decimal/integer source classes into fixed-capacity integer rationals. Shared statistic grids retain nonzero dispersion; directed endpoints enclose exact source intervals. Reciprocal rates round the exact numerator/time ratio once. The codec grammar rejects alternate decimal spellings. The declared 54-limb capacity and worked fixtures follow (`[DOCS-alg:nrz:compute-design-decimal-rounding]`).

## Requests · `sec:compute:requests`

The catalog performs no measurement. Requests carry explicit case order, independent seeds, capacities and sampling settings. Validation rejects unencodable budgets and execution bounds before workload preparation and rejects colliding effective generator offsets. Library calls install no process-global subscriber or implicit benchmark.

## Evidence boundary · `sec:compute:evidence-boundary`

Only x86_64 Linux is claimed. Build facts are captured with read-only Git commands. Raw observations remain outside the repository. Summary admission refuses missing repository byte policies; the adopted design policy awaits implementation under (`[DOCS-req:nrz:compute-admission-activation]`).

## Library and command surfaces · `sec:compute:surfaces`

`catalog()` returns the fixed operation vocabulary without probing the machine. `RunRequest::validate` checks explicit policy; `run(&request, &context)` returns an owned `RunReport` containing every requested case/repetition, including unsupported, incomplete and unvisited outcomes. Complete measurements expose typed seconds and rate views, while raw evidence retains exact integer durations, source bits and decimal views. Independent repetitions are not pooled.

`write_report` and `read_report` validate the current raw schema; the decoder also requires byte-identical canonical re-encoding. `summarize(&raw, &raw_id)` is a pure projection of captured evidence and the caller-supplied canonical Git blob identity. `write_summary` and `read_summary` handle its fixed statistical shapes. These functions perform no measurement, telemetry acquisition or command execution. The caller remains responsible for binding a supplied digest to the raw bytes.

The binary accepts `collect --request PATH --output PATH` and `summarize --raw PATH --output PATH`. Collection requires an explicit external raw destination and independent export byte/time caps. Publication uses a synced sibling staging file followed by atomic rename. The summarizer hashes canonical raw bytes through an isolated temporary Git context without storing the raw blob. Exit codes are zero for complete collection or successful summarization, one for an exported partial collection, and two for request/export failure.

## Workload scopes · `sec:compute:workload-scopes`

The inventory includes seven scalar/SIMD arithmetic variants, four contiguous streaming operations, serial raw matrix product/factor/solve operations through the matrix-owned benchmark feature, and five optimized naive LC expressions. Every selected width, dependency, shape, worker count and preparation/restoration scope remains part of the case identity. Portable SIMD expresses a logical width; machine-code lowering remains unknown without separate inspection.

Workers use bounded generation-tagged channels, prepare their own disjoint buffers and are joined before return. Pilot controls and workload items use the selected dispatch boundary. Declared host execution bounds are caller assumptions; observed pilot times cannot establish or enlarge them. Repository admission is a separate byte-policy and review-receipt check, with raw evidence retained externally.

## Owned reservations · `sec:compute:owned-reservations`

The collector initializes one failure record per requested case and repetition before capture or workload preparation. Owned request copies use fallible reservations and charge actual vector and string capacities; caller spare capacity is excluded. Worker recipe copies and matrix recipe copies reduce the allowance passed to their kernels. Streaming buffers and LC inputs, output, shared preparation and simultaneous root scratch use a single decreasing allowance before initialization. Private capture and its simultaneous public projection contribute their actual retained capacities. Blocking transports reserve one command and completion slot per worker alongside the handle population. The nested sampler receives a separate allowance for its iteration records, rows and analysis; conversion slots are reserved before sampling. Probe reads use a fixed byte buffer with a separate truncation sentinel. These reservations support (`[DOCS-rule:nrz:compute-design-budget]`); opaque-packed backend shapes are refused under (`[DOCS-open:nrz:compute-backend-packing-requirement]`) because no requirement route is available.

## LC recipe ledgers · `sec:compute:lc-ledgers`

Raw and summary schema revision 1.1.0 carries the LC preparation, traversal, storage and amortization ledger. Roots, divisions, multiplies, adds and fused operations remain separate, and only the selected timed scope contributes to rate numerators. Both summary decoders reconstruct these counts from the public input populations; exact input bits remain bound by the raw content reference. Nonsquare forward, shared-denominator and finite inverse-branch fixtures implement (`[DOCS-def:nrz:compute-design-lc]`) without timing assertions. Earlier raw or summary revisions are refused; the empty admission index requires no measured-data migration.

## Phase accounting · `sec:compute:phase-accounting`

Reset and dry-validation duration is recorded separately from kernel item time. Pilot wall time excludes that separately recorded reset duration. Worker release and owned report conversion have explicit finalization measurements; unclassified harness and sampler work remains a residual field and is not relabelled as finalization. Both raw and summary duration validation include these fields in the elapsed-time ledger.

## Captured environment and lowering · `sec:compute:environment-evidence`

The environment separates process limits, resolved cgroup CPU/memory/cpuset controls, coordinator and worker placement, NUMA allowance, core-class evidence, and frequency-policy bounds. Before/after snapshots retain signed native temperature, integer kHz and exact decimal MHz sources; unavailable probes remain tagged. An observed frequency never converts wall time into cycles. Structured lowering fields retain inspection status, artifact content identity, symbols, vectorization, splitting, helper and FMA evidence. Known instruction claims require an explicitly inspected record, independently of baseline/native build intent (`[DOCS-rule:nrz:compute-design-baseline]`).

### Worker storage and case protocol · `sec:compute:worker-storage-protocol`

Each started worker records actual numerical buffer capacities, live bytes, row and column strides, and base-address residues modulo 64. No address is exported. Worker identity and first-touch ownership remain separate from a procfs startup CPU snapshot: that snapshot does not claim continued CPU residence. NUMA page residence is explicitly unavailable through the safe runtime. These records are bounded per worker, never per observation (`[DOCS-def:nrz:compute-design-case-identity]`).

The case protocol names reset, warm reuse without cache flushing, direct calls or blocking-channel start/wait, read reduction partials and scalar tail population, and matrix conditioning and timed restoration populations. Exact conditioning margins remain bound to the raw case pointer. Both summary decoders verify these identities and reject inconsistent worker ownership, address residues and reduction tails (`[DOCS-def:nrz:compute-design-wire]`).

### Explicit raw reanalysis · `sec:compute:raw-reanalysis`

`replay::verify_raw_analysis` reconstructs complete statistics from the recorded row source bits, exact confidence and repetition seed using the named available estimator. It compares every estimate, fence, outlier count and chronological classification. It takes an explicit cancellation callback, charges the reconstructed rows against analysis scratch, and never reads a clock or collects a workload. Ordinary summary decoding does not invoke reanalysis. A summary-only receipt therefore remains distinct from a raw-reanalyzed receipt (`[DOCS-rule:nrz:compute-design-admission]`).

Matrix protocol records disclose column-major generation and ascending diagonal summation, with a condition estimate explicitly marked uncomputed. Repeated factors restore before every traversal; prepared solves restore between traversals. Each restoration retains its copied bytes and its separate logical read/write bytes. The accepted item length determines the traversal and restoration populations; these timed copies are included in the reported scope (`[DOCS-def:nrz:compute-design-matrices]`).

Initial environment capture derives its probe allowance from the caller's scratch cap, including truncation sentinels, bounded cache-field joins and cgroup parsing. Embedded build constructors reserve every captured string and vector through the shared report allowance without a runtime JSON decoder. Public flag redaction happens before cloning public build fields, so a private flags copy is never allocated merely to discard it. Request copies, upfront failure records and retained capture share one report ownership allowance (`[DOCS-rule:nrz:compute-design-budget]`).

Caller command text and public projection copies reserve each string and vector fallibly against the shared report allowance. Public path and flag redaction occurs before copying; metadata headers and actual returned capacities are charged together. The machine alias is admitted against capture scratch before any host probe.
