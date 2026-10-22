<!-- SPDX-License-Identifier: CC0-1.0 -->

# nrz-accuracy · `guide:accuracy:overview`

## Purpose · `sec:accuracy:purpose`

`nrz-accuracy` composes nonnegative numerical bounds and makes conservative accuracy decisions for the engine's float computations. The exact descriptor graph states what a result means; the computational graph records which region versions and kernel applications produced it. Accuracy supplies the arithmetic and policy for deciding whether an already certified bound meets a request. It does not establish the certificate's physical meaning or turn a small residual into a proved result.

## Place in the engine · `sec:accuracy:engine`

Accuracy depends only on `nrz-scalar` in production. Matrix uses both: scalar encloses primitive operations and imported constants, accuracy combines magnitudes and controls finite recovery policies, and matrix retains the resulting evidence against live heap versions. The descriptor dependency direction is matrix → bindings → CAS; neither scalar nor accuracy depends back on that meaning layer.

A region's interpretation comes from its binding. Accuracy therefore owns no heap, quantity ledger, assurance token, source model or provenance store. Its caller validates the target, coordinate system, norm, coverage and versions before giving a magnitude physical significance. Exact constants reach these numerical calculations only after bindings exports them and scalar encloses their spellings. See `docs/design/descriptor-binding.md` and `docs/design/matrix-core-design.md`.

## In the LC target · `sec:accuracy:lc-target`

The reference `L=25 mH`, `C=10 µF` has `ω₀=2000 rad/s` and `f₀=1000/π Hz`. A requested angular-frequency tolerance must be compared with an error bound restored to angular-frequency coordinates. The voltage/current state `(v,i)` cannot acquire a physical Euclidean norm merely by putting its two coordinates in one buffer: bindings must first establish the references and metric, and matrix must restore each error with its value.

Accuracy already provides the nonnegative arithmetic, explicit norms, budgets and bounded refinement/promotion state used by the existing homogeneous solves. The coordinate-lowering step will connect those foundations to mixed-coordinate restoration, and the spectral-lowering step and the numerical-acceptance step will establish and test the LC spectral bounds. The LC-quantities step must provide certified π evaluation before a numerical `f₀` budget is meaningful. The LC-plans step will assemble observable policies, and the LC-reproducibility step will compare reruns within tracked error. These later stages in `docs/design/lc-resonance.md` are not implemented LC capabilities of this crate.

## Main concepts and types · `sec:accuracy:concepts`

`Nonnegative<P>` admits a finite nonnegative magnitude. `BoundVectorRef` validates and borrows an immutable slice; its reductions produce outward bounds. `NormSummary` retains the chosen `NormKind`, logical entry count and upper bound. The caller supplies every logical entry and multiplicity, such as both off-diagonal copies of a symmetric matrix.

```rust
use nrz_accuracy::{BoundVectorRef, NormKind};
use nrz_scalar::ArithmeticContext;

let context = ArithmeticContext::new()?;
let entries = [3.0_f64, 4.0];
let summary = BoundVectorRef::new(&entries)?.norm(NormKind::Euclidean, &context)?;
assert!(summary.upper().get() >= 5.0);
# Ok::<(), nrz_accuracy::AccuracyFailure>(())
```

`Budget` holds absolute and relative tolerances plus an explicit positive difference reference scale. `assess_budget` returns a `BudgetAssessment` and a `BudgetDecision`. `RecoveryPolicy` starts a `Recovery` with cumulative `Limits`; the caller reserves attempts, runs its own kernel, and supplies a certified bound or arithmetic failure. `RecoveryEvidence` retains progress and stopping reasons. Detailed contracts are in `src/nonnegative.rs`, `src/vector.rs`, `src/budget.rs`, `src/work.rs` and `src/recovery.rs`.

## Guarantees and refusals · `sec:accuracy:guarantees`

Bound addition, scaling and contractions round outward through scalar enclosures. All fallible arithmetic is explicit. Budget assessment encloses `absolute + relative * reference_scale`: `Met` requires the error upper bound to be at most the lower threshold endpoint, `Exceeded` requires it to exceed the upper endpoint, and overlap yields `Unresolved`. `Exceeded` rejects the available bound, not necessarily the actual error. No request gives `NotRequested`, never `Met`.

The reference scale remains mandatory for an absolute-only budget; a shifted point center is not an implicit relative reference. Negative or nonfinite magnitudes, unequal contraction lengths and unavailable finite arithmetic refuse. `One` and `Infinity` are entry norms, not induced matrix norms; this crate checks no physical metric.

Attempt and work reservations are atomic, bounded and never refunded after execution fails. Refinement progress means a strictly smaller best certified radius. Stagnation, limits and arithmetic failure stop explicitly. Authorized binary32-to-binary64 retry preserves cumulative charges and budget widths, but requires fresh numerical certificates from the caller. A reduced residual alone cannot authorize success.

## Current scope and limits · `sec:accuracy:scope`

Accuracy supports scalar's sealed binary32/binary64 formats and admitted arithmetic environment. It executes no solver and retains no numerical buffers. It provides deterministic magnitude policies, not a statistical model: covariance cannot substitute for a worst-case radius. Physical/source assumptions and existing import or conversion errors remain the numerical owner's responsibility.

Recovery supports one explicitly authorized promotion from binary32 to binary64 after specified recoverable stops. It cannot recover information lost in an earlier input conversion, widen a physical domain or authorize arbitrary retries. Successful executions of the same exact target can have different centers while agreeing within their tracked error; these policies promise no bitwise reproducibility.

## Testing · `sec:accuracy:testing`

Independent exact-rational oracles check addition, scaling, contractions and every named norm in both formats, including ties, subnormals, underflow and huge/tiny mixtures. Public lifecycles preserve logical multiplicity and compare norms before budget decisions; compile twins reject forged private state. Budget tests cover equality, overlap and rounded-up thresholds that would falsely accept. Recovery tests exercise exact resource boundaries, overflow, state transitions, strict progress, failure evidence and promotion with cumulative charges. Run `cargo test -p nrz-accuracy --test public` for the public contract and lifecycle tests; rustdoc checks the example and compile twins.
