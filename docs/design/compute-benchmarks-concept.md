<!-- SPDX-License-Identifier: CC0-1.0 -->

# Machine compute micro-benchmarks · `spec:nrz:compute-benchmarks-concept`

## Problem and purpose · `sec:nrz:compute-purpose`

**Principle (An instrument for the user's machine)** · `prin:nrz:compute-instrument`

The proposed public `nrz-compute` micro-benchmarks let a library user ask what floating-point arithmetic, memory traffic and representative numerical kernels their machine actually delivers under a recorded load. Instruction mixes, vector widths, working sets and preparation costs help explain why a mathematically small computation can be slow and where a larger computation spends time. A stable report and committed datasets make those observations inspectable and comparable without requiring every reader to repeat the experiment. This concept defines the questions and evidence obligations; it does not fix a Rust API, dataset schema or implementation.

**Caveat (Reproducible computation and variable duration)** · `cav:nrz:compute-reproducibility`

An engine's agreement within tracked numerical error does not imply repeatable elapsed time. Measurements describe a particular executable, machine, workload and load state; their variability is part of the result. Publishing them beside the engine's mathematical contracts is useful precisely when that distinction remains visible. Deterministic Work, host timing and phase tracing retain their separate meanings under (`sec:nrz:tiles-streams-observability`); neither a fast run nor a narrow timing interval establishes numerical correctness.

## What an experiment establishes · `sec:nrz:compute-experiment`

**Definition (Dependency latency and sustained throughput)** · `def:nrz:compute-latency-throughput`

A dependent chain feeds an operation's output into the next operation and probes the cost of waiting for that dependency, including loop and timer effects. Several independent streams permit overlapping execution and probe sustained throughput for the declared stream count, instruction mix and register pressure. Too few streams can remain latency-limited; too many can spill to memory. Report chain length, stream count, timed repetitions and scalar operations per item separately. Neither experiment alone establishes a hardware peak, a vendor instruction latency or the speed of a mixed application. A wall-time result cannot become cycles per operation by assuming the advertised processor frequency.

**Requirement (Scalar and portable widths)** · `req:nrz:compute-widths`

The arithmetic inventory spans scalar binary64 and a declared set of binary64 vector lane counts, assuming safe `std::simd` to express portable lane-wise operations within the workspace's `unsafe_code = "forbid"` boundary. The workspace toolchain selects undated `nightly`; accepting the public crate's feature gate and bounding the width set remain (`open:nrz:compute-crate`). Each run identifies its requested lane count, logical bits, target features and generated execution strategy; unavailable cases remain explicit, not zero-rate results. A one-lane vector, where supported, remains distinguishable from scalar source. Logical width need not equal a physical instruction width: a compiler can split vectors, scalarize an operation or use a helper. Scalar source can also be auto-vectorized when independent operations permit it. Machine-code inspection is evidence for an instruction-level interpretation, not something a portable source type guarantees. Width comparisons report elements per second as well as vector operations per second, and do not assume wider is faster (`def:nrz:compute-latency-throughput`). Portable vector syntax does not widen the supported-platform claim (`dec:vision:supported-platform`).

**Caveat (Compiler and safe-code boundary)** · `cav:nrz:compute-compiler`

Constant folding, unused-output elimination, loop-invariant motion, unrolling and legal instruction reordering can change what a source-level loop measures. Inputs supplied at run time and outputs made observable through `std::hint::black_box` at the measured item's boundary discourage removal and specialization; `black_box` is a best-effort optimization barrier, not an instruction-selection guarantee, hardware fence or prohibition on every reordering. Placing it around every arithmetic operation can instead measure barrier overhead. Under the assumption in (`req:nrz:compute-widths`), safe `std::simd` expresses lane-wise computations without unsafe intrinsics but does not require one instruction per operation or prevent legal optimization. The benchmark must state the actual arithmetic, including fused versus separate operations, and inspect lowering before calling a result instruction throughput. Timer, loop and observation overhead remain visible; an empty-item control is diagnostic rather than a universally valid quantity to subtract (`rule:nrz:host-sampling-repetition`).

**Definition (Cache and memory regimes)** · `def:nrz:compute-memory-regimes`

Vary working-set size across the reported private caches, shared last-level cache and main-memory capacity regimes, recording strides, access order, read/write mix, alignment, allocation and first-touch placement. A size smaller than a nominal cache does not prove residency when other threads compete for it. Warm reuse and cold or capacity-exceeding traversal answer different questions; any eviction procedure belongs to the experiment's provenance. Streaming rates include effects of prefetching, write allocation and cache reuse. Logical bytes processed are not necessarily memory-controller traffic, and a bandwidth plateau alone identifies neither a particular cache level nor random-access latency.

**Caveat (Frequency, topology and shared load)** · `cav:nrz:compute-host-state`

Frequency scaling, turbo, thermal and power limits, and vector-dependent frequency behavior can alter time during a run. SMT siblings share resources; affinity, core type or class on hybrid processors, active-core count, thread migration, NUMA placement, virtualization and competing processes can change the observed rate. Record controls and observations separately, including unavailable telemetry, and retain run order and concurrent load information. Warm-up can establish a different thermal or cache state from a short cold request. Repeated runs and changed ordering help reveal drift, but neither pinning a thread nor a narrow interval proves isolation on a loaded machine. Such observations remain useful as descriptions of that load, not guaranteed unloaded performance.

## Candidate inventory · `sec:nrz:compute-inventory`

**Requirement (Common inventory identity)** · `req:nrz:compute-inventory-identity`

Each candidate names its input domain and generation recipe, arithmetic variant, dependency structure, width, active-thread count and placement (single thread versus scaling across cores), iteration/item unit, output consumption and timed boundary. Arithmetic cases use both dependent chains and independent streams at the widths of (`req:nrz:compute-widths`). Inputs and recurrences must avoid collapsing into constants, zero, subnormal values, infinity or NaN over the declared run; subnormal and exceptional domains, if studied, are separate named cases. Setup, steady execution and output allocation are distinguished so a reader can compare like scopes. FLOP reporting declares its convention, including whether a fused multiply-add counts as two floating operations; operations per second and bytes per second retain their own meanings.

**Definition (Binary64 addition candidate)** · `def:nrz:compute-add`

Addition chains and streams reveal dependency and issue-rate behavior for repeated binary64 sums at each width. They help assess addition-heavy kernels when the operands remain in the intended register regime. They do not predict a reduction with a different dependency tree, memory traversal, compensated sum or tracked-error propagation (`req:nrz:compute-inventory-identity`).

**Definition (Binary64 multiplication candidate)** · `def:nrz:compute-multiply`

Multiplication chains and streams reveal the corresponding repeated-product cost and available overlap. They help distinguish multiplication pressure from addition pressure on the measured target. They do not establish the rate of mixed multiply/add kernels, and a recurrence whose values overflow or decay to zero does not describe sustained ordinary finite multiplication (`req:nrz:compute-inventory-identity`).

**Definition (Binary64 fused multiply-add candidate)** · `def:nrz:compute-fma`

Fused multiply-add cases probe one fused rounding operation with explicit input dependencies, at each width. A dependent chain records which operand carries the dependency: the addend accumulator or a multiplicand; those paths can have different forwarding latency. They show how well an FMA-rich kernel can use the measured execution path. A separate multiply followed by addition has different rounding and potentially different lowering, so it is a separately identified comparison, not an interchangeable name. These rates do not establish matrix performance, which also depends on loads, stores, packing and reuse (`req:nrz:compute-inventory-identity`).

**Definition (Binary64 division candidate)** · `def:nrz:compute-divide`

Division cases use a declared finite operand range and nonzero divisors to expose dependency and throughput costs at each width. They help estimate the importance of division in a kernel's arithmetic mix. Division by a compile-time constant, reciprocal multiplication and approximate reciprocal refinement can execute different work; none may silently stand in for general binary64 division. A result from one operand range does not cover every exponent, exceptional case or helper path (`req:nrz:compute-inventory-identity`).

**Definition (Binary64 square-root candidate)** · `def:nrz:compute-square-root`

Square-root cases use a declared positive finite range to expose root dependency and throughput costs at each width, with range maintenance identified in the timed work where necessary. They help assess the benefit of sharing roots across an axis. Repeatedly taking the root of the previous result can converge to a fixed value, so that loop alone is not a representative sustained root experiment. Reciprocal square root is a different operation, and measured root speed establishes neither an enclosure nor the cost of certified root construction (`req:nrz:compute-inventory-identity`).

**Definition (Memory streaming candidate)** · `def:nrz:compute-streaming`

Read, write and copy streams across working-set sizes reveal attainable logical bandwidth and the transition from reuse to capacity pressure. A named arithmetic stream can show overlap between arithmetic and traffic if its operation and byte counts are explicit. These cases inform data-layout and tile-size experiments, but do not establish random-access latency, exact physical traffic or bandwidth under a different read/write mix, thread count or NUMA placement (`def:nrz:compute-memory-regimes`).

**Definition (Faer matrix-kernel candidates)** · `def:nrz:compute-matrix`

Matrix multiplication, matrix-vector products, and representative factorization and solve kernels on the workspace's faer backend connect primitive rates to numerical work with blocking, reuse and dependencies. Name the kernel, binary64 scalar domain, dimensions including nonsquare shapes, strides, layout, thread count, matrix structure and conditioning recipe. Distinguish allocation, packing, factor construction and repeated solves so a reused factor does not masquerade as a fresh solve. These observations tell a user how the selected backend handles that workload; they establish neither every matrix kernel's speed nor the cost or numerical guarantees of the engine's certified wrappers. The backend integration boundary remains (`rule:vision:backend-boundaries`), with benchmark access left to (`open:nrz:compute-crate`).

**Definition (Optimized naive LC candidates)** · `def:nrz:compute-lc`

Floating-point-only LC candidates use the question families of (`sec:nrz:lc-grid-questions`): forward phase samples, inductance and capacitance outputs, and finite selections from inverse-time families. Forward evaluation uses θ = θ₀ + t/√(L·C); known-product element queries include L = P/C and C = P/L; inverse-time selections use the declared phase difference, cycle index and frequency. Each case identifies its chosen positive domain, shared fixed operands, sample and branch extents, output traffic and generic nonsquare L by C axes. These cases show the attainable rate of a stated optimized naive expression and its memory reuse; a floating branch selection does not prove complete cycle coverage, feasibility, identifiability or any certified LC answer (`rule:nrz:lc-grid-forward-series`), (`rule:nrz:lc-grid-inductance-query`), (`rule:nrz:lc-grid-capacitance-query`), (`rule:nrz:lc-grid-inverse-time`).

**Caveat (Hoisting changes the measured recipe)** · `cav:nrz:compute-lc-hoisting`

For positive real L and C, √(L·C) = √L·√C permits computing roots per axis and sharing a frequency across time samples. An optimized naive candidate should expose such per-axis preparation, reciprocal reuse and any FMA variant rather than repeat avoidable roots per cell. Record both preparation-inclusive and prepared traversal scopes, actual axis lengths, additional storage and amortization population. Binary64 evaluations of the factored and unfactored expressions need not agree bitwise and can have different overflow or underflow behavior; declare the chosen expression and valid input range. This is an optimization experiment, not a proof-preserving replacement of the engine graph and not the theoretical reference of (`claim:nrz:lc-closing-naive-model`).

## Statistics and provenance · `sec:nrz:compute-statistics`

**Principle (Reuse the host-sampling statistical basis)** · `prin:nrz:compute-statistical-basis`

The natural basis is the matrix-owned host sampler and its attributed Criterion statistics extraction in `crates/matrix/src/sampling/`, under (`dec:nrz:host-sampling`) and (`rule:nrz:host-sampling-estimates`). Preserve complete iteration-count/elapsed-time rows in the external raw run report, and retain in the committed summary normalized mean and median, standard deviation, scaled MAD, through-origin slope, bootstrap standard errors and percentile confidence intervals, with raw outliers labelled rather than removed and summary outlier counts retaining that distinction. Summary quantities use the declared decimal grids and outward interval rounding of (`def:nrz:compute-design-grids`); original computed source bits remain in the raw report. Record source and estimator revisions and seed; a seed reproduces analysis of fixed rows, not live timing. Reusing that basis does not import the Criterion harness or its private report files (`rule:nrz:criterion-benchmark-boundary`), (`rule:nrz:host-sampling-criterion-api`).

**Requirement (Sampling protocol and completion)** · `req:nrz:compute-sampling`

The raw result retains requested and achieved sample counts, iterations per sample, independent run count, warm-up procedure and duration, measurement target and elapsed duration, bootstrap count, confidence level, timer, resolution, stop reason and completeness. The existing sampler's validation requirements remain those of (`def:nrz:host-sampling-surface`); this concept introduces no defaults or smaller minima. Warm-up observations are distinguished from estimation rows. Committed summaries retain counts, settings, phase totals and statuses without per-item arrays. A timed item must contain enough meaningful work for the timer's resolution, with its batching and consumption overhead reported. The reused sampler times each iteration separately, so timer-call cost is paid per item; item length must also account for that cost when studying nanosecond-scale arithmetic (`rule:nrz:host-sampling-repetition`). A stopped prefix, failed item or incomplete analysis stays visible without a nominal complete confidence interval. Total observation cost includes preparation and finalization even when the selected kernel timer excludes them (`rule:nrz:host-sampling-repetition`), (`rule:nrz:host-sampling-deadline`).

**Caveat (Intervals are conditional evidence)** · `cav:nrz:compute-intervals`

Bootstrap intervals describe uncertainty in the selected statistic under representative, sufficiently stable sampling assumptions; they are neither numerical-error enclosures nor worst-case or future-run guarantees. Correlated iterations or drift do not become independent evidence by increasing the cell count. Preserve chronological rows in external raw reports and independent run identities in summaries so aggregation does not hide changing load, and report differing regimes separately. Summary-only readers cannot inspect within-run drift without the raw report. For a positive per-item time and positive interval endpoints, rate intervals invert the endpoints under (`rule:nrz:host-sampling-estimates`). Narrow intervals cannot repair biased inputs, wrong scope or compiler removal, and overlapping or disjoint marginal intervals alone are not a declared cross-machine comparison procedure.

**Requirement (Environment and experiment provenance)** · `req:nrz:compute-provenance`

Retain source commit and any tree modifications, benchmark and workload revisions, input recipe or digest, executed command and arguments, run timestamp, toolchain and compiler/backend versions, build profile, optimization and target flags, enabled features and dependency versions. Record CPU identity and available instruction features, topology and affinity, the core type or class that executed the work, SMT and active threads, caches and memory configuration, NUMA policy, operating system/kernel, container or virtual-machine limits, power/frequency policy, turbo state, observed thermal/frequency information where available, background load and tracing configuration. Requested settings and observed states are distinct; unknown information stays explicitly unknown. Such provenance extends the observation identity of (`def:nrz:engineering-measurement-identity`), without publishing credentials or private host identifiers.

**Rule (Comparison preserves the measured question)** · `rule:nrz:compute-comparison`

Cross-machine comparisons keep workload revision, inputs, arithmetic semantics, item/byte counts, timed boundary and statistical method explicit while identifying machine differences as the experiment's variables. Cross-run comparisons identify changed compiler flags, widths, thread counts, cache state and load rather than pooling them under one name. Report unsupported and incomplete cases alongside completed cases. A source revision or environment mismatch may still be informative, but it is not evidence attributing a change to one cause (`rule:nrz:engineering-measurement-comparison`), (`req:nrz:compute-provenance`). Such comparisons remain within the x86_64 Linux support scope unless the evidence required by (`dec:vision:supported-platform`) is added first.

## Datasets and documentation · `sec:nrz:compute-datasets`

**Principle (Commit summaries so readers can inspect them)** · `prin:nrz:compute-datasets`

Summary datasets belong in versioned records committed under `data/`; raw collection output remains outside the repository and is never committed. Summaries retain case identity, integer counts and captured integer durations, host-computed statistics as exact decimal strings with declared units and grids, completion/analysis status and public provenance, distinguishing missing evidence from zero. Point estimates round to nearest with ties away from zero; interval endpoints round outward. Fields spanning an unsuitable range for a fixed grid declare significant figures. Canonical numeric bytes follow the exact values and quantity declarations, independent of serializer float formatting; raw source bits preserve computed binary64 values for regeneration. The concrete field declarations and integer-only rounding are (`def:nrz:compute-design-numbers`), (`def:nrz:compute-design-grids`) and (`alg:nrz:compute-design-decimal-rounding`). A deterministic summarize step binds each summary to its raw report's content digest and schema revision; a holder can regenerate it, while a summary-only reader cannot reconstruct omitted observations or independently recompute statistics. Reviewable summaries trace displayed figures to their workload and machine, preserve historical characterization and permit deterministic rendering without that host or its raw report. No dataset or measurement is supplied by this concept; the schema and admission contract remain questions for design.

**Rule (Rendering reads recorded data only)** · `rule:nrz:compute-render`

`nrz-docgen` renders committed datasets through registered templates and checks the generated fixed point; it never runs a benchmark during data derivation, rendering or checking. Measurement collection, deterministic summarization and explicit summary admission precede documentation generation, following (`def:nrz:engineering-measurement-identity`) and (`[DOCGEN-rule:docgen:recorded-observations]`). Rendering validates committed summaries and derives display context only; it never reads raw timing rows, refreshes measurements from the render host, executes recorded commands or invents missing samples. Pages retain measurement provenance and incomplete status, rather than presenting a historical result as current engine capability (`def:nrz:engineering-measurement-currency`).

## Boundaries of authority · `sec:nrz:compute-boundaries`

**Rule (Measurements are never correctness or test oracles)** · `rule:nrz:compute-no-oracle`

Measured durations, rates, rankings and confidence intervals are never correctness oracles and are never asserted by a test. Tests may check deterministic report validation, fixed-row statistics, workload semantics and refusal/completion states, but do not assert a live performance figure or compare it with a stored machine result as a pass condition. Observations cannot set engine limits, defaults, pins, Work charges, numerical algorithms or mathematical refusals (`dec:nrz:host-sampling`), (`req:nrz:host-sampling-acceptance`).

**Rule (The LC overhead reference remains theoretical)** · `rule:nrz:compute-model-boundary`

The sole reference of the LC certification overhead bound remains the theoretical optimized naive binary64 model in (`claim:nrz:lc-closing-naive-model`) and (`claim:nrz:lc-closing-overhead-bound`). `nrz-compute` observations, including optimized naive LC runs, neither replace nor calibrate that reference into a measured baseline. `nrz-compute` rates, including primitive cases, are not sources for the model's machine throughputs or memory bandwidth; any proposal to allow that must be raised against (`claim:nrz:lc-closing-naive-model`) in the LC resonance specification, not decided here. They may appear beside the model, labelled as one machine under one load, with their separate scope and uncertainty. They do not discharge the predicted component budget or measured certified-path accounting obligations of (`claim:nrz:lc-closing-overhead-budget`) and (`claim:nrz:lc-closing-overhead-tracking`); a slow naive implementation cannot relax the bound.

## Questions handed to design · `sec:nrz:compute-open`

**Open Question (Public invocation and bounded runs)** · `open:nrz:compute-api`

What public library surface selects cases, supplies inputs, returns stable reports and exposes unsupported or partial outcomes, and what optional command interface serves explicit collection? How are total time, iteration counts and storage bounded across preparation, warm-up, timing, analysis and cleanup? The design must distinguish cooperative admission deadlines and their finalization allowances from hard termination, account for shared-host scheduling delay, and decide when a host-owned process is required (`rule:nrz:host-sampling-deadline`). It must not promise that a measurement target alone bounds return time or start background measurements implicitly.

**Open Question (Dataset and report schema)** · `open:nrz:compute-schema`

How are the external raw report and committed summary separately versioned, with workload identity, units, widths, run groupings, requested/achieved counts, estimator revisions, environment and stop states? Who admits summaries from other machines, how are source/build identity, recipes, load, completeness and raw digests reviewed, and how are public fields scrubbed? How does external raw retention enable deterministic regeneration, and which verification claims remain possible for summary-only readers? Collection remains nondeterministic while summarizing, rendering and checking fixed inputs are deterministic (`rule:nrz:compute-render`).

**Open Question (Crate boundary and portable SIMD assumptions)** · `open:nrz:compute-crate`

Where should the public `nrz-compute` instrument and its workload adapters live so sampling statistics are reused without copying a competing estimator or reversing the architecture's dependency direction (`rule:vision:layer-enforcement`)? Is requiring `#![feature(portable_simd)]` in a public crate acceptable, and which supported toolchains does that imply? How is the admitted width set bounded so oversized logical vectors that primarily probe splitting and register pressure are explicitly identified rather than assumed to represent native widths (`def:nrz:compute-latency-throughput`)? The design must use existing workspace dependencies only so the public instrument does not widen the published dependency graph, preserve matrix's ownership of faer integration under (`rule:vision:backend-boundaries`), and specify access to matrix kernels without silently measuring certified wrappers as raw kernels. It must keep the existing `publish = false` Criterion host `crates/bench` distinct from the public instrument: Criterion remains confined by (`rule:nrz:criterion-benchmark-boundary`), and its console output and private JSON are not the stable report. No crate, manifest edge, dependency pin or backend exception is introduced here.

**Open Question (Build target and baseline reporting)** · `open:nrz:compute-target`

How does a library user select an executable that reaches the intended machine instruction set: compile-time `target-cpu` or `target-feature` flags, or another route consistent with the workspace's `unsafe_code = "forbid"` boundary? A runtime call to a `#[target_feature]` function from code without that feature would require unsafe code. How are baseline builds and their actual lowering labelled so split vectors or a software `mul_add` helper are not presented as native wide-vector or hardware FMA throughput (`req:nrz:compute-widths`), (`cav:nrz:compute-compiler`)? Resolve the build and reporting policy alongside the recorded flags and instruction features in (`req:nrz:compute-provenance`). These build choices remain within the supported platform (`dec:vision:supported-platform`).

**Open Question (Rendered organization)** · `open:nrz:compute-pages`

Should pages group results by machine, workload family or comparison question, and how should readers navigate widths, working sets and independent runs without hiding incompatible scopes? Decide table/plot organization, provenance links and presentation of unknown, unsupported and incomplete cases while keeping every displayed figure attached to its committed record. Any juxtaposition with the theoretical LC model must preserve (`rule:nrz:compute-model-boundary`), and every page must obey (`rule:nrz:compute-render`).
