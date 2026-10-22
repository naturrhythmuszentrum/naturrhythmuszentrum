<!-- SPDX-License-Identifier: CC0-1.0 -->

# Engineering catalogue evidence · `guide:docgen:engineering-evidence`

**Rule (Reviewed source fingerprints)** · `rule:docgen:source-review-fingerprint`

The source fingerprint processes bytes in order by exclusive-or and wrapping multiplication using the registered initial word and multiplier; only files supporting authored catalogue rows are fingerprinted. A changed fingerprint names the source, guarded rows and input fields to review before the catalogue can render again. This detects source-version drift under the declared recurrence and grants no authenticity or mathematical-truth claim.

**Rule (Recorded observations)** · `rule:docgen:recorded-observations`

An observation retains its exact decimal spelling, unit, UTC date, complete commit identifier, executed command and server source; the generator validates and renders those records without running their commands or inventing missing observations.

## Unit test matrix · `tab:docgen:engineering-unit-test-matrix`

**Table (Unit test matrix)**

|Test|Area|Claim|
|----|----|-----|
|(`test:unit:engineering-record-order-and-fixed-point`)|docgen|Reviewed records serialize deterministically and sort observation identities.|
|(`test:unit:engineering-source-drift-refuses`)|docgen|Any changed reviewed source refuses before the catalogue can render.|
|(`test:unit:engineering-record-mutations-refuse`)|docgen|Invalid provenance, table structure, source ownership and unknown fields refuse explicitly.|
|(`test:unit:engineering-render-rechecks-inputs`)|docgen|Rendering rejects forged stored rows even when their envelope remains valid.|
