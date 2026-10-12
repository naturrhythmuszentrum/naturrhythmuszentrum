<!-- SPDX-License-Identifier: CC0-1.0 -->

# Descriptor boundary prototype · `guide:descriptors:overview`

This package is a decision prototype, not an engine API. It depends only on the existing algebra and units packages and demonstrates schema and storage admission in safe Rust.

**Definition (Schema interning)** · `def:descriptors:schema-interning`

Quantity equality is exactly CAS descriptor equality, including measure names and count domains. Hashing uses public dimensions, measures and roles; omitted kind fields cause collisions resolved by full equality. Conversion compatibility is a separate checked relation. IDs include the pool identity. Registry admission is independent of descriptor equality.

Shapes describe ordered coordinate meanings, extents and entry quantities in column-major logical order. This prototype admits only full dense shapes with identity entry maps; block partitions, sparse structures and view maps require a later implementation. Values, sources, precision, normalization maps and storage offsets do not belong in a shape key (`def:descriptors:schema-interning`).

**Rule (Checked access)** · `rule:descriptors:checked-access`

Handles carry category and precision without borrowed storage. Resolution checks heap identity, slot and arena generations, category and range. Value mutation advances a separate version. Covariance owns separate storage and two ordered target selections with captured versions. Equal schemas never identify equal values or shared sources. Exhausted identity counters refuse rather than wrap.

**Caveat (Prototype boundary)** · `cav:descriptors:prototype-boundary`

Floating count storage is refused even when the count descriptor and an exact literal are admitted by the CAS. The heap has append-only contiguous slots and does not implement production allocation, numerical certification or discrete arenas. Covariance attachment checks semantic dimensions and coverage; it does not prove positive semidefiniteness or a statistical model.

Original equations enter `Plan<Unchecked>`. `check` retains CAS witnesses and admits only the exact two-equation resistor recipe implemented here; a dimensionally valid different expression is explicitly unsupported. The immutable context and explicit symbol-to-slot map belong to the plan. `Plan<Checked>` can propose a binding, which captures versions but cannot lend numerical views. `validate` checks actual shapes, versions, coverage and exact maps and acquires an exclusive heap borrow. Only `Bound` lends `NormalizedView`. Physical views and fixed point/difference roles have separate types (`rule:descriptors:checked-access`).

Publication borrows an unpublished candidate and consumes its binding, checks the candidate's binding identity, rechecks output expressions against the retained CAS snapshot, validates output shape contracts, restores coordinates, and checks finite values and paired covariance meanings before appending both result records. A retained candidate cannot authorize another publication because every new binding has a distinct identity. Published handles still need fresh resolution. This state describes semantic admission, not numerical certification (`cav:descriptors:prototype-boundary`).

**Example (Resistor pipeline)** · `ex:descriptors:resistor-pipeline`

A `2 Ω` resistor driven by a shared random voltage source centered at `4 V` gives `(4 V, 2 A)`, where the current is through the resistor. The original equations are `v - R*i = zero_v` and `v + zero_r*i = s`; every zero has a physical descriptor. With solution references `(2 V, 1 A)` and equation references `(2 V, 2 V)`, the normalized operator is `[[1, -1], [1, 0]]`, the right-hand side is `(0, 2)`, and the solution is `(2, 2)`. Source variance `1/4 V²` gives full covariance `[[1/4 V², 1/8 V A], [1/8 V A, 1/16 A²]]`; paired reference scaling makes every normalized coefficient `1/16`. A separate rational oracle and bit comparisons in both f32 and f64 exercise this dyadic fixture.

The propagation law is exact for the stated linear shared-source model with finite second moments and exact resistance. It is neither a deterministic uncertainty radius nor a bound on floating error. Runtime negatives cover hidden invalid original operations, typed zeros, stale versions and companions, mismatched maps and shapes, unsupported coverage, foreign candidates and nonfinite outputs. Rustdoc rejection examples each have a positive twin for category, precision, role, validation state, view provenance, borrow exclusivity and publication state (`ex:descriptors:resistor-pipeline`).
