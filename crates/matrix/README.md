<!-- SPDX-License-Identifier: CC0-1.0 -->

# nrz-matrix · `guide:matrix:overview`

`nrz-matrix` owns a homogeneous descriptor heap with separate binary32 and binary64 faer arenas. A scalar is a 1×1 region. Values and declared absolute bounds have separate storage regions and units descriptors; every bound names the exact identity and version of its entire target region. Heap coordinates never enter CAS arithmetic. See the matrix-core design for the complete operation contract.

`Heap::import_represented` admits finite coherent coordinates with explicitly represented-only coverage. `Heap::import_declared_bounds` atomically admits finite centers and nonnegative finite componentwise radius entries, deriving the radius descriptor through units. These supplied bounds remain caller assumptions. Imports use logical column order, canonicalize signed zero, and retain point/difference roles. Floating count storage and noncoherent coordinate imports refuse. Unsupported nested relative-radius derivation preserves the units error.

Opaque `Handle<Category, P>` locators carry a heap scope and descriptor occupation generation. Value and companion access checks ownership, occupation, category, precision, region bounds and target versions before lending a read-only `RegionView`. Equal schemas are interned using exact descriptor equality without merging value identities. Each arena slab owns a one-column faer matrix; logical rectangles select its entries through checked layouts. `RegionView::export` returns an owned raw copy without evidence or a declaration of source independence. Live borrows prevent heap mutation.

`HeapLimits` bounds descriptor slots, elements per precision (including companions), and immutable schemas. Preparation scans and validates the complete bundle, reserves metadata capacity and builds every slab before an allocation-free publication step. Replacement scratch is bounded by the same element limit as the new live bundle; old and prepared storage may coexist, so peak heap-owned addressable elements can reach twice that limit, in addition to backend padding and metadata. A refusal publishes no partial value or companion. Rust allocator aborts remain outside the recoverable-capacity contract. Process-local identities never wrap and make no persistence promise.

```rust
use nrz_matrix::{Extent, Heap, HeapLimits};
use nrz_units::Descriptor;

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

`Heap::replace_represented` and `Heap::replace_declared_bounds` require the caller's captured `ValueVersion`. They preserve the value handle and `ValueId`, advance the version exactly once, and atomically replace the entire supported companion set. Extents, quantity schemas and precision remain fixed. Even identical center bits with changed bounds advance the version. Represented replacement explicitly removes declared-input coverage. Captured old bound handles become stale; unrelated values and companions remain valid. A stale expected version, invalid payload, capacity refusal or exhausted version leaves the old bundle unchanged.

`Heap::retire` retires the whole value/bound bundle and releases its storage. Vacant descriptor and arena slots are reused with incremented generations; exhausted generations are never reused. Old handles and cached internal arena keys cannot revive. A fresh import has a fresh `ValueId` and starts at version one even when its schema and bytes match retired data. Schema capacity counts immutable lifetime entries, including those no longer used by a live value. Retiring a base also retires every alias of that occupation. Retiring only an alias releases its descriptor; nested aliases already point directly to the base.

`Layout::strided` checks signed affine extents, products, sums and both extreme addresses against the owning region. `Layout::gather` validates each explicit index under its supplied work limit. Repeated indices and zero strides are valid read selections. Empty maps have no addressable entry. All maps use logical order `i + m*j`; a selection of a view addresses that view's logical entries, and `Layout::compose` checks and flattens the map into base-region offsets. General composition refuses if enumeration exceeds the work limit.

`Heap::alias` creates a descriptor pointing directly to the base value. It retains the base identity and resolves its current version and coverage on every read, including after whole-base replacement. The selected extent and entry map are available from `RegionView`; its schema identity names the owning homogeneous schema before selection. `ValueView::bound` names the whole-base companion, while `Heap::selected_bound` applies the identical entry map to the bound and exposes the base target and current version. An alias cannot yet be used as a replacement destination; checked selected writes arrive with output injectivity validation.

`HeapLimits::with_layout_work` bounds gather validation, general composition and raw/packed copies (default 1,048,576 entries). Copy size is also bounded by the per-precision element capacity. `RegionView::pack` copies any admitted map through checked indices into a fresh contiguous faer slab in logical column order. `PackedRegion` is an owned raw snapshot with read-only access, independent of subsequent heap changes; it carries no publication or numerical evidence. The temporary raw copy and packed slab may coexist during packing. No backend mutable view escapes.

The prototype solve capability has been removed: `DenseSystem`, bare `certified_solve`, mutable-field `Outcome`/`Report`, the old backend adapter and solver-only tests are retired. This heap is not a replacement solve implementation or verification of one. Historical feasibility records and adversarial fixture data remain intact. Production solving returns at the later checked operation boundary.

This subset supplies storage extents, not ordered physical axes or linear-operator admission. Numerical kernels, conversions, covariance/source models, compaction and operation bindings are absent. `Factor` and `Workspace` are category markers only. The only scalar dependencies used here are the sealed primitive-format contract, finite admission, zero and comparison; no prototype scalar interval or certificate is used.
