<!-- SPDX-License-Identifier: CC0-1.0 -->

# nrz-matrix · `guide:matrix:overview`

## Purpose · `sec:matrix:purpose`

`nrz-matrix` owns numerical regions, float kernels and their error evidence. Its role in the two-graph engine is the computational graph: region versions and the checked kernel applications that produced them. The separate descriptor graph states exact meanings and relations, and bindings joins it to numerical representations. The architectural rule is that the heap knows only floats: a region has no inherent unit, kind, sort or physical meaning. Descriptor-side records retain meanings beside numerical storage; the supported binding coverage is explicit below.

## Place in the engine · `sec:matrix:engine`

Matrix depends on bindings, scalar and accuracy. Bindings depends on CAS, so descriptor authority flows matrix → bindings → CAS with no reverse numerical dependency. Bindings checks original meanings and exact map definitions; scalar encloses exported constants and primitive arithmetic; accuracy composes nonnegative bounds and finite budget policies. Matrix owns precision-separated binary32/binary64 faer arenas, safe views, numerical verification, companion regions, lifetime checks and atomic publication.

A kernel is a meaning-free float procedure with an exact array law and an enclosure theorem. Descriptor-side admission retains the original `RegionContract` witnesses beside execution; `KernelContract` contains numerical extents, its closed law and work allowance. Fixed affine schedules retain numerical coefficient definitions, extents and zero obligations without physical descriptors or axes. Matrix can use a fast backend candidate because an independent outward path checks its error against the law. No heap coordinate enters CAS arithmetic: bindings exports an exact constant spelling, scalar encloses it once per compatible map/format/arithmetic revision, and matrix retains that enclosure and import error.

`docs/design/descriptor-binding.md` specifies this ownership. Homogeneous import/copy semantic admission is implemented in `crates/bindings/src/admission/`, and the kernel boundary is separated in `src/binding/kernel/`. Numerical `Schema` records contain only extents. Physical interpretations are interned separately under `src/binding/schema.rs`, and import admission lives under `src/binding/imports.rs`. `RegionView` exposes numerical entries and selection identity; homogeneous meanings are reached through `ValueBinding::symbol` and its companion symbols. Some shape/source meanings still reside in matrix facade records.

## In the LC target · `sec:matrix:lc-target`

For `L=25 mH`, `C=10 µF`, the physical state `(v,i)` has operator `A=[[0,1/C],[-1/L,0]]`. With references `S=diag(5 V,1/10 A)` and `t_ref=1/2000 s`, its checked similarity is `B=t_ref*S⁻¹*A*S=[[0,1],[-1,0]]`. The later restricted spectral kernel will enclose both roots `±i`, restore rates `±2000i s⁻¹`, and let bindings retain the relation to `ω₀=2000 rad/s` and `f₀=1000/π Hz`. Arbitrary row/column scaling would not preserve that spectrum.

For phasors, the binding `R(a+ib)=[[a,-b],[b,a]]` permits ordinary float block products and solves after the algebraic correspondence is checked. At the reference frequencies `1000`, `2000` and `4000 rad/s`, impedance is `−75i`, `0` and `+75i ohm`. Zero impedance is a valid result; admittance/current at resonance refuses and phase is undefined. Neither a solve residual nor a zero center certifies the missing spectral, branch or whole-family premises.

Existing homogeneous kernels, solves, maps and source propagation provide ingredients, and `src/binding/` supplies symbol-based import/copy. The mixed LC lifecycle is still later work: the descriptor-graph step and the region-bindings step establish graph-backed representations and general versioned bindings; the coordinate-lowering step adds mixed restoration; the symbolic-lowering step supplies exact LC evidence and cumulative symbolic policy; the phasor-lowering step and the spectral-lowering step implement phasor and spectral recipes; the numerical-acceptance step integrates acceptance. The LC-quantities step adds certified π/phase evaluation and quantities, the LC-plans step reusable plans, and the LC-reproducibility step within-error reruns with environment provenance. The route is in `docs/design/lc-resonance.md`.

## Main concepts and types · `sec:matrix:concepts`

`Heap` owns storage under `HeapLimits`. An `Extent` and checked `Layout` map logical column order to a region; opaque `Handle<Category, P>` values carry owner/generation information. `ValueId` and `ValueVersion` distinguish a mathematical value occupation from its current contents. Read-only `RegionView` borrows prevent mutation while live. Raw exports copy coordinates and carry no evidence or source-independence claim.

```rust
use nrz_matrix::{Extent, Heap, HeapLimits};
use nrz_bindings::Descriptor;

let mut heap = Heap::new(HeapLimits::default())?;
let imported = heap.import_declared_bounds(
    Extent::new(2, 1)?,
    &Descriptor::ratio(),
    &[2.0_f64, 3.0],
    &[0.125, 0.25],
)?;
let value = heap.value(imported.value())?;
let copy = value.region().export()?;
assert_eq!(copy.len(), 2);
# Ok::<(), nrz_matrix::HeapError>(())
```

These convenience imports prepare exact value/companion symbols before storage allocation. `Heap::import_symbols` instead accepts a predeclared bindings inventory. `ValueBinding` captures a symbol and selected versioned tickets; `value_by_symbol` rechecks that exact symbol and live selection. Copy outputs have planned symbols before evaluation, with `SymbolState::Pending`, `Published` or `Unavailable` distinguishing access states. `Bound::predeclare_result_readings` admits complete initial result families beside a checked recipe before evaluation. A solve vector and its proved components publish together after one numerical write bundle; abandonment or failed publication makes every planned reading unavailable. Result family attachment is an initial declaration, never an equal-schema reinterpretation of a live input.

`ShapeDraft` validates ordered axes, entry meanings and structural scope into `CheckedShape`. `OperationDraft` then checks a closed recipe. The numerical lifecycle is `Binding` → exclusively borrowed `Bound` → private `Candidate` → consuming publication of an `Outcome` containing all `PublishedBundle` outputs. A checked shape alone proves no invertibility or numerical accuracy. `RecipeEvidence`, `OperationRecord` and provenance retain the actual dependencies and numerical claim.

`ConversionContract` retains bindings-checked maps and their lowered constants. `SolveEvidence` retains inverse-defect, residual, input-family and restored-output bounds; `FactorRef` is an opaque reusable resource, never an exported certified factor matrix. `JointModel`, `CompanionSetDraft` and `MomentView` retain explicit source relations and statistical parameter evidence. Detailed contracts live in `src/heap/`, `src/binding/`, `src/publication/`, `src/kernels/`, `src/solve/`, `src/conversion/`, `src/statistics/` and `src/provenance/`.

## Read-only descriptor adapters · `sec:matrix:reading-adapters`

`ReadingAdapter<T>` retains a bindings-owned representation witness and every selected storage position as an `EntryTicket<T>`. The opaque ticket identifies the numerical owner's scope, occupation and version; each entry index is a base-region logical position. A Complex block has four selected positions even though it has two logical value coordinates. Exact `None` has no numerical positions and requires a whole-family constancy proof. Missing or repeated positions refuse. No adapter resolves a heap handle or grants live access.

`ReadingCoverage<T>` checks a complete required entry universe and every pairwise intersection of its reading adapters. Each intersecting pair needs a `ReadingRelation` naming the exact ordered selected coordinates and complete physical definitions. A proof for another equal copy or logical result projection refuses. Every required entry must be covered, every selected entry must belong to the universe, and an unused or absent overlap claim refuses. These are metadata checks over owner-supplied tickets; the owner must match the universe to its actual live regions, including scratch and intermediate entries.

`CompanionAdapter<T>` keeps complete parameter positions and exact target adapters, distinguishing numerical error, deterministic bounds and ordered covariance through the bindings record. `AffineAdapter<T>` matches both represented endpoints of a checked affine operation. `OperatorAdapter<T>` keeps every formal coefficient, the exact represented input and planned native-product result positions, with the result definition, axes and references reached through its `OperatorApplication`. These are operation requirements and inspectable evidence, not executable kernel authority or numerical enclosure certificates.

`Heap::import_readings` matches the complete value and companion entry universe to actual finite regions before allocation. One coverage inventory retains one allocation sponsor and any number of checked readers. `ReadingBinding` preserves the exact reading, ordered stored copies, companion maps, schema, precision and entry revisions. A no-storage reading allocates no floats but remains registered for structural invalidation. Covariance and factor parameters, source sensitivities, means, evaluation radii and product contributions have complete per-entry bindings declarations over actual mathematical symbols.

Adapter tests check total and partial coverage, every pair intersection, wrong stored copies, substituted targets, changed opaque versions, no-storage exact values and complete operation inventories. Every budgeted constructor has exact Work and staged-payload `N`/`N−1` boundaries under unchanged default ceilings. No floating payload supplies a CAS premise.

## Guarantees and refusals · `sec:matrix:guarantees`

Finite numerical admission, checked addressing and live owner/version checks precede access. Read selections may overlap or repeat; every output must be injective and all destinations disjoint. Mutation advances the whole base version, including changes through aliases or companion-only replacement. Old dependent evidence becomes stale even when center bytes agree. Compaction can preserve mathematical versions while changing the physical layout epoch.

Outputs and every required companion remain private until complete validation. Preparation performs fallible allocation and verification before a commit that runs no allocation, numerical kernel or caller callback. A late refusal publishes no prefix. Failure records can retain independently completed proof stages, but never manufacture a successful result handle. Explicit heap, scratch, provenance and execution limits bound work and publication; allocator aborts remain outside recoverable-capacity guarantees.

Deterministic kernels enclose their exact laws over the admitted input boxes. Matrix multiplication includes both propagated input terms and their interaction. A solve requires an independently checked inverse-defect margin and residual; bounded solves also need a positive perturbation margin. The numerical-only error is already included in the total radius and must not be added twice. Affine conversion propagates the selected input numerical-error companion through the absolute scale and adds the new map discrepancy; repeated conversion retains the original nominal target. Declared input bounds remain assumptions; represented-only coverage does not become physical exactness merely because a computed bound exists.

Statistical covariance and its numerical evaluation error are separate from deterministic radii. Repeated operands retain dependence; absent or unknown correlations never mean independence. Affine moments and nonlinear local linearizations retain different assurance. A budget refusal can preserve a valid certificate whose bound is insufficient; a small residual alone cannot pass the budget. Factor reuse revalidates exact operator dependencies and recomputes each new result's certificate.

Successful runs of the same exact target reproduce within tracked error under admitted arithmetic models. Centers may differ across valid schedules or environments. The existing same-environment transcript tests are narrower regression evidence, not a cross-machine bitwise guarantee.

## Current scope and limits · `sec:matrix:scope`

Implemented operations include copy, homogeneous affine/precision conversion, addition/subtraction, fixed linear maps, entry transforms, matrix products, elementwise square/reciprocal/square root, verified nonempty square solve/inverse, finite recovery, eligible factor reuse and the documented statistical propagation. Original physical and whole-box domain checks remain mandatory. Numerical π-dependent conversion refuses at exact export; the current kernels do not include a certified π evaluator, generic eigensolver or integrator.

Binding admission in `src/binding/symbols.rs` requires complete coverage and refuses unrelated overlapping declarations. Homogeneous recipes remain restricted to coherent concrete differences. `select_reading` admits identity-coordinate homogeneous general readings and preserves their source lineage through copies, snapshots and provenance. It currently refuses companion-bearing general readings; executing their bound, error and covariance maps belongs to coordinate lowering. General expanded layouts are limited to complete Complex blocks with matching represented copies; unequal rounded copies with independent errors require further numerical family admission. Private kernel scratch and lowered coefficient temporaries still need explicit entry inventories.

Cached CAS reading and witness dependency inventories are reused on writes. Direct family maps retain per-entry revisions: writing the current component invalidates that component and the whole state while an independent voltage component survives. Domain predicates and overlap proofs are queried separately, including across reading groups on the same graph. Missing fine inventories and unsupported storage-to-family maps invalidate conservatively. Ordinary whole-base replacement and retirement also invalidate readers. Native bounded results refuse value-only entry writes because their companions must change atomically.

## Testing · `sec:matrix:testing`

Public conformance tests check exact-rational containment, independent square-root/norm oracles, strict domains, stale/foreign handles, alias selections, publication rollback, numerical coverage and exact `N`/`N−1` capacity/work boundaries in both formats. Compile twins enforce category, precision, borrow and checked-state boundaries. Symbol lifecycle tests cover pending/abandoned outputs, wrong owners, complete companion inventories and repeated copies.

Integration acceptance exercises the physical truss and reduced common-source resistor ladder, restored outputs and cross-point covariance. The full five-by-five statistical ladder remains a symbolic-budget refusal; direct mixed numerical admission also refuses. These tests do not stand in for LC execution. Run `cargo test -p nrz-matrix --test public` for public contracts and lifecycles. `docs/records/matrix-core-acceptance.md` records the acceptance environment, measured costs and limits; `src/solve/mod.rs` retains the measurement commands and coverage details.
