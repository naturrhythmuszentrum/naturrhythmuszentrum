<!-- SPDX-License-Identifier: CC0-1.0 -->

# nrz-scalar · `guide:scalar:overview`

## Purpose · `sec:scalar:purpose`

`nrz-scalar` is the engine's boundary from exact constant spellings to finite floating values with proven enclosures and import error. It also supplies the outward arithmetic used to verify float kernels. In the two-graph architecture, exact descriptors stay in CAS while numerical region versions and kernel applications form the computational graph. Scalar supports the latter without assigning meaning to its numbers: an enclosure is a closed real set, whether a binding later reads its coordinates as voltage, current or something else.

## Place in the engine · `sec:scalar:engine`

Scalar has no production dependency on CAS, bindings, accuracy or matrix. Bindings exports a checked exact constant spelling; `NumericLiteral` encloses that spelling in the requested binary32 or binary64 format; matrix retains the candidate, enclosure and import error with the numerical computation. Accuracy builds nonnegative bounds and budget policies on scalar, and matrix uses both crates. Heap values never travel back into CAS arithmetic.

Scalar owns finite admission, temporary enclosures, primitive arithmetic, reductions, format changes, bounded literal decoding and a certified mathematical pi enclosure. It owns no heap, quantity descriptor, physical uncertainty, source identity or coverage ledger. Bindings supplies the reading of a region; matrix is responsible for attaching scalar's numerical evidence to the right region version. See `docs/design/descriptor-binding.md` and `docs/design/matrix-core-design.md` for those boundaries.

## In the LC target · `sec:scalar:lc-target`

For `L=25 mH=1/40 H` and `C=10 µF=1/100000 F`, the exact descriptor calculation gives `ω₀=2000 rad/s` and `f₀=1000/π Hz`. Scalar can already enclose rational spellings such as `1/40` and `1/100000`, retaining the discrepancy of each floating candidate from its exact value. Its multiplication, division and square-root enclosures are numerical ingredients for the later LC kernels; decoding the constants alone does not certify an LC computation.

The coordinate and spectral paths in `docs/design/lc-resonance.md` use scalar arithmetic for their supported numerical kernels. `PiEnclosure` supplies checked rational bounds of mathematical pi, outward binary endpoints and a separate constant revision. Bindings recognizes exact rational factors of pi or its reciprocal; matrix explicitly admits those coefficients through a shared `PiConstant` owner before native affine conversion. The scalar primitive has no physical meaning or graph identity. A decimal approximation to pi is not an exact replacement for its definition.

## Main concepts and types · `sec:scalar:concepts`

`Scalar` seals the formats to `f32` and `f64`. `Finite<P>` admits a finite primitive and canonicalizes signed zero. `ArithmeticContext` admits the supported floating environment. `Enclosure<P>` keeps private, finite, ordered endpoints and provides fallible operations under that context. `NumericLiteral` and `LiteralLimits` describe a bounded exact spelling; decoding returns a `DecodedLiteral` with a center, enclosure, displacement bound and charged work.

```rust
use nrz_scalar::{ArithmeticContext, Finite, LiteralLimits, NumericLiteral};

let context = ArithmeticContext::new()?;
let literal = NumericLiteral::new("1/8", LiteralLimits::default())?;
let decoded = literal.decode::<f64>(&context)?;
assert!(decoded.enclosure().contains(Finite::new(0.125)?));
# Ok::<(), nrz_scalar::ArithmeticFailure>(())
```

`sum` and `dot` use a fixed increasing-index fold; `norm_upper` bounds a Euclidean or logical-entry Frobenius norm. `widen`, `narrow` and `PrecisionChange` make representation changes explicit. Their displacement is an additional numerical contribution; the caller must preserve existing error. Detailed arithmetic, decoding and conversion contracts live in `src/enclosure.rs`, `src/fused.rs`, `src/reduction.rs`, `src/literal/mod.rs` and `src/conversion.rs`.

**Rule (Bounded pi constant policy)** · `rule:scalar:pi-policy`

`PiEnclosure::new` refines `pi = 16 atan(1/5) - 4 atan(1/239)` from `3 < pi < 4`. Revision 1 encloses alternating terms and their signed next-term remainders with integer numerators over `2^96`, intersects successive rational bounds, then quantizes both endpoints outward. The lower endpoint is the candidate; its outward displacement includes quantization and must fit the caller’s positive absolute tolerance. The rational bounds remain inspectable through `PiBounds`. This algorithm has finite rational and binary resolution; it does not accept expression text.

`PiLimits` has no defaults. `PiBudget` commits cumulative Work, paired refinement steps and logical output bytes only on success; staged bytes record a high-water mark of fixed algorithm state plus result. Work prepays five initialization units, 26 units per paired refinement, 22 units per endpoint plus one per exact binary scaling, and one unit per displacement check. Logical storage excludes stack frames, compiler temporaries, the caller’s context/ledger and allocator overhead. There is no heap allocation or cache. `PiError` distinguishes invalid tolerance, unavailable resolution, Work, steps, staged bytes, output bytes and scalar arithmetic; every refusal preserves prior usage.

## Guarantees and refusals · `sec:scalar:guarantees`

Successful arithmetic contains the exact real result under the admitted arithmetic model. Rounded operations widen outward; only proved identities avoid widening. A rounded zero residual does not prove exactness. Literal decoding promises containment, not a tight interval or a correctly rounded fraction candidate. Numerical reproducibility means agreement within the tracked error for the same exact target, not identical result bits.

Nonfinite inputs, inverted endpoints, division by an enclosure containing zero, negative square-root domains and unavailable finite outward neighbours refuse. Overflowing intermediates can refuse even when another algorithm would succeed. Literal digit, exponent and work limits are finite; zero allowances are valid refusing policies.

## Current scope and limits · `sec:scalar:scope`

`ArithmeticContext::new` restricts admission to `x86_64-unknown-linux-gnu` and checks selected rounding and gradual-underflow probes. Each compiler/profile also needs the independent boundary corpus. IEEE round-to-nearest ties-to-even, gradual underflow and correctly rounded primitives are assumptions; fast math, excess precision and a changed floating environment are unsupported. Probes cannot guarantee that external code preserves those assumptions.

The decoder accepts integers, decimals, scientific notation and one literal fraction, with no CAS or production rational dependency. It is not an expression interpreter. The separate pi primitive evaluates only its fixed mathematical definition. Scalar proves numerical containment; physical domains, source assumptions and operation-specific meaning remain above it.

## Testing · `sec:scalar:testing`

Public conformance tests independently decode binary endpoints into exact rationals, including cancellation, halfway ties, subnormals, underflow and finite limits in both formats. Square-root and norm checks compare squared bounds with exact rational targets. The Rump lifecycle contains the independent target `−54767/66192`; import/conversion lifecycles keep old error and new displacement separate. Compile twins enforce private finite state and sealed formats, while literal tests cover syntax and resource refusals. Pi tests use the independent rational identity `pi/4 = atan(1/2) + atan(1/3)`, check nested refinement and both formats, and pin atomic resource boundaries. The public target is run with `cargo test -p nrz-scalar --test public`; rustdoc checks the example and compile twins.
