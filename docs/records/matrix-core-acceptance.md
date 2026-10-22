<!-- SPDX-License-Identifier: CC0-1.0 -->

# Integrated matrix acceptance record · `guide:nrz:matrix-core-acceptance`

## Scope and reproducible inputs · `sec:nrz:matrix-acceptance-scope`

This record covers lane I of the reviewed matrix-core design (`tab:nrz:matrix-core-lane-plan`). It adds public acceptance and measurement evidence, with no production numerical changes or dependencies. The tests and `acceptance_measurements` example share committed inputs under `crates/matrix/tests/integration/acceptance/support/`. The server supplies the toolchain gate and measurements; documentation generation reads committed text and never runs the example (`rule:nrz:matrix-core-measurements`).

The static truss uses the three-member reference problem's reduced stiffness `[[2001/2,1/2],[1/2,3/2]] N/m` and load `(1,1/1000) N`. Scalar literal decoding supplies the inexact load import's deterministic enclosure. The physical displacement is `(2999/3001000,1001/3001000) m`; the two orthogonal member forces are `(2999/3001,1001/3001000) N`. A separate illustrative finite-moment scenario gives both represented load means one common fluctuation of variance `1/16 N²`, with an exactly fixed operator. That moment declaration is not a physical bounded-support claim. The exact derivative uses the represented nominal RHS, and the deterministic total additionally encloses the rational imported load (`sec:nrz:matrix-core-oracles`).

The ladder uses gains 1, 2 and 3. The original five-by-five normalized system has the exact response `(1,5/13,2/13,1/13,-8/13) E`, checked by independent rational elimination. Its statistical solve currently refuses in symbolic sensitivity admission. The successful formulation exactly eliminates `V1=E` and `I=V2-E`, solving `[[2,-1,0],[-1,3,-1],[0,-1,3]] (V4,V3,V2) = (0,0,E)`. The fixed exact map `(3V2−V3,V2,V3,V4,V3−2V2)` reconstructs all five original entries before physical restoration. This removes redundant circuit coordinates without changing the physical problem or increasing any limit. The original source variance `1/10000` is enclosed by adjacent binary64 endpoints in a dense PSD declaration; it is not replaced by an exactly represented binary approximation. Each solve preserves the shared source. Separate homogeneous voltage and current maps restore physical outputs. Nine five-by-five cross blocks cover the complete fifteen-entry joint covariance. An additional four-by-one physical voltage/current cross block includes `−3/42250 V A`. The ordered mean has covariance `4 U0`, and the oracle rejects the independence-based `14/9 U0` (`ex:nrz:resistor-source-batch`).

The resistor requests an absolute `1e-9` Frobenius solve budget on the three dimensionless nodal coordinates at each gain. Reconstructed physical maps retain propagated total radii; they do not acquire a new solve-budget decision. The truss requests a relative coefficient `1e-6` with characteristic displacement `0.001 m`, giving a `1e-9 m` threshold on the displacement solve.

The exact tests independently decode binary sign, exponent and significand into integer arithmetic, use rational elimination for solutions and derivatives, and compare returned center/radius pairs with exact integer inequalities. Production enclosure arithmetic, CAS equality and a second floating solve do not supply the acceptance oracle. Prior public exact-oracle helpers are reused without changing their implementation (`dec:nrz:matrix-core-test-layers`).

## Server environment and commands · `sec:nrz:matrix-acceptance-environment`

The server observation supplied on 2026-10-16 names machine `straka`: `rustc 1.101.0-nightly (21b707e3f 2026-09-30)`, Linux `6.8.0-111-generic`, `x86_64`, AMD EPYC 7C13 64-Core with 256 logical processors, and `1,056,595,364 kB` reported memory. Load was about 17. The binary used `NRZ_MEASUREMENT_ENV=straka`; faer is locked to `0.24.4`. The recorded numbers were measured under the former release profile and must be distinguished from new measurements using the workspace dev profile (`opt-level = 2`). These are server-reported facts, not locally observed hardware.

The first measurement batch used source commit `f361e9c`, with a reported standalone build wall time of 94 s under the former release profile. Its verification build separately took 129.9 s and peaked at 1,071,160 kB. The accepted rerun used commit `475fbd8`, with load averages `26.99 18.67 18.25` and a 92 s measurement build. Its separate verification build under the former release profile took 130.3 s and peaked at 1,074,908 kB.

The public operation records retain IEEE binary32/binary64, nearest ties-to-even, gradual underflow and outward arithmetic revision 1. This corpus executes binary64. Admission of other targets or compiler profiles is not inferred from these runs (`rule:nrz:matrix-core-rounding`). The complete compiler/backend/machine identification above belongs to the recorded server environment; the current operation record does not automatically capture that build identity.

Build once with the workspace dev profile (`opt-level = 2`) using `cargo build -p nrz-matrix --locked --example acceptance_measurements`. Run `target/debug/examples/acceptance_measurements CASE` separately for each of `truss`, `resistor`, `certificate-edge`, `shared-a-b`, `stale-factor` and `bounded-refinement`. The server wraps each process in `/usr/bin/time -v` to capture peak resident memory. The example uses `std::time::Instant` for stage observations and fallible standard output. Tests never print measurements or assert wall-time thresholds.

## Recorded costs and bounds · `sec:nrz:matrix-acceptance-costs`

The following first-batch observations come from the server's measurement table for `f361e9c`. Process wall and peak RSS come from `/usr/bin/time -v`; internal runtime and work counts come from `acceptance_measurements`. A displayed wall of `0.00 s` is timer resolution, not zero execution time. The full-formulation resistor failure is retained separately from the accepted reduced-formulation rerun.

|Case in first batch|Public result or refusal|Process wall (s)|Peak RSS (kB)|
|-------------------|------------------------|---------------:|------------:|
|Truss|2 published, 0 refused; affine moments; solve work 1,536; statistical work 14,272 and 1,472; internal total 0.0538 s|0.05|8,196|
|Full five-coordinate resistor|Exit 1, symbolic draft Work requires 2,003,865 against 2,000,000; no successful result|0.02|10,252|
|Certificate edge|1 published, 1 refused; `CannotCertifyInverse`, factorization, attempt work 1,536; inverse witness magnitude 2,097,152.5|0.00|4,096|
|Shared A and b|1 published, 0 refused; linearized moments; solve work 384 and statistical work 2,584|0.02|6,152|
|Stale factor|1 published, 1 refused; `ForeignFactor`, factorization, attempt work 1,536|0.00|4,108|
|Bounded refinement|0 published, 1 refused; `Stagnation`, evaluation, attempt work 256; requested budgets accepted 0/1|0.00|4,096|

The accepted rerun used `475fbd8104c7ef5cb8368e28c22353f5d7525f1d`. The following tables transcribe values from its complete server stdout. Region counts, payload bytes, work, decisions, widths and internal clocks come from public fixture observations. Process wall and peak RSS come from `/usr/bin/time -v`. These are single observations under load; no runtime threshold or speedup claim follows.

|Case|Published / refused|Regions / descriptors|Payload scalars|Payload bytes|Requested budgets met|
|----|------------------:|--------------------:|--------------:|------------:|--------------------:|
|Truss|2 / 0|30|141|1128|1/1|
|Reduced resistor|18 / 0|224|3705|29640|3/3|
|Certificate edge|1 / 1|7|18|144|0/0|
|Shared A and b|1 / 0|15|48|384|0/0|
|Stale factor|1 / 1|5|12|96|0/0|
|Bounded refinement|0 / 1|2|2|16|0/1|

`0/0` means no budget was requested. Each case successfully constructed one factor. The certificate-edge case also made one unsuccessful factor attempt. The resistor reused its one factor across the second and third solve while recomputing each certificate. The stale-factor observation is captured before operator replacement and retained as historical evidence afterward.

|Case|Admitted dimensions and mode|Internal total (s)|Process wall (s)|Peak RSS (kB)|
|----|----------------------------|-----------------:|---------------:|------------:|
|Truss|2×2 operator, one RHS; displacement m, forces N; affine moments|0.061112976|0.06|8,204|
|Reduced resistor|3×3 operator, one RHS at each gain; five reconstructed entries, 4 V and 1 A; affine moments|0.759475622|0.77|32,792|
|Certificate edge|Two 2×2 represented-input systems; no statistical claim|0.001941749|0.00|4,100|
|Shared A and b|1×1 operator, two RHS columns; linearized moments|0.019023896|0.02|6,152|
|Stale factor|2×2 represented-input system before/after replacement; no statistical claim|0.001602691|0.00|4,100|
|Bounded refinement|1×1 represented-input system; valid failed-budget proof, no output|0.001651231|0.00|4,112|

Public provenance accounting is ordered `(records, edges, depth, bytes, traversal work)`: truss `(4,8,2,3154,9)`, resistor `(36,76,6,26978,69)`, certificate edge and stale factor `(2,4,1,1841,6)`, shared A and b `(2,4,1,1817,6)`, and bounded refinement `(0,0,0,0,0)`.

|Timed phase|Truss (s)|Reduced resistor (s)|
|-----------|--------:|-------------------:|
|Input and shape|0.001745701|0.002788135|
|Factor and inverse witness|0.000031640|0.000033600|
|Solve, certificate, moments and publication|0.000307938|0.000381338; 0.000422998; 0.000448408 in gain order|
|Physical member-force map|0.000816616|Included in total|
|Ten cross blocks|Not applicable|0.003360412|
|Ordered mean|Not applicable|0.010257763|

The adversarial fixtures expose one total clock each; their factor/certificate allowance remains available in the proof counters below.

|Output phase|Deterministic work allowance|Statistical work allowance|Solve derivative columns|Output payload scalars|
|------------|---------------------------:|-------------------------:|-----------------------:|---------------------:|
|Truss displacement|1536|14272|6|66|
|Truss member forces|74|1472|Not a solve|50|
|Resistor RHS gain map, each|147|1056|Not a solve|69|
|Reduced resistor solve, each|4608|74424|12|177|
|Five-entry reconstruction, each|243|11752|Not a solve|225|
|Voltage / current block, each|293 / 77|10112 / 3080|Not a solve|188 / 41|
|First / second ordered sum|40 / 40|15672 / 15672|Not a solve|295 / 295|
|Ordered mean|365|12872|Not a solve|245|
|Certificate-edge accepted / refused attempt|1536 / 1536|0|0|6 accepted|
|Shared A and b|384|2584|3|38|
|Stale accepted / refused attempt|1536 / 1536|0|0|6 historical|
|Bounded refinement|256 per attempt; 512 across two attempts|0|0|No published output|

All primary published phases have eleven regions: value, total, evaluation, and four parameter center/error pairs. The shared nonlinear solve has nine because it does not claim output means; nonstatistical solves have three. The reduced solve retains covariance shape 3×3 and two derivative parameter shapes 3×12. Reconstruction carries a 5×12 source sensitivity and a 5×3 local Jacobian.

|Proof|Inverse defect q|Inverse norm bound g|Residual norm|Total output radius|Maximum differential certificate radius|
|-----|---------------:|-------------------:|------------:|------------------:|--------------------------------------:|
|Truss|6.377746085211916e-16|0.6667786566557157|2.220450284411013e-16|1.4863322407552828e-16|3.1624477146797915e-16|
|Reduced resistor gain 1|9.519191346087106e-16|0.9514859136040775|4.750989259956125e-16|4.520499356532517e-16|1.1461530628741367e-15|
|Reduced resistor gain 2|9.519191346087106e-16|0.9514859136040775|9.50197851991225e-16|9.040998713065034e-16|2.2923061257482735e-15|
|Reduced resistor gain 3|9.519191346087106e-16|0.9514859136040775|1.4054300946555656e-15|1.3372469376200167e-15|2.5407636915019147e-15|
|Certificate edge|3.2927241100071817e-10|2097152.500690716|0.0|0.0|0.0|
|Shared A and b|2.220446049250315e-16|0.5000000000000007|8.88178419700126e-16|4.440892098500639e-16|5.900916318210367e-16|
|Stale factor|6.401858262767183e-16|0.7745966692414845|2.5121479338940426e-15|1.9459014222362038e-15|0.0|
|Bounded refinement|2.220446049250315e-16|0.33333333333333365|3.330669073875472e-16|1.1102230246251593e-16|0.0|

Every successful family has `r_A=0` and positive family margin. The truss additionally carries RHS radius `8.673617379884043e-19 N`; all other shown proofs have `r_B=0`. Refusal before an inverse certificate retains no invented radius. Bounded refinement retains radii `7.401486830834394e-17` and `1.1102230246251593e-16`, both exceeding its zero budget, with stop `Stagnation`, two reserved attempts and work 512.

|Published output|Maximum component width (twice its total radius)|
|----------------|-----------------------------------------------:|
|displacement|2.97266448151056564e-16 m|
|orthogonal-member-forces|2.97488492755981709e-13 N|
|rhs-gain-1|0.00000000000000000e0 (normalized)|
|reduced-solution-gain-1|9.04099871306503407e-16 (normalized)|
|solution-gain-1|4.28253330000110913e-15 (normalized)|
|voltage-gain-1|4.28253330000110913e-15 V|
|current-gain-1|3.15638882376957402e-15 A|
|rhs-gain-2|0.00000000000000000e0 (normalized)|
|reduced-solution-gain-2|1.80819974261300681e-15 (normalized)|
|solution-gain-2|8.56506660000221826e-15 (normalized)|
|voltage-gain-2|8.56506660000221826e-15 V|
|current-gain-2|6.31277764753914804e-15 A|
|rhs-gain-3|0.00000000000000000e0 (normalized)|
|reduced-solution-gain-3|2.67449387524003331e-15 (normalized)|
|solution-gain-3|1.24743323403603884e-14 (normalized)|
|voltage-gain-3|1.24743323403603884e-14 V|
|current-gain-3|9.35574925527029210e-15 A|
|sum-1-2|1.37357783197034558e-14 (normalized)|
|sum-1-2-3|2.79864674994641025e-14 (normalized)|
|mean|1.15492685490716853e-14 (normalized)|
|near-edge|0.00000000000000000e0 (normalized)|
|shared-operator-rhs|8.88178419700127796e-16 (normalized)|
|before-operator-replacement|3.89180284447240759e-15 (normalized)|

Parameter evaluation errors remain separate from physical total widths. For example, maximum covariance-parameter radii were `6.24500451351650677e-17` for truss displacement, `9.32413868337533967e-18` for gain-three reconstruction and `5.09575021068187161e-18` for the ordered mean. The corresponding derivative-parameter radii are inspected and covered by the exact rational tests; the solve differential certificate maximum is listed separately above.

The truss displacement centers reported by the server were `0.000999333555481506` and `0.0003335554815061644`, with component total radii about `1.486e-16 m`; member-force centers were `0.9993335554815062` and `0.0003335554815061644 N`. Its first gate exposed an oracle-scope error in the member-force numerical-only check, corrected by decoding the actual represented displacement operand. A composed map's new evaluation radius covers its local represented-input problem, while its total carries the upstream physical enclosure. The physical target was retained unchanged.

Region and descriptor counts enumerate actual addressable imported and published regions through public views. They include centers, total and evaluation radii, original source covariance/factor regions, declared-mean error regions, and each propagated covariance, sensitivity, Jacobian and mean center/error pair. Statistical record objects, shared descriptor declarations, private factors, scratch, faer padding and allocator overhead are not represented as payload bytes. Public provenance bytes are the crate's separate accounted metadata, not measured allocator memory. Peak RSS includes the complete single-case process and its setup.

A solve's work number is its conservative admission allowance, covering factorization, candidate solves, independent contractions, norms and scratch. It is not a hardware instruction or flop count. Statistical work additionally charges the nominal enclosure, every logical derivative column, composed sensitivities, covariance and parameter storage. The factor phase time includes its independent inverse witness; the solve phase time includes certification, sensitivities, moments and publication. The API does not expose separate backend allocation sizes or an actual per-primitive operation counter. The internal total clock spans fixture setup through observation capture; it excludes return-time destruction and output printing. Binding validation and observation collection are included in that total but are not part of the explicitly timed factor or solve phases. External process wall time includes the complete execution.

For `N` logical output entries, a complete covariance has `N²` centers and another `N²` evaluation-error entries. The five-entry ladder output uses 25 covariance centers; all three points together require 225. Materializing all nine cross blocks therefore stores 450 scalars including their errors. The six-coordinate truss source factor uses one column plus one weight, while the ladder's twelve-coordinate reduced dense source model owns two 144-entry regions. These are different models and dimensions, so the record makes no performance or memory-savings comparison between factor and dense representations.

## Dependency and transcript evidence · `sec:nrz:matrix-acceptance-transcripts`

The dependency harness is Rust within the public conformance test. It invokes offline locked `cargo tree` for default, no-default and all-feature configurations, checking each package's direct normal, build and development edges separately with `--target all`, `--prefix none` and `--depth 1`. Resolved package names precede their versions; paths and display annotations are not dependency identities. It also checks the workspace owner inventory at depth zero. The current workspace manifests admit no feature switches, optional edges or target-specific declarations; a narrow declaration policy rejects their introduction, while Cargo remains responsible for parsing and resolving manifests. In-memory negative twins inject unsupported features, optional/target declarations and forbidden normal/build/development targets with misleading display paths (`tab:nrz:matrix-core-dependency-graph`).

No lock digest is asserted. Cargo's locked resolution and the permission table check the architecture without coupling this matrix test to legitimate package-version refreshes in other lanes. The measured lock state is the unchanged `Cargo.lock` of source commit `475fbd8`, including faer `0.24.4`. No second runtime or new dependency is needed by the test.

Repeated runs compare an evidence-only inspection transcript byte for byte. It encodes stable case/output names, arithmetic assumptions, target/profile, precision, quantity and parameter dimensions, exact floating bits, complete numerical companions, solve theorem stages, derivative certificates, factor reuse relationships, recovery order, budget decisions and public work/storage counts. Runtime samples are separate observations. Every case is repeated on fresh heaps; an additional solve repeats within one heap and across another heap (`conv:nrz:batch-order-and-replay`).

This transcript is a test artifact, not a production serialization API. Raw `OperationRecord` objects retain fresh heap scopes, value IDs, execution IDs and candidate tokens, so their complete Debug bytes are deliberately unequal after re-execution. The tests assert those distinct identities as well as equal numerical transcripts. The owner confirmed that fresh raw identities are the intended behavior and clarified this lane to require transcript determinism. Machine and build identity belong in the server measurement record. No normalization or persistence behavior is added here.

## Refusals and findings · `sec:nrz:matrix-acceptance-findings`

The certificate-edge pair has the exact nonsingular matrices `[[1,1],[1,1+2^-20]]` and `[[1,1],[1,1+2^-52]]`, both with RHS `(1,1)` and exact solution `(1,0)`. The latter's insufficient inverse witness refuses without claiming singularity. A shared-source scalar operator and two RHS columns check the negative cross terms in the implicit derivative. An operator replacement refuses the earlier factor and makes its old result historical. A zero budget for `1/3` ends bounded refinement with a retained nonzero certificate and no output handle (`rule:nrz:matrix-core-refusal-tests`).

The full mixed voltage/current resistor shape and its normalization maps check, but direct `OperationDraft::solve` admission returns `OperationError::UnsupportedShape`: `RegionContract` requires homogeneous numerical storage. The accepted lifecycle uses explicitly normalized coordinates and separate physical output regions. This is an observed boundary, not a claim that the heterogeneous heap portion of the design is implemented. The owner accepted the second finding as a correction to the original dispatch: deterministic transcripts replace its request for identical whole records. The full normalized five-by-five statistical solve additionally refuses when dense symbolic draft validation asks for 2,003,865 Work units against 2,000,000. The matrix sensitivity path creates its own default bindings context and default override, so neither the caller's shape owner nor its solve/execution policy reaches this budget. It constructs five equations and all 175 polynomial partials, including structural zeros, before numerical derivative work. The refusal test retains the nested public sensitivity/algebra cause, its location and exact budget diagnostic. The owner accepted the exact three-node reduction and reconstruction as the successful lifecycle, with this full-system boundary retained as an engine finding. No limit is inflated and no production patch is included.

The server accepted first commit `475fbd8` with 1,159 tests passing and none failing (225.3 s, repeated at 222.0 s), clean strict clippy (28.6 s), clean host lint (5.3 s), docgen and a build under the former release profile. Its complete verification wall was 763.6 s. The dependency/transcript harness and this recorded document are delivered in the second commit; the final per-commit server gate result is preserved in the lane reply of record rather than a self-referential commit hash in this document. No local Rust toolchain was run.
