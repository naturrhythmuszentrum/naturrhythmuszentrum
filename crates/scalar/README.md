<!-- SPDX-License-Identifier: CC0-1.0 -->

# nrz-scalar · `guide:scalar:overview`

`nrz-scalar` supplies sealed binary32/binary64 element arithmetic for the matrix kernels. `Finite<P>` admits finite primitives and canonicalizes signed zero. `Enclosure<P>` is a closed real set with private finite endpoints; it owns no quantity, ledger, descriptor or physical uncertainty. See the matrix-core design for the heap's use of this substrate.

`ArithmeticContext::new` admits only the `x86_64-unknown-linux-gnu` target and checks selected rounding and gradual-underflow probes. Certification additionally requires the independent boundary corpus to pass for each compiler and profile. Arithmetic assumes IEEE round-to-nearest ties-to-even, gradual underflow and correctly rounded primitives. Fast math, excess precision and changes to the floating environment are unsupported; probes cannot establish that external code preserves that environment. Callers can retain the context's arithmetic revision in their records.

Addition, subtraction, multiplication, division and square root enclose each rounded result by its outward neighbours. Only proved zero/unit identities bypass widening; a zero rounded residual proves no exactness. Multiplication and division take all four endpoint pairs, square handles zero crossings and nonnegativity, and negation and absolute value are exact. Nonfinite admission, inverted endpoints, zero-containing divisors, negative square roots and nonfinite required neighbours return errors. The arithmetic context is explicit at every rounded operation.

Public conformance tests decode binary endpoints into independent exact rationals, cover cancellation, ties, subnormals and finite limits, and square root endpoints before comparison. The Rump lifecycle contains the independent target −54767/66192 in both formats. Compile twins exercise finite-state privacy and format sealing. This commit exposes primitive arithmetic only; reductions, norms, fused evaluation, precision changes and literal decoding follow separately.
