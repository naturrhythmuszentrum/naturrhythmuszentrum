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
