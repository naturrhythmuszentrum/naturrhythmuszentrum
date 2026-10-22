<!-- SPDX-License-Identifier: CC0-1.0 -->

# Schedule-code guard evidence · `guide:docgen:ordinal-evidence`

This folder tests the schedule-code grammar used by `nrz-docgen check`: named ordering contexts refuse numeric plan codes, while mathematical symbols, measured numbers, types and ordinary English retain their meanings. The tests cover prose, identifiers, numeric heads and ranges without filename exceptions.

## Unit test matrix · `tab:docgen:ordinals-unit-test-matrix`

**Table (Unit test matrix)**

|Test|Area|Claim|
|----|----|-----|
|(`test:unit:ordinal-ordering-forms-refuse`)|docgen|Ordering vocabulary catches prose, slug and identifier forms with exact spans.|
|(`test:unit:ordinal-heads-and-slices-refuse`)|docgen|Numeric heads, contextual LC ranges and CAS slice identifiers refuse.|
|(`test:unit:ordinal-mathematical-lookalikes-pass`)|docgen|Physical symbols, literals, types and mathematical step indices stay valid.|
