<!-- SPDX-License-Identifier: CC0-1.0 -->

# Numerical laws and admitted policies · `spec:nrz:engineering-kernels`

**Definition (Evidence boundary)** · `def:nrz:engineering-kernel-evidence`

A catalogue row distinguishes the exact law, admitted input domain, numerical shape, enclosure or policy guarantee and named refusal boundary. A finite candidate alone supplies no error certificate; the cited executable claims describe the tested behavior rather than a universal proof (`def:nrz:descriptor-binding-kernel-law`), (`thm:nrz:descriptor-binding-kernel-enclosure`).

**Table (Adopted numerical laws)** · `tab:nrz:engineering-kernel-laws`

|Owner|Operation|Law|Domain|Shape|Guarantee|Refusals|Source|Evidence|
|-----|---------|---|------|-----|---------|--------|------|--------|
|ACCURACY|Conservative tolerance comparison|absolute + relative\*reference_scale|Nonnegative tolerances and a strictly positive reference scale.|A certified bound and optional numerical budget.|Acceptance uses a conservative lower tolerance endpoint; overlap remains unresolved.|InvalidBudget; Arithmetic|`crates/accuracy/src/budget.rs`|(`[ACCURACY-claim:accuracy:budget-decisions-rational-oracle]`) (`[ACCURACY-claim:accuracy:budget-boundary-decisions]`) (`[ACCURACY-claim:accuracy:tolerance-never-rounds-up-for-acceptance]`)|
|ACCURACY|Bound composition|Nonnegative sums, scales and contractions.|Finite nonnegative magnitudes and matching contraction lengths.|Scalar bounds and bound vectors.|Outward arithmetic retains an upper bound; repeated entries retain their multiplicity.|InvalidBound; LengthMismatch; Arithmetic|`crates/accuracy/src/nonnegative.rs`|(`[ACCURACY-claim:accuracy:bound-composition-rational-oracles]`) (`[ACCURACY-claim:accuracy:bound-vector-composition-lifecycle]`)|
|ACCURACY|Named bound norms|Declared maximum, sum and Euclidean summaries.|Finite nonnegative entries.|Bound vectors.|The selected summary remains conservative, including empty inputs.|InvalidBound; Arithmetic|`crates/accuracy/src/vector.rs`|(`[ACCURACY-claim:accuracy:named-norm-rational-oracles]`) (`[ACCURACY-claim:accuracy:norm-empty-and-overflow-contracts]`)|
|ACCURACY|Bounded recovery and promotion|Retain cumulative attempts and work across admitted recovery transitions.|A valid policy, state and caller-supplied allowance.|A per-precision recovery state.|Reservations precede work and are not refunded by failure or precision retry.|InvalidPolicy; InvalidRecoveryState; ResourceLimit; Arithmetic|`crates/accuracy/src/recovery.rs`|(`[ACCURACY-claim:accuracy:recovery-resource-stops]`) (`[ACCURACY-claim:accuracy:precision-retry-cannot-reset-limits]`) (`[ACCURACY-claim:accuracy:precision-retry-permission-boundaries]`)|
|SCALAR|Addition|x+y|Finite interval endpoints.|Scalar intervals.|Outward endpoint sums enclose the complete interval sum.|NonFinite; InvertedEndpoints; Overflow; UnsupportedEnvironment|`crates/scalar/src/enclosure.rs`|(`[SCALAR-claim:scalar:binary32-primitive-boundary-oracle]`) (`[SCALAR-claim:scalar:binary64-primitive-boundary-oracle]`) (`[SCALAR-claim:scalar:endpoint-hulls-enclose]`)|
|SCALAR|Division|x/y|The complete divisor interval is separated from zero.|Scalar intervals.|The hull of outward endpoint quotients encloses every admitted quotient.|ZeroInDivisor; Overflow; UnsupportedEnvironment|`crates/scalar/src/enclosure.rs`|(`[SCALAR-claim:scalar:binary32-primitive-boundary-oracle]`) (`[SCALAR-claim:scalar:binary64-primitive-boundary-oracle]`) (`[SCALAR-claim:scalar:endpoint-hulls-enclose]`)|
|SCALAR|Ordered dot product|sum x_i\*y_i|Finite slices of matching length.|Paired finite slices.|Each product and fixed-order addition is enclosed independently of backend reduction order.|LengthMismatch; Overflow; UnsupportedEnvironment|`crates/scalar/src/reduction.rs`|(`[SCALAR-claim:scalar:fixed-reduction-rational-oracles]`) (`[SCALAR-claim:scalar:dot-to-norm-lifecycle]`)|
|SCALAR|Fused multiply-add|x\*y+z with a fused rounding schedule.|Finite endpoint triples.|Scalar intervals.|The outward corner hull encloses the multi-affine image under the explicit fused law.|Overflow; UnsupportedEnvironment|`crates/scalar/src/fused.rs`|(`[SCALAR-claim:scalar:fused-endpoint-triples]`) (`[SCALAR-claim:scalar:fused-rounding-is-explicit]`) (`[SCALAR-claim:scalar:fused-intermediate-range]`)|
|SCALAR|Bounded literal import|The exact real value of admitted numeric text.|Admitted grammar within digit, exponent and work ceilings.|A scalar numeric literal.|The independently computed enclosure contains the exact literal and the selected candidate.|InvalidLiteral; LiteralRange; ResourceLimit; ZeroInDivisor|`crates/scalar/src/literal/evaluate.rs`|(`[SCALAR-claim:scalar:literal-constant-rational-oracles]`) (`[SCALAR-claim:scalar:literal-resource-boundaries]`) (`[SCALAR-claim:scalar:literal-range-and-divisor-refusals]`)|
|SCALAR|Multiplication|x\*y|Finite interval endpoints.|Scalar intervals.|The hull of outward endpoint products encloses the complete product family.|NonFinite; InvertedEndpoints; Overflow; UnsupportedEnvironment|`crates/scalar/src/enclosure.rs`|(`[SCALAR-claim:scalar:binary32-primitive-boundary-oracle]`) (`[SCALAR-claim:scalar:binary64-primitive-boundary-oracle]`) (`[SCALAR-claim:scalar:endpoint-hulls-enclose]`)|
|SCALAR|Precision narrowing|Round the center and enclose its representation displacement.|Source center and endpoints fit the destination finite range.|Scalar center and interval.|Directed endpoints retain the old enclosure; a separate bound covers center displacement.|Overflow; UnsupportedEnvironment|`crates/scalar/src/conversion.rs`|(`[SCALAR-claim:scalar:outward-narrowing-rational-oracle]`) (`[SCALAR-claim:scalar:narrowing-halfway-ties]`) (`[SCALAR-claim:scalar:literal-precision-change-lifecycle]`)|
|SCALAR|Scaled Euclidean norm|sqrt(sum x_i\*x_i)|Finite slice entries.|Finite slices.|Positive power scaling avoids direct squaring overflow while retaining an outward norm upper bound.|Overflow; UnsupportedEnvironment|`crates/scalar/src/reduction.rs`|(`[SCALAR-claim:scalar:scaled-norm-rational-oracle]`) (`[SCALAR-claim:scalar:reduction-and-norm-refusals]`)|
|SCALAR|Square|x\*x|Finite interval endpoints.|Scalar intervals.|Endpoint extrema and the exact minimum at the origin enclose the full image.|Overflow; UnsupportedEnvironment|`crates/scalar/src/enclosure.rs`|(`[SCALAR-claim:scalar:binary32-primitive-boundary-oracle]`) (`[SCALAR-claim:scalar:binary64-primitive-boundary-oracle]`) (`[SCALAR-claim:scalar:endpoint-hulls-enclose]`)|
|SCALAR|Square-root|sqrt(x)|The complete input interval is nonnegative.|Scalar intervals.|Monotone outward endpoint roots enclose the full image.|NegativeRoot; Overflow; UnsupportedEnvironment|`crates/scalar/src/enclosure.rs`|(`[SCALAR-claim:scalar:binary32-primitive-boundary-oracle]`) (`[SCALAR-claim:scalar:binary64-primitive-boundary-oracle]`) (`[SCALAR-claim:scalar:endpoint-hulls-enclose]`)|
|SCALAR|Subtraction|x-y|Finite interval endpoints.|Scalar intervals.|Opposite endpoint differences enclose the complete interval difference.|NonFinite; InvertedEndpoints; Overflow; UnsupportedEnvironment|`crates/scalar/src/enclosure.rs`|(`[SCALAR-claim:scalar:binary32-primitive-boundary-oracle]`) (`[SCALAR-claim:scalar:binary64-primitive-boundary-oracle]`) (`[SCALAR-claim:scalar:endpoint-hulls-enclose]`)|
|SCALAR|Ordered sum|sum x_i|Finite entries in fixed index order.|Finite slices.|An outward increasing-index fold encloses the exact sum, including its empty identity.|Overflow; UnsupportedEnvironment|`crates/scalar/src/reduction.rs`|(`[SCALAR-claim:scalar:fixed-reduction-rational-oracles]`) (`[SCALAR-claim:scalar:reduction-order-and-empty-laws]`)|
|SCALAR|Precision widening|Preserve the represented center and enclosure.|The admitted narrower finite format.|Scalar center and interval.|Center and endpoints widen exactly without additional displacement.|The input must already satisfy finite admission.|`crates/scalar/src/conversion.rs`|(`[SCALAR-claim:scalar:exact-widening-preserves-coverage]`)|

**Argument (Primitive and reduction schedules)** · `arg:nrz:engineering-arithmetic-schedules`

Outward primitive bounds, explicit fused rounding and ordered reductions have separate schedules. Intermediate finite-range refusal is observable, and reassociation cannot be silently substituted for the admitted contraction (`[SCALAR-claim:scalar:fused-rounding-is-explicit]`), (`[SCALAR-claim:scalar:fused-intermediate-range]`), (`[SCALAR-claim:scalar:reduction-order-and-empty-laws]`), (`rule:nrz:matrix-core-rounding`).

**Argument (Representation and bound policies)** · `arg:nrz:engineering-representation-policies`

Precision changes preserve existing coverage and account for the changed center separately. Conservative tolerance comparison retains unresolved overlap and absent requests, while recovery retains cumulative work instead of inferring success from a smaller residual (`[SCALAR-claim:scalar:literal-precision-change-lifecycle]`), (`[ACCURACY-claim:accuracy:budget-boundary-decisions]`), (`[ACCURACY-claim:accuracy:certified-radius-stagnation-window]`).

**Definition (Matrix design obligations)** · `def:nrz:engineering-matrix-law-boundary`

The design specifies complete-family affine propagation, product interactions, direct nonlinear domains, verified square solves and complete joint-source statistics. Those laws remain distinct from the future meaning-free heap/kernel boundary and mixed LC lowerings; this catalogue does not replace their source-linked evidence inventory (`tab:nrz:matrix-core-operation-propagation`), (`rule:nrz:matrix-core-solve-theorem`), (`rule:nrz:matrix-core-joint-sources`), (`rule:nrz:descriptor-binding-kernel-boundary`).

## Open intents · `sec:nrz:engineering-kernel-intents`

**Claim (Matrix kernel evidence adoption)** · `claim:nrz:engineering-matrix-kernel-evidence`

The matrix kernel catalogue will register every current kernel law, domain, shape, refusal and executable claim; numerical acceptance owns the integrated LC evidence using those registered contracts (`step:nrz:lc-resonance-numerical-acceptance`).

**Claim (Meaning-free mixed numerical execution)** · `claim:nrz:engineering-lc-kernels`

Region bindings will separate physical meaning from numerical contracts, and coordinate, phasor and spectral lowering will supply their original mixed restoration and closed LC operations with independent enclosure and refusal evidence (`step:nrz:lc-resonance-region-bindings`), (`step:nrz:lc-resonance-coordinate-lowering`), (`step:nrz:lc-resonance-phasor-lowering`), (`step:nrz:lc-resonance-spectral-lowering`).

**Claim (Constant, phase and further domains)** · `claim:nrz:engineering-further-kernels`

The quantity stage will connect exact constants to bounded numerical enclosures and branch-aware phase; the advanced-matrix bite will admit further numerical domains only with their own domain and perturbation arguments (`step:nrz:lc-resonance-quantity-bite`), (`step:nrz:bite-advanced-matrix-domains`).
