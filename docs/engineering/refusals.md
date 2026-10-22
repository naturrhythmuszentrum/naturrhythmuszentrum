<!-- SPDX-License-Identifier: CC0-1.0 -->

# Named refusals and stopping outcomes · `spec:nrz:engineering-refusals`

**Definition (Refusal interpretation)** · `def:nrz:engineering-refusal-meaning`

Invalid input, unsupported operations, unavailable evidence, stale identities, arithmetic range and resource exhaustion name different failed obligations. Refusing a certificate establishes no opposite mathematical theorem, and an exceeded available bound does not measure actual error (`rule:vision:completion-boundaries`).

**Rule (Preserve outcome distinctions)** · `rule:nrz:engineering-refusal-causes`

Wrappers retain their underlying numerical or symbolic cause. Recovery stop enums also contain successful outcomes; their names alone cannot classify every variant as failure. Missing covariance or unavailable output cannot become a fabricated value (`rule:nrz:matrix-core-joint-sources`), (`rule:nrz:descriptor-binding-result-symbols`).

**Table (Adopted named outcomes)** · `tab:nrz:engineering-refusal-inventory`

|Owner|Type|Variant|Category|Meaning|Source|
|-----|----|-------|--------|-------|------|
|ACCURACY|`AccuracyFailure`|`Arithmetic`|failure|The scalar substrate cannot enclose the operation.|`crates/accuracy/src/error.rs`|
|ACCURACY|`AccuracyFailure`|`InvalidBound`|failure|A bound entry is negative or nonfinite.|`crates/accuracy/src/error.rs`|
|ACCURACY|`AccuracyFailure`|`InvalidBudget`|failure|Tolerances or the positive reference scale fail admission.|`crates/accuracy/src/error.rs`|
|ACCURACY|`AccuracyFailure`|`InvalidPolicy`|failure|The stagnation window is invalid.|`crates/accuracy/src/error.rs`|
|ACCURACY|`AccuracyFailure`|`InvalidRecoveryState`|failure|The operation violates the recovery phase or retry permission.|`crates/accuracy/src/error.rs`|
|ACCURACY|`AccuracyFailure`|`LengthMismatch`|failure|Contraction vectors have different lengths.|`crates/accuracy/src/error.rs`|
|ACCURACY|`AccuracyFailure`|`ResourceLimit`|failure|An attempt or work reservation exceeds its cumulative allowance.|`crates/accuracy/src/error.rs`|
|ACCURACY|`ResourceLimit`|`Attempts`|resource|No further candidate or refinement attempt can be reserved.|`crates/accuracy/src/work.rs`|
|ACCURACY|`ResourceLimit`|`Work`|resource|Requested work exceeds the cumulative allowance.|`crates/accuracy/src/work.rs`|
|ACCURACY|`StopReason`|`Arithmetic`|refusing stop|Arithmetic or tolerance evaluation failed.|`crates/accuracy/src/recovery.rs`|
|ACCURACY|`StopReason`|`BudgetMet`|successful stop|The certified upper radius satisfies the requested tolerance.|`crates/accuracy/src/recovery.rs`|
|ACCURACY|`StopReason`|`NotRequested`|successful stop|No budget was requested; the available bound ends recovery.|`crates/accuracy/src/recovery.rs`|
|ACCURACY|`StopReason`|`RefinementLimit`|refusing stop|The configured refinement allowance is used.|`crates/accuracy/src/recovery.rs`|
|ACCURACY|`StopReason`|`ResourceLimit`|refusing stop|A cumulative attempt or work reservation failed.|`crates/accuracy/src/recovery.rs`|
|ACCURACY|`StopReason`|`Stagnation`|refusing stop|The certified radius did not improve within the permitted window.|`crates/accuracy/src/recovery.rs`|
|DOCGEN|`Class`|`Changed`|failure|Committed bytes differ from the current generation.|`tools/docgen/src/check.rs`|
|DOCGEN|`Class`|`Missing`|failure|An expected generated path is absent.|`tools/docgen/src/check.rs`|
|DOCGEN|`Class`|`Stale`|failure|An undeclared managed path remains in the tree.|`tools/docgen/src/check.rs`|
|DOCGEN|`Class`|`Unstable`|failure|Repeated generation does not preserve bytes or inventory.|`tools/docgen/src/check.rs`|
|DOCGEN|`Error`|`DatasetCompatibility`|failure|A required dataset is absent or incompatible.|`tools/docgen/src/error.rs`|
|DOCGEN|`Error`|`Envelope`|failure|A required envelope field is absent or invalid.|`tools/docgen/src/error.rs`|
|DOCGEN|`Error`|`Io`|failure|A filesystem operation failed at the retained path.|`tools/docgen/src/error.rs`|
|DOCGEN|`Error`|`JsonSyntax`|failure|A committed JSON document is syntactically invalid.|`tools/docgen/src/error.rs`|
|DOCGEN|`Error`|`Manifest`|failure|The declared output inventory is invalid.|`tools/docgen/src/error.rs`|
|DOCGEN|`Error`|`Membership`|failure|Workspace membership or a supported manifest fact is invalid.|`tools/docgen/src/error.rs`|
|DOCGEN|`Error`|`OrdinalCode`|failure|A tracked path or text line contains an ordinal schedule code; the refusal retains its path, line and exact matched spelling.|`tools/docgen/src/error.rs`|
|DOCGEN|`Error`|`Payload`|failure|A typed record or its semantic constraints are invalid.|`tools/docgen/src/error.rs`|
|DOCGEN|`Error`|`Template`|failure|A template cannot be loaded or evaluated.|`tools/docgen/src/error.rs`|
|DOCGEN|`Error`|`TemplateHeader`|failure|A template header violates the declared source contract.|`tools/docgen/src/error.rs`|
|DOCGEN|`Error`|`TomlSyntax`|failure|An input TOML document is syntactically invalid.|`tools/docgen/src/error.rs`|
|DOCGEN|`Error`|`TrackedText`|failure|Git cannot supply the tracked-file inventory, or that inventory contains a path that is not UTF-8.|`tools/docgen/src/error.rs`|
|SCALAR|`ArithmeticFailure`|`InvalidLiteral`|failure|Text is outside the bounded numeric grammar.|`crates/scalar/src/error.rs`|
|SCALAR|`ArithmeticFailure`|`InvertedEndpoints`|failure|The lower endpoint exceeds the upper endpoint.|`crates/scalar/src/error.rs`|
|SCALAR|`ArithmeticFailure`|`LengthMismatch`|failure|Dot-product slices differ in length.|`crates/scalar/src/error.rs`|
|SCALAR|`ArithmeticFailure`|`LiteralRange`|failure|Literal evaluation needs an unbounded intermediate or result.|`crates/scalar/src/error.rs`|
|SCALAR|`ArithmeticFailure`|`NegativeRoot`|failure|The real root enclosure includes negative inputs.|`crates/scalar/src/error.rs`|
|SCALAR|`ArithmeticFailure`|`NonFinite`|failure|An input is infinite or not a number.|`crates/scalar/src/error.rs`|
|SCALAR|`ArithmeticFailure`|`Overflow`|failure|A candidate or required outward neighbor is nonfinite.|`crates/scalar/src/error.rs`|
|SCALAR|`ArithmeticFailure`|`ResourceLimit`|failure|Literal digits, exponent or work exceed the declared ceiling.|`crates/scalar/src/error.rs`|
|SCALAR|`ArithmeticFailure`|`UnsupportedEnvironment`|failure|The target or floating-environment probes are unsupported.|`crates/scalar/src/error.rs`|
|SCALAR|`ArithmeticFailure`|`ZeroInDivisor`|failure|Division is undefined within the divisor enclosure.|`crates/scalar/src/error.rs`|

**Argument (Finite arithmetic and policy refusals)** · `arg:nrz:engineering-numerical-refusals`

Scalar domain, literal and finite-range failures stay distinct from accuracy policy, state and resource failures. Arithmetic failures preserve valid earlier recovery evidence without inventing a successful tolerance comparison (`[SCALAR-claim:scalar:primitive-domain-and-range-refusals]`), (`[SCALAR-claim:scalar:literal-range-and-divisor-refusals]`), (`[ACCURACY-claim:accuracy:budget-domain-and-overflow-refusals]`), (`[ACCURACY-claim:accuracy:recovery-arithmetic-stopping-evidence]`).

**Argument (Generator refusals)** · `arg:nrz:engineering-generator-refusals`

The generator rejects malformed envelopes before typed payloads, rejects forged committed evidence, and reports source/template/inventory drift without writing during check. The engineering catalogue additionally refuses changed reviewed sources and malformed observation provenance (`[DOCGEN-claim:docgen:envelope-error-precedes-payload-error]`), (`[DOCGEN-claim:docgen:refuses-legacy-formulas-and-forged-evidence]`), (`[DOCGEN-claim:docgen:binary-fixed-point-and-exit-classes]`), (`[DOCGEN-claim:docgen:engineering-record-mutations-refuse]`).

## Open intents · `sec:nrz:engineering-refusal-intents`

**Claim (Remaining owner refusal catalogues)** · `claim:nrz:engineering-remaining-refusals`

The remaining owner documentation will register refusal variants and their executable claims, preserving mathematical nonexistence, missing proof and resource exhaustion as distinct outcomes; symbolic lowering and numerical acceptance own their LC-facing use (`step:nrz:lc-resonance-symbolic-lowering`), (`step:nrz:lc-resonance-numerical-acceptance`).

**Claim (LC domain outcomes through reusable plans)** · `claim:nrz:engineering-lc-refusal-outcomes`

Phasor, spectral, quantity and physics stages will retain successful independent outputs beside undefined, refused, blocked or inactive results, with complete entry coverage, checked binding overlap and explicit branch/domain evidence (`step:nrz:lc-resonance-phasor-lowering`), (`step:nrz:lc-resonance-spectral-lowering`), (`step:nrz:lc-resonance-quantity-bite`), (`step:nrz:lc-resonance-physics-bite`).
