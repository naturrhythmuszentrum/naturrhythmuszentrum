<!-- SPDX-License-Identifier: CC0-1.0 -->

# Conceptual architecture of the Naturrhythmuszentrum engine · `spec:vision:conceptual-architecture`

## Vision and principles · `sec:vision:vision-and-principles`

**Definition (Purpose and audience)** · `def:vision:purpose-and-audience`

Naturrhythmuszentrum is a matrix-based physics calculation engine for researchers, engineers, and developers who need numerical results whose dimensions, accuracy, and assumptions remain inspectable. It represents bodies, constraints, fields, and computational dependencies with petgraph, performs linear algebra through faer, and exposes physical quantities through composite types built from the seven SI base dimensions. Its central product is a result accompanied by evidence about its numerical reliability, conditional on a stated mathematical model and declared input uncertainty.

**Principle (Accuracy before speed)** · `prin:vision:accuracy-before-speed`

Accuracy determines whether a computation is acceptable; speed determines which acceptable implementation is preferable. This ordering prevents a fast result from concealing inadequate precision or an unsuitable algorithm. For example, a faster factorization loses to a slower solve when only the latter establishes the requested error budget, while equally adequate algorithms are compared by measured time and memory.

**Principle (Dimensions survive abstraction)** · `prin:vision:dimensions-survive-abstraction`

Physical dimensions remain present through scalar arithmetic, matrix operations, solver interfaces, and graph evaluation because dimensional mistakes are structural errors that compilation can prevent. For example, solving a homogeneous system derives the solution dimension from the right-hand side and operator dimensions; it does not require callers to reconstruct that information from comments.

**Principle (Every physical value carries evidence)** · `prin:vision:values-carry-evidence`

Every public physical scalar and matrix carries an error companion because an isolated floating-point value cannot communicate its accuracy. Exact structural metadata, such as indices and permutations, is outside this requirement. For example, importing measured values requires explicit uncertainty information or an explicit statement that only their represented numerical values are being modeled; absence of metadata never silently means physical exactness.

**Principle (Guarantees name their scope)** · `prin:vision:guarantees-name-scope`

Deterministic bounds, statistical uncertainty, numerical estimates, and modeling assumptions remain distinguishable because they support different conclusions. A certified solve bounds the solution of the declared mathematical problem over its declared admissible inputs; it does not certify the physical adequacy of that problem. For example, a small residual cannot erase uncertainty from an approximate constitutive law.

**Principle (Safety defines implementation boundaries)** · `prin:vision:safety-defines-boundaries`

Workspace code uses safe Rust and preserves the workspace-wide prohibition on unsafe code because performance cannot justify introducing unchecked memory obligations into engine code. Dependency integration uses safe public APIs. For example, matrix storage is designed around directly owned backend scalars instead of requiring an unsafe cast from slices of unit wrappers.

**Principle (Documentation is reproducible data)** · `prin:vision:documentation-is-reproducible`

Every repository Markdown document is a committed render of a minijinja template over hand-edited TOML inputs and derived JSON datasets because independently edited explanations and generated facts otherwise diverge. For example, changing a package dependency updates its derived documentation and committed render in the same meaningful commit.

**Principle (Boundaries remain independently reviewable)** · `prin:vision:reviewable-boundaries`

Packages, computational stages, and commits have explicit responsibilities because numerical trust depends on understanding where assumptions enter and where they are checked. For example, the matrix layer owns factorization certificates while the physics layer owns the decision that a particular constraint system represents a physical problem; neither silently supplies the other’s missing assumptions.

## Numerical model · `sec:vision:numerical-model`

**Decision (Precision selection)** · `dec:vision:precision-selection`

Precision is a scalar type parameter inside the numerical engine, initially supporting `f32` and `f64`, with `f64` as the default. Applications may select between complete typed computations through runtime dispatch at a problem boundary. This combines specialization of hot kernels with configurable execution without allowing a setting change to reinterpret existing storage. Extended precision is an admission path, not an initial capability: it requires compatible safe faer integration, validated arithmetic, and evidence that the additional accuracy justifies its cost. Exemplars: (`ex:exemplars:arb`), (`ex:exemplars:drake`).

**Table (Precision alternatives)** · `tab:vision:precision-alternatives`

|Alternative|Deciding criterion|Cost and disposition|
|-----------|------------------|--------------------|
|Type parameter throughout|Preserve specialized kernels and explicit conversion boundaries|Selected internally; increases compilation and potentially binary size through monomorphization.|
|Runtime scalar dispatch within arithmetic|Permit arbitrary precision changes inside one container|Rejected initially because dispatch, representation complexity, and backend compatibility would affect every operation.|
|Runtime selection of typed calculations|Configure a problem without obscuring its actual arithmetic|Selected externally; pays dispatch at coarse boundaries and requires explicit handling of differently typed outputs.|
|Arbitrary or extended precision immediately|Establish an accuracy need and a compatible backend|Deferred; storage, kernel support, and certification costs require measurement before admission.|

Exemplars: (`ex:exemplars:arb`), (`ex:exemplars:drake`).

**Rule (Precision conversions)** · `rule:vision:precision-conversions`

Conversion creates a new value and updates its numerical envelope because changing representation can introduce rounding even when dimensions remain unchanged. Widening preserves existing uncertainty; it cannot recover information already lost. Narrowing includes conversion error and rechecks applicable budgets. An authorized promotion policy may retry an entire operation or computation at higher precision, but records the attempt and returns an explicitly identified precision. Exemplars: none.

**Decision (Accuracy policy)** · `dec:vision:accuracy-policy`

A calculation policy selects assurance requirements, algorithms, refinement limits, and permitted precision escalation. Public operations default to requiring certified deterministic evidence for their declared admissible inputs; diagnostic estimates require explicit selection. Observable budgets use an absolute tolerance with the observable’s dimension and a dimensionless relative tolerance against a declared characteristic scale of that dimension. The accepted bound is at most the absolute tolerance plus the relative tolerance multiplied by that scale. Explicit scales avoid making near-zero outputs or arbitrary unit choices define acceptance accidentally. Exemplars: (`ex:exemplars:higham-accuracy-and-stability`), (`ex:exemplars:sundials-and-differentialequations-jl`).

**Rule (Local and observable budgets)** · `rule:vision:local-and-observable-budgets`

An operation can track certified error without a local tolerance, but reports that no local budget was specified; it cannot report budget success in that case. Physics evaluations require budgets for requested observables because accuracy is otherwise undefined at the application boundary. Intermediate allocations are policy choices that may be revised during refinement, while the final observable budget remains authoritative. Exemplars: (`ex:exemplars:sundials-and-differentialequations-jl`).

**Decision (Factorizations and scaling)** · `dec:vision:factorizations-and-scaling`

The default square nonsingular solve uses pivoted LU, with partial pivoting as the initial candidate and explicit attention to growth and residuals. Rectangular least-squares problems use rank-revealing QR candidates, with SVD considered when rank separation or accuracy demands it. Cholesky is selected only for a supported symmetric positive-definite problem contract. Normal equations are excluded as the default least-squares route because they amplify conditioning difficulties. These are required adapter behaviors, not assertions about a particular faer release’s available APIs. Exemplars: (`ex:exemplars:higham-accuracy-and-stability`), (`ex:exemplars:kahan-and-wilkinson-matrices`).

**Rule (Equilibration)** · `rule:vision:equilibration`

Row and column scaling is considered when magnitude imbalance threatens accuracy, using recorded, invertible, dimensionless transformations because results and certificates must translate back to the original problem. Powers-of-two scaling is preferred where representable without damaging underflow because it can reduce additional rounding. Scaling never substitutes for conditioning analysis: improving the representation does not necessarily remove intrinsic sensitivity. Exemplars: (`ex:exemplars:higham-accuracy-and-stability`), (`ex:exemplars:kahan-and-wilkinson-matrices`).

**Decision (Reductions and residuals)** · `dec:vision:reductions-and-residuals`

Engine-controlled scalar reductions use compensated summation where its measured benefit warrants the cost; compensation improves accuracy but does not itself constitute a bound. Certified residuals and norms use an independently validated arithmetic path with conservative rounding treatment. This distinction allows faer to compute candidates efficiently while verification establishes what those candidates mean, without assuming an undocumented reduction order inside backend kernels. Exemplars: (`ex:exemplars:error-free-transformations`), (`ex:exemplars:extra-precise-iterative-refinement`).

**Decision (Refinement and mixed precision)** · `dec:vision:refinement-and-mixed-precision`

Iterative refinement is the first recovery mechanism after a useful factorization because correction solves can improve accuracy without repeating the most expensive work. A policy may factor in `f32` and evaluate residuals and corrections in `f64`, but accepts the result only after verification in the reported guarantee model. Refinement stops on success, stagnation, non-finite arithmetic, or an explicit iteration limit; failure triggers an authorized higher-precision retry or a numerical failure. Mixed precision is an optimization demonstrated by certificates and benchmarks, never an assumption that extra iterations restore missing digits. Exemplars: (`ex:exemplars:extra-precise-iterative-refinement`).

**Rule (Conditioning and refusal)** · `rule:vision:conditioning-and-refusal`

Condition estimates guide algorithm selection and explain sensitivity, but only validated bounds support certified conclusions. There is no universal condition-number cutoff because acceptable sensitivity depends on input uncertainty and the requested output budget. A computation refuses the requested guarantee when its perturbation margin cannot be established, its certified bound exceeds the budget, or its domain assumptions cannot be checked. An unsuccessful certificate means “cannot certify,” not “proved singular” or “proved inaccurate.” Exemplars: (`ex:exemplars:intlab`), (`ex:exemplars:hilbert-matrices`).

**Decision (Initial numerical domains)** · `dec:vision:initial-numerical-domains`

Initial production coverage is real dense arithmetic and supported real solve problems. Symmetric spectral problems follow once their structure and gap conditions can be validated. General spectra, complex results, and matrix functions require separately admitted domains because branch choices, defective eigenproblems, and singularities need guarantees beyond those of a solve. Unsupported requests return an explicit failure instead of projecting a complex result into real arithmetic or borrowing an unrelated certificate. Exemplars: none.

## Error-native, matrix-based error tracking · `sec:vision:error-native-matrix-tracking`

**Definition (Admissible values)** · `def:vision:admissible-values`

A scalar quantity consists of a represented center, its dimension, an error companion, and provenance. A matrix consists of backend-native center storage, shape and structural metadata, a dimension descriptor, and the corresponding companion. For deterministic tracking, a matrix represents admissible values of the form its center plus a perturbation whose Frobenius norm is bounded by a radius. Scalars use absolute bounds; vectors use Euclidean bounds. Structural restrictions, such as symmetry, further restrict the admissible set when explicitly established. Exemplars: (`ex:exemplars:arb`).

**Decision (Primary error model)** · `dec:vision:primary-error-model`

The primary model combines backward-error accounting for numerical algorithms with conservative forward envelopes for public values. Residuals establish how well a computed candidate satisfies its defining equation; verified sensitivity information converts that evidence into an enclosure of the requested result. Frobenius-radius envelopes are the initial representation because they compose with ordinary dense storage, have modest metadata cost, and bound the spectral norm where needed. Their conservatism is accepted initially and measured before introducing more expensive componentwise representations. Exemplars: (`ex:exemplars:intlab`), (`ex:exemplars:arb`), (`ex:exemplars:higham-accuracy-and-stability`).

**Table (Error models, costs, and user meaning)** · `tab:vision:error-model-comparison`

Here, `n` is a representative dense matrix order, `T` is the cost of the underlying operation, `k` is a repeated-evaluation count, and `p` is a retained uncertainty-symbol count. Costs describe architectural expectations rather than measured performance. Exemplars: (`ex:exemplars:higham-accuracy-and-stability`), (`ex:exemplars:arb`), (`ex:exemplars:cadna-and-verificarlo`), (`ex:exemplars:measurements-jl`), (`ex:exemplars:interval-arithmetic-jl`).

|Model|Promise and deciding criterion|Time and memory|Interaction with faer and user interpretation|
|-----|------------------------------|---------------|---------------------------------------------|
|Forward bounds with conditioning|Encloses output error only when input bounds, rounding analysis, and sensitivity bounds are valid; judged by enclosure validity and tightness|Basic propagation adds matrix scans; verified sensitivity may require additional quadratic work per solve or cubic verification; scalar-radius metadata is small|Plain kernels produce centers; separate analysis supplies a bound. Users read a conditional worst-case distance, not a probability.|
|Backward-error accounting|Establishes a nearby problem solved by the candidate; judged by independently checkable residuals and convertibility to useful forward bounds|Residuals cost matrix products or matrix-vector products; scratch storage is usually quadratic; rigorous inverse-norm verification may be cubic|Fits candidate-producing kernels directly. Users read residual size and perturbation margin alongside the resulting forward enclosure.|
|Interval or affine arithmetic|Interval methods enclose admissible results; affine methods can retain selected dependencies; judged by enclosure strength against dependency growth|Interval storage is roughly endpoint pairs before overhead; operations have larger constants. Affine storage can grow as `O(p n²)` and straightforward cubic algebra as `O(p n³)`|Full arithmetic does not automatically fit plain-scalar kernels. Verified wrappers use faer candidates; users read ranges or correlated uncertainty forms.|
|Stochastic arithmetic|Measures variation under a specified perturbation or rounding experiment; judged by diagnostic sensitivity, never unconditional enclosure|Approximately `k T`; simultaneous storage can approach `O(k n²)`, or sequential runs trade memory for aggregation|Requires an actual rounding experiment, not merely rerunning deterministic kernels. Users read empirical variation with its sampling assumptions.|
|Derivative-based covariance propagation|Approximates output uncertainty under a probabilistic input model and local linearization; judged by whether those assumptions are appropriate|Directional methods reuse solves; dense covariance over all `n²` entries can require `O(n⁴)` storage, with substantial derivative work|Plain kernels can evaluate derivative formulas. Users read variances, correlations, and model-dependent confidence statements, not rounding guarantees.|

**Table (Composition across matrix operations)** · `tab:vision:error-model-operation-composition`

|Model|Decomposition|Solve and inverse|Eigenproblems and matrix functions|
|-----|-------------|-----------------|----------------------------------|
|Forward bounds|Factor-entry bounds need factor sensitivity; reconstruction alone is insufficient|Sensitivity converts residual and input bounds to output bounds; inverse output can be especially expensive and sensitive|Eigenvalue and eigenvector conditioning differ; functions require domain-specific perturbation bounds. A solve condition number cannot be reused indiscriminately.|
|Backward error|Reconstruction and orthogonality residuals provide relational certificates|Residuals support robust candidate checking; forward accuracy additionally needs inverse-norm information|Eigenpair residuals support nearby-problem claims; matrix functions require certificates tied to the particular function or defining relation.|
|Interval or affine|Naive interval pivoting can fail when a pivot enclosure contains zero; dependency growth can dominate|Verified inverse or residual techniques can enclose solutions using ordinary candidate factorizations|Spectral enclosures and function-domain checks need specialized methods; a successful interval solve does not establish these capabilities.|
|Stochastic|Pivot or rank changes can produce discontinuous run-to-run behavior|Repeated solutions expose some sensitivity but may miss common systematic error|Eigenvalue matching and branch changes complicate aggregation; low variation does not establish correctness.|
|Covariance|Derivatives may fail at rank changes and unstable pivot selections|Differentiating the defining equation allows factorization reuse; inverse derivatives inherit sensitivity|Repeated eigenvalues undermine individual eigenvector derivatives; functions require suitable Fréchet derivatives and domain separation.|

Exemplars: (`ex:exemplars:higham-accuracy-and-stability`), (`ex:exemplars:intlab`), (`ex:exemplars:arb`), (`ex:exemplars:cadna-and-verificarlo`), (`ex:exemplars:measurements-jl`).

**Decision (Roles of alternative models)** · `dec:vision:alternative-error-model-roles`

Forward enclosures are the public expression of the primary backward-error approach. Interval techniques provide the validated arithmetic substrate and independent small-problem test oracles; full interval matrices and affine tracking remain optional layers admitted only after demonstrating useful tightness for their cost. Covariance propagation is an optional probabilistic layer with explicit source correlations. Stochastic arithmetic is a diagnostic and testing technique. Neither covariance nor stochastic variation is accepted as evidence that numerical roundoff lies within a deterministic budget, because neither promises that conclusion. Exemplars: (`ex:exemplars:interval-arithmetic-jl`), (`ex:exemplars:cadna-and-verificarlo`), (`ex:exemplars:measurements-jl`).

**Definition (Error companion surface)** · `def:vision:error-companion-surface`

The companion records the target mathematical meaning, deterministic radius when available, assurance kind, coverage assumptions, provenance identifiers, and a ledger of input, representation, arithmetic, and verification contributions. It separately records optional covariance information and modeling or discretization claims. Nonlinear interaction terms receive an explicit interaction entry instead of an invented attribution to independent causes. A total deterministic envelope exists only when its relevant inputs are bounded and its composition is justified. Exemplars: (`ex:exemplars:arb`), (`ex:exemplars:measurements-jl`), (`prob:exemplars:heat-conduction-one-dimensional`).

**Rule (Imports and exactness)** · `rule:vision:imports-and-exactness`

Constructors distinguish exact represented binary values, decimal values requiring conversion accounting, deterministic input enclosures, and probabilistic-only measurements because these describe different admissible inputs. Strict deterministic computation rejects missing required enclosures. Probabilistic-only values remain usable in explicitly selected uncertainty modes, with no claim that their physical values are deterministically enclosed. Constructors reject invalid radii and non-finite centers so later certificates do not inherit undefined arithmetic silently. Exemplars: (`ex:exemplars:measurements-jl`), (`prob:exemplars:pendulum-gravity-estimate`).

**Rule (Arithmetic closure)** · `rule:vision:arithmetic-closure`

Addition propagates both input radii and a validated bound on the discrepancy between the exact operation on represented centers and the stored candidate. Multiplication uses a bound of the form `norm(A) rB + rA norm(B) + rA rB + rounding`, with compatible conservative norms and dimensions. These formulas include the perturbation interaction term because dropping it would turn a claimed enclosure into a first-order approximation. Transposition preserves the Frobenius radius; extraction and reshaping use justified transformations rather than copying an unrelated certificate. Exemplars: (`ex:exemplars:arb`), (`ex:exemplars:higham-accuracy-and-stability`).

**Decision (Independent verification arithmetic)** · `dec:vision:independent-verification-arithmetic`

Certification does not depend on assuming a particular faer summation order. An independent safe arithmetic path encloses residuals and norms through validated endpoint operations, adjacent-representable-value adjustments where appropriate, and overflow-aware scaling. Its contract explicitly includes supported rounding behavior, subnormals, fused operations, and non-finite cases. This path can be substantially slower than a kernel; that cost is accepted for trustworthy initial guarantees and becomes a measured optimization target. Exemplars: (`ex:exemplars:fptaylor-and-gappa`), (`ex:exemplars:interval-arithmetic-jl`), (`cex:exemplars:rump-polynomial`).

**Definition (Certified solve contract)** · `def:vision:certified-solve-contract`

For a square center matrix `A`, center right-hand side `B`, and candidate `X`, let `rho` bound the Frobenius norm of the exact represented-center residual `B − A X`. Let `rA` and `rB` bound their input perturbations, `xnorm` bound the candidate’s Frobenius norm, and `g` be a validated upper bound on the spectral norm of the inverse of `A`. If `g rA < 1`, the output radius is bounded by `g (rho + rB + rA xnorm) / (1 − g rA)`, evaluated conservatively. The denominator expresses the admissible family’s distance from loss of invertibility, which is why the condition is part of the guarantee. Exemplars: (`ex:exemplars:intlab`), (`ex:exemplars:higham-accuracy-and-stability`).

**Decision (Initial inverse-norm certificate)** · `dec:vision:inverse-norm-certificate`

A baseline verifier may construct an approximate inverse `Z` and validate an upper bound `q` on the spectral norm of `I − Z A`, using a Frobenius upper bound where necessary. If `q < 1`, a conservative upper bound on `norm(Z) / (1 − q)` supplies the required inverse-norm bound. This method is chosen for its explicit, independently checkable theorem, not its scalability. Its cubic work and potentially loose Frobenius bounds make tighter verification methods a named early experiment. Exemplars: (`ex:exemplars:intlab`).

**Rule (Decomposition objects)** · `rule:vision:decomposition-objects`

Factorizations return opaque handles carrying permutations, transformations, reconstruction evidence, and applicable orthogonality or positivity evidence because those are relational claims about a decomposition. A reconstruction certificate does not imply small forward error in each factor. Exposing a factor as a general matrix therefore requires its own appropriate error companion and assurance status. Reusing a handle validates that the original center, precision, and relevant policy have not changed, preventing stale certificates. Exemplars: (`ex:exemplars:higham-accuracy-and-stability`), (`ex:exemplars:kahan-and-wilkinson-matrices`).

**Rule (Solves and explicit inverses)** · `rule:vision:solves-and-inverses`

Solving is the preferred interface because forming a complete inverse consumes storage and can produce unnecessary error when only its action is needed. An explicitly requested inverse is treated as a matrix-valued solve against an exact dimensionless identity and receives a matrix envelope. Least-squares and rank-deficient operations need distinct rank and perturbation contracts; a square nonsingular solve certificate does not apply to them merely because a backend returns a candidate. Exemplars: (`ex:exemplars:higham-accuracy-and-stability`), (`ex:exemplars:intlab`).

**Rule (Spectral guarantees)** · `rule:vision:spectral-guarantees`

For supported symmetric admissible inputs, a normalized eigenpair residual can bound distance to some eigenvalue, but identifying a particular eigenvalue or bounding its eigenvector requires additional separation evidence. Repeated or clustered eigenvalues favor invariant-subspace guarantees because individual eigenvectors are not uniquely stable. General nonnormal eigenproblems remain separately gated because small residuals can coexist with large spectral sensitivity. Every spectral report identifies whether it bounds an eigenvalue set, an associated eigenpair, or a subspace. Exemplars: (`ex:exemplars:higham-accuracy-and-stability`), (`prob:exemplars:coupled-oscillators-normal-modes`).

**Rule (Matrix-function guarantees)** · `rule:vision:matrix-function-guarantees`

Each admitted matrix function supplies a domain check, an approximation-error bound, and a perturbation argument because function behavior depends on singularities and branch structure. Examples include separating a logarithm’s admissible spectrum from its excluded domain and bounding truncation in a supported exponential algorithm. A kernel returning a finite matrix is insufficient. Until these obligations are met, checked evaluation returns an unsupported-guarantee failure. Exemplars: (`ex:exemplars:arb`), (`ex:exemplars:higham-accuracy-and-stability`).

**Definition (Operation result and failure)** · `def:vision:operation-result-and-failure`

A numerical operation returns `Result<Outcome<T>, NumericalFailure>`. The outcome contains the value with its companion and a report recording algorithm, precision, transformations, residuals, conditioning evidence, bound, budget status, and recovery attempts. Dimensionally invalid expressions fail at compile time; dynamic shape errors, unsupported guarantees, failed domain checks, non-finite arithmetic, and unmet budgets are runtime failures. Ordinary numerical failure never uses a documented panic, because callers must be able to handle it as part of calculation. Exemplars: (`ex:exemplars:intlab`), (`ex:exemplars:hilbert-matrices`), (`cex:exemplars:rump-polynomial`).

**Rule (Assurance and correlation)** · `rule:vision:assurance-and-correlation`

Certified and estimated results have distinguishable API access, and checked constructors control any certified wrapper, because documentation alone cannot prevent accidental promotion of an estimate. Deterministic balls remain conservative without assuming input independence, although they can overcount shared error. Covariance layers retain common-source identifiers and cross-covariances; they cannot silently treat reused measurements as independent. Reports retain compact provenance references rather than copying complete histories into every matrix, limiting memory growth without deleting assumptions. Exemplars: (`ex:exemplars:cadna-and-verificarlo`), (`ex:exemplars:measurements-jl`), (`prob:exemplars:pendulum-gravity-estimate`).

## Units in the type system, as they meet matrices · `sec:vision:units-and-matrices`

**Decision (Homogeneous numerical leaves)** · `dec:vision:homogeneous-numerical-leaves`

Each dense numerical leaf has one composite dimension for its entire matrix because this maps directly to faer storage and makes matrix multiplication’s dimensional meaning uniform. Heterogeneous systems are represented as typed homogeneous blocks with explicit domain and codomain structure. Arbitrary per-element dimensions are excluded from the initial dense abstraction because they obstruct ordinary kernels and make even addition and multiplication depend on element-level compatibility. Exemplars: (`ex:exemplars:uom-and-dimensioned`), (`ex:exemplars:nalgebra`), (`prob:exemplars:resistor-network`).

**Rule (Heterogeneous assembly)** · `rule:vision:heterogeneous-assembly`

A block operator identifies each variable block’s dimension and each equation block’s dimension, so the block connecting them has the corresponding quotient dimension. Assembly checks this relationship before producing numerical storage because a physically heterogeneous vector cannot acquire a single honest dimension by concatenation. Numerical nondimensionalization records typed scales for every block and reverses them in outputs and error bounds. Exemplars: (`prob:exemplars:spring-mass-truss`), (`prob:exemplars:resistor-network`).

**Decision (Storage and scalar wrapping)** · `dec:vision:storage-and-scalar-wrapping`

A scalar quantity wraps a supported numerical scalar with dimensional and error information, while a matrix stores plain backend scalars and attaches dimensions and error metadata to the matrix owner. Kernels receive safe borrowed views of that storage; mutable access is confined to operations that update or invalidate evidence. This arrangement avoids per-element wrapper conversion in hot kernels without pretending that error tracking itself has zero cost. Layout, allocation, and throughput claims require the matrix-layer experiment. Exemplars: (`ex:exemplars:faer`), (`ex:exemplars:nalgebra`).

**Table (Dimension representation candidates)** · `tab:vision:dimension-representation-candidates`

|Candidate|Advantage|Deciding criterion and remaining cost|
|---------|---------|-------------------------------------|
|Seven const-generic integer exponents|Directly expresses an SI exponent vector and its arithmetic|Admit only if nightly normalization, diagnostics, and trait constraints work for the required operations without unsafe code or excessive annotation.|
|Type-level integers and arithmetic|Can express dimension composition through trait machinery|Compare compiler time, error readability, coherence, and public API complexity on equivalent examples.|
|Closed generated dimension table|Makes supported operations explicit and can simplify diagnostics|Accept only if completeness for intended composite forms and regeneration burden are demonstrated; otherwise its closed world is too restrictive.|

Exemplars: (`ex:exemplars:uom-and-dimensioned`), (`ex:exemplars:nalgebra`).

**Requirement (Units design decision criteria)** · `req:vision:units-design-criteria`

The units design bite chooses the exponent representation after compile-only prototypes demonstrate dimensional equality, normalization of equivalent expressions, multiplication, division, integer powers, matrix solves, and useful rejection messages. Its criteria are soundness, expressiveness, inference, diagnostics, compilation cost, and compatibility with the lint posture, in that order. This architecture fixes those obligations rather than choosing their implementation, because compiler behavior and usability require evidence. Exemplars: (`ex:exemplars:fsharp-units-of-measure`), (`ex:exemplars:uom-and-dimensioned`), (`ex:exemplars:nalgebra`).

**Rule (Canonical quantities and affine coordinates)** · `rule:vision:canonical-quantities-and-affine-coordinates`

Numerical storage uses declared canonical SI scaling, with conversion error included in the companion, because equivalent units must not alter physical meaning or hide representation loss. Offset scales are modeled as affine coordinates at conversion boundaries rather than ordinary multiplicative units; absolute temperatures and temperature differences therefore cannot share every operation indiscriminately. Angle, count, and other dimensionless semantic distinctions remain an explicit units-design question instead of being inferred from the seven exponents alone. Exemplars: (`ex:exemplars:mp-units`), (`prob:exemplars:heat-conduction-one-dimensional`).

**Requirement (Powers and factor dimensions)** · `req:vision:powers-and-factor-dimensions`

The units bite establishes which roots and powers its representation can express and rejects unsupported dimensional results because fractional exponents cannot be smuggled into an integer-exponent system. Matrix adapters document factor dimensions: for example, QR separates a dimensionless orthogonal factor from a factor carrying the original matrix dimension. Cholesky may require a square-root dimension, so its public exposure must respect the selected representation even when its internal numerical computation is available. Exemplars: (`ex:exemplars:mp-units`), (`prob:exemplars:coupled-oscillators-normal-modes`).

## Repository structure · `sec:vision:repository-structure`

**Definition (Baseline and proposed tree)** · `def:vision:baseline-and-proposed-tree`

The starting repository is a virtual nightly Cargo workspace with no members, a strict workspace lint configuration, an orchestration-linter snapshot assigning the root files and documents to one owner, and separate code and documentation/data licenses. The following packages are the target structure. Packages enter the workspace when they contain a coherent implemented responsibility, because empty scaffolding would create maintenance and documentation obligations without a usable boundary. Exemplars: none.

**Table (Packages and dependency direction)** · `tab:vision:package-structure`

|Directory and crate|Sole responsibility|Engine dependencies|
|-------------------|-------------------|-------------------|
|`packages/scalar`, `nrz-scalar`|Supported scalar contracts, conversions, and validated primitive numerical operations|None of the other engine packages|
|`packages/accuracy`, `nrz-accuracy`|Dimension-neutral envelope primitives, assurance, policy, and compact provenance|`nrz-scalar`|
|`packages/units`, `nrz-units`|SI dimension algebra, typed scalar quantities, and scale conversions|`nrz-scalar`, `nrz-accuracy`|
|`packages/matrix`, `nrz-matrix`|Unit-aware matrices, faer adapters, and operation-specific certificates|`nrz-scalar`, `nrz-accuracy`, `nrz-units`|
|`packages/physics`, `nrz-physics`|Physical structures, constraint assembly, and computation graphs using petgraph|`nrz-units`, `nrz-matrix`, `nrz-accuracy`|
|`packages/engine`, `nrz`|Curated application-facing exports and entry points|The public interfaces of the engine packages above|
|`packages/docgen`, `nrz-docgen`|TOML ingestion, deterministic datasets, minijinja rendering, and fixed-point verification|No runtime engine dependency|

Exemplars: (`ex:exemplars:faer`), (`ex:exemplars:herbie`), (`ex:exemplars:estate-tools-and-templated-documentation`).

**Rule (Layer enforcement)** · `rule:vision:layer-enforcement`

Dependencies flow downward through the declared package graph; accuracy primitives receive their dimensional meaning from typed wrappers rather than depending back on units. A metadata-based architecture check compares normal, build, development, and feature-dependent edges against the permitted graph because a forbidden dependency remains a coupling even when hidden in tests. Cross-layer integration tests live at an existing upper layer. The facade reexports supported concepts and adds no competing numerical implementation. Exemplars: none.

**Rule (Backend boundaries)** · `rule:vision:backend-boundaries`

Only the matrix package integrates faer, and only the physics package integrates petgraph, because backend details must not spread into dimensional algebra or the error contract. Backend matrices are borrowed or owned behind engine interfaces that preserve metadata. Public escape hatches clearly return raw centers and consume or detach guarantees where necessary, preventing external mutation from leaving apparently valid evidence attached. Exemplars: (`ex:exemplars:faer`).

**Decision (Physical and computational graphs)** · `dec:vision:physical-and-computational-graphs`

Physical relationships and computation dependencies use distinct graph schemas because a physical cycle is not automatically a scheduling error. Acyclic dependencies can be scheduled directly; coupled constraints are assembled into explicit systems or declared iterative procedures. Cache validity includes input identity, precision, units, policy, and evidence dependencies because reusing a center under changed assumptions would invalidate its companion. Discretization and modeling errors remain separate ledger entries with their own coverage. Exemplars: (`ex:exemplars:drake`), (`ex:exemplars:modelica`).

**Rule (Tests, examples, and worked problems)** · `rule:vision:verification-locations`

Package-local tests exercise the package contract; integration tests live under that package’s `tests/` directory when they cross its public interface. Units include compile-failure and compile-success cases because dimensional rejection is an observable feature. Matrix fixtures live under `packages/matrix/tests/fixtures/`, while executable examples and benchmarks live in the relevant package’s `examples/` and `benches/` directories. Cross-layer worked problems live under `problems/`, each with an identified physical model, inputs, expected observables, and generated explanatory documentation, so validation connects numerical results to a stated problem. Exemplars: (`prob:exemplars:spring-mass-truss`), (`prob:exemplars:resistor-network`).

**Rule (Independent numerical evidence)** · `rule:vision:independent-numerical-evidence`

Tests combine small exact or independently bounded references, adversarial conditioning cases, structural identities, and whole worked problems because no single oracle covers both arithmetic correctness and model assembly. Higher precision is useful reference evidence but is not automatically truth. Fixtures include scale imbalance, cancellation, near-singularity, repeated spectra, correlated inputs, and budget boundaries. Benchmark reports pair speed with achieved assurance and accuracy; a faster failing computation is not recorded as an improvement. Exemplars: (`ex:exemplars:herbie`), (`ex:exemplars:interval-arithmetic-jl`), (`ex:exemplars:hilbert-matrices`), (`ex:exemplars:kahan-and-wilkinson-matrices`), (`cex:exemplars:rump-polynomial`).

**Decision (Documentation layout)** · `dec:vision:documentation-layout`

Each Markdown render has its minijinja template beside it, using a distinguishable suffix such as `.md.j2`. Hand-edited TOML inputs live in the nearest owning documentation area, and generated JSON datasets live in a clearly generated sibling area. Root and cross-cutting documents use a central `docs/` area for their inputs and datasets while retaining templates beside their actual outputs. This arrangement keeps the source of an explanation discoverable without requiring all documentation to move away from the code it explains. Exemplars: (`ex:exemplars:estate-tools-and-templated-documentation`).

**Rule (Documentation envelope and derivation)** · `rule:vision:documentation-envelope-and-derivation`

Each input envelope identifies its document or dataset, labels, ownership reference, license classification, and source relationships because rendering must preserve estate metadata as well as prose. Exact keys follow the orchestration linter’s actual schema once available. Derived JSON is reproducible from committed inputs, metadata, and fixtures; live clocks, network responses, unordered traversal, and live benchmark timing are excluded because they would prevent reproducible renders. Recorded measurements include their environment and provenance before entering documentation. Exemplars: (`ex:exemplars:estate-tools-and-templated-documentation`).

**Rule (Fixed-point gate and bootstrap)** · `rule:vision:fixed-point-gate-and-bootstrap`

The documentation gate regenerates datasets and renders in an isolated output area, compares bytes and output inventory with the committed state, and verifies that another generation produces no change. This detects drift, stale outputs, and self-feeding transformations. The generator’s first usable commit includes its own documentation inputs, templates, and renders, together with the converted architecture document; the bootstrap therefore establishes the same rule that subsequent commits must obey. Exemplars: (`ex:exemplars:estate-tools-and-templated-documentation`).

**Decision (Labels and ownership)** · `dec:vision:labels-and-ownership`

Root policy remains with the declared root owner until an explicit partition update is committed. The proposed partition assigns each package subtree and each cross-cutting documentation or worked-problem subtree exactly one declared owner responsibility, with an actual estate owner resolved through the linter configuration. Ownership of templates, inputs, datasets, and renders follows their subtree responsibility. Labels remain stable across moves and use unique semantic identifiers because citations should survive changes in file layout. Exemplars: (`ex:exemplars:estate-tools-and-templated-documentation`).

**Rule (Metadata and licenses)** · `rule:vision:metadata-and-licenses`

Every document and source carries the required labels and passes the orchestration linter because ownership and traceability apply to generated and handwritten material alike. Program code is AGPL-3.0-only; data and documentation are CC0-1.0. Generator logic, narrative content, fixtures, and outputs receive explicit classifications so a generated file does not accidentally inherit a tooling file’s metadata. Third-party material retains its applicable attribution and is not relabeled by generation. Exemplars: (`ex:exemplars:estate-tools-and-templated-documentation`).

**Rule (Gate of record)** · `rule:vision:gate-of-record`

A green commit passes the pinned nightly formatter, the declared rustc and clippy posture, documentation checks, appropriate unit and integration tests, dimensional compile checks, orchestration linting, architecture checks, and documentation fixed-point verification. Relevant supported feature configurations are checked because inactive features can otherwise hide violations. Unsafe code remains forbidden; missing documentation, missing Debug implementations, unreachable public items, unwrap, unfinished placeholders, debugging macros, and print macros remain denied as specified. Routine tooling output uses an appropriate fallible I/O interface instead of weakening these rules. Exemplars: (`ex:exemplars:estate-tools-and-templated-documentation`).

**Rule (Meaningful commit series)** · `rule:vision:meaningful-commit-series`

Each main-branch commit is one independently reviewable, green unit with a Conventional Commits subject and a body explaining why. A typical bite first establishes a usable contract and its rendered documentation, then adds a minimal supported implementation with its meaningful tests, then adds separately justified capabilities or optimizations. Tests accompany the behavior they validate; generated artifacts accompany the source change that causes them. An indivisible cross-package contract change stays together because artificial splitting would leave intermediate commits incoherent. Exemplars: none.

## Risks and open questions · `sec:vision:risks-and-open-questions`

**Caveat (Backend capability uncertainty)** · `cav:vision:backend-capability-uncertainty`

No particular faer version, decomposition API, threading behavior, or extended-scalar capability is established here. The numerical feasibility bite audits a selected version’s safe interfaces and exercises the required candidate operations. Acceptance requires safe integration and correct residual verification; an unavailable advanced operation remains unsupported until a compatible implementation path exists.

**Caveat (Validated arithmetic assumptions)** · `cav:vision:validated-arithmetic-assumptions`

Certified envelopes depend on correctly handling primitive rounding, subnormals, overflow, and norms. The feasibility bite tests a small validated arithmetic implementation against exact small-case references and targeted floating-point boundaries on supported environments. Failure narrows the supported platform contract or blocks certification; it does not downgrade a guarantee invisibly.

**Caveat (Certificate cost and looseness)** · `cav:vision:certificate-cost-and-looseness`

Frobenius envelopes and approximate-inverse verification can be too conservative or expensive for large systems. The first matrix bite measures accepted-budget rates, certificate width, runtime, and memory over scaled and ill-conditioned problems. Componentwise bounds, tighter inverse-norm verification, or structured uncertainty become candidates only if they materially improve these measurements while preserving a checkable argument.

**Caveat (Transitive unsafe code)** · `cav:vision:transitive-unsafe-code`

Workspace-wide prohibition establishes a rule for workspace-authored code; it does not establish that third-party implementation internals contain no unsafe code. The feasibility audit records the dependency trust boundary and verifies that engine integration needs only safe APIs. A requirement to exclude unsafe transitively would need separate examination against the fixed faer backend and could require an explicit constraint amendment rather than an unsupported assurance.

**Caveat (Lint feasibility)** · `cav:vision:lint-feasibility`

The declared lint posture is stricter than ordinary numerical projects and may expose conflicts in tests, generated interfaces, or primitive arithmetic. The generator and feasibility bites compile minimal representative implementations under the actual configuration. Any genuine incompatibility is reported with the smallest necessary argued change; this architecture grants no blanket allowances or exemption for test code.

**Open Question (Dimension encoding and semantic kinds)** · `open:vision:dimension-encoding-and-semantic-kinds`

The exponent representation, treatment of roots, and distinctions among dimensionless semantic kinds remain for the units design bite. Equivalent compile-only examples compare diagnostics, inference, compilation cost, and expressible physics. The decision must cover block assembly and factor dimensions as well as elementary scalar arithmetic, because success on a multiplication example alone is insufficient.

**Caveat (Correlated and nonlinear uncertainty)** · `cav:vision:correlated-and-nonlinear-uncertainty`

Covariance linearization can understate uncertainty near nonlinear transitions, and deterministic balls can greatly overstate it when dependencies repeat. The uncertainty extension compares shared-source examples, affine or interval references, and sampling diagnostics without treating sampling as proof. Admission requires a visible validity domain and useful improvement over the primary companion.

**Caveat (Spectral and function coverage)** · `cav:vision:spectral-and-function-coverage`

Repeated eigenvalues, nonnormality, rank changes, and function-domain boundaries defeat generic error recipes. A separate advanced matrix stage begins with symmetric spectral examples and explicit separation tests, then evaluates each additional operation against an operation-specific oracle. Failure to establish a domain or certificate keeps that capability outside checked evaluation.

**Open Question (Physical model coverage)** · `open:vision:physical-model-coverage`

The first representative bodies, fields, constraints, and integration procedures are not specified. The physics-design portion selects worked problems that exercise homogeneous and heterogeneous assembly, coupled constraints, and measurable numerical sensitivity. Each problem states whether discretization and modeling error are bounded, estimated, or outside its guarantee, so numerical certification cannot be mistaken for complete physical validation.

**Caveat (Estate schema and reproducibility)** · `cav:vision:estate-schema-and-reproducibility`

Exact envelope fields, source-label placement, and additional owner identities are unavailable at this architectural boundary. The documentation generator bite resolves them against the declared linter snapshot and proves the output fixed point. Numerical reproducibility is evaluated separately across execution settings: deterministic documentation is mandatory, while numerical reports must identify settings that can affect centers, certificates, and diagnostic experiments.

## Mapping onto the plan · `sec:vision:mapping-onto-the-plan`

**Decision (Sequencing adjustment)** · `dec:vision:sequencing-adjustment`

Documentation generation remains first because every later architectural decision needs a reproducible repository representation. A numerical feasibility stage follows before the units design is finalized, because scalar contracts, certification costs, and backend boundaries constrain a useful units interface. The matrix bite is split into basic certified operations and advanced numerical domains so the engine can establish a trustworthy core without implying that every decomposition or matrix function shares one proof.

**Table (Bites and architectural obligations)** · `tab:vision:bite-mapping`

|Sequence|Relationship to the original plan|Architectural work and completion evidence|
|--------|---------------------------------|------------------------------------------|
|1. Documentation generator|Original bite 1|Establish `packages/docgen`, `nrz-docgen`, envelope handling, deterministic datasets, templates beside outputs, ownership integration, and fixed-point checks; import this architecture as a committed render.|
|2. Numerical feasibility|Added prerequisite|Validate scalar and arithmetic contracts, safe faer integration, representative lint compatibility, and baseline residual/inverse-norm certification; record measured limitations before committing to broad APIs.|
|3. Units design|Original bite 2|Decide exponent representation, semantic kinds, conversions, roots, block dimensions, and diagnostics using the criteria in the units section; render the decision and rejected alternatives.|
|4. Units and numerical foundations|Original bite 3 with necessary foundations|Implement supported scalar primitives, error-companion foundations, and units contracts with dimensional acceptance/rejection tests and independent arithmetic checks; keep package dependencies acyclic.|
|5. Certified matrix core|First part of original bite 4|Establish backend-native storage, sidecars, arithmetic closure, factor handles, and square solve guarantees; demonstrate budget failures and certificate cost on representative fixtures.|
|6. Initial physics core|Original bite 5, scoped to proven matrix capabilities|Implement petgraph structures, typed block assembly, dependency evaluation, cache validity, and selected worked problems; expose model and numerical error separately.|
|7. Advanced matrix domains|Second part of original bite 4|Admit least-squares/rank-sensitive guarantees, symmetric spectral operations, and individual matrix functions as their proofs and experiments succeed; physics capabilities expand only when dependencies are ready.|
|8. Accuracy and benchmarks|Original bite 6, retained as a consolidation stage|Evaluate complete problems, conditioning coverage, mixed precision, uncertainty extensions, certificate tightness, time, and memory; publish reproducible recorded results and capability limits.|

**Rule (Verification is continuous)** · `rule:vision:verification-is-continuous`

Accuracy testing and representative measurements accompany every numerical bite because the final benchmark stage cannot repair an architecture built on untested guarantee assumptions. The consolidation stage broadens coverage and compares alternatives; it is not the first point at which accuracy is examined. Algorithm changes regenerate affected reports and fixtures in the same meaningful commit when their recorded conclusions change.

**Rule (Completion boundaries)** · `rule:vision:completion-boundaries`

Each bite declares its supported domain, assurance level, error behavior, and outstanding exclusions in generated documentation because completion must be reviewable without assuming future work. A bite that cannot establish its intended guarantee either narrows its explicitly supported domain or remains incomplete. The engine’s growth is therefore measured by verified capabilities and usable physical problems, rather than by the number of backend operations exposed.
