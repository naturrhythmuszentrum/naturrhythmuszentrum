<!-- SPDX-License-Identifier: CC0-1.0 -->

# Scalar tests conformance evidence · `guide:scalar:tests-conformance-evidence`

## Integration test matrix · `tab:scalar:conformance-integration-test-matrix`

**Table (Integration test matrix)**

|Test|Area|Claim|
|----|----|-----|
|(`test:integration:finite-format-admission`)|scalar|Canonicalization, adjacency and format identity are visible through the facade.|
|(`test:integration:exact-identity-shortcuts`)|scalar|Only proved identities retain singleton endpoints.|
|(`test:integration:primitive-domain-and-range-refusals`)|scalar|Both formats reject undefined real operations and nonfinite enclosures.|
|(`test:integration:exact-widening-preserves-coverage`)|scalar|Widening preserves exact centers and both endpoints, including every binary32 boundary.|
|(`test:integration:outward-narrowing-rational-oracle`)|scalar|Subnormal loss, finite limits, signs and halfway values retain their exact original targets.|
|(`test:integration:narrowing-halfway-ties`)|scalar|Nearest conversion obeys even-significand tie selection, including zero/subnormal ties.|
|(`test:integration:narrowing-enclosure-and-range`)|scalar|Endpoint conversion is outward even when its rounded center and source center differ.|
|(`test:integration:fused-endpoint-triples`)|scalar|Eight-corner hulls cover mixed signs, uncertainty widths and cancellation.|
|(`test:integration:fused-rounding-is-explicit`)|scalar|Fused cancellation has one rounding and differs from multiply-then-add.|
|(`test:integration:fused-boundary-oracle`)|scalar|Fused arithmetic covers underflow, ties and overflow in both formats.|
|(`test:integration:fused-intermediate-range`)|scalar|Fused evaluation does not accidentally round or overflow its product first.|
|(`test:integration:literal-constant-rational-oracles`)|scalar|Mathematical descriptor constants and signed numeric grammar succeed in both formats.|
|(`test:integration:literal-halfway-ties`)|scalar|Decimal halfway literals contain the exact ties and select the nearest even candidate.|
|(`test:integration:literal-subnormal-and-underflow-oracles`)|scalar|Scientific notation reaches subnormals without forming an overflowing decimal denominator.|
|(`test:integration:literal-syntax-refusals`)|scalar|Grammar rejects expressions, unsupported spellings and malformed fraction components.|
|(`test:integration:literal-resource-boundaries`)|scalar|Limits are enforced at, below and above each resource boundary.|
|(`test:integration:literal-range-and-divisor-refusals`)|scalar|Overflow and zero-containing denominator enclosures refuse even when an exact quotient is small.|
|(`test:integration:binary32-primitive-boundary-oracle`)|scalar|Binary32 boundaries include ties, cancellation, subnormals and both finite limits.|
|(`test:integration:binary64-primitive-boundary-oracle`)|scalar|Binary64 boundaries exercise the same laws at its independent exponent range.|
|(`test:integration:endpoint-hulls-enclose`)|scalar|All four signs and zero crossings preserve inclusion in both formats.|
|(`test:integration:outward-neighbours-without-residual-shortcuts`)|scalar|The outward-neighbour rule applies at exact ordinary sums and underflow alike.|
|(`test:integration:fixed-reduction-rational-oracles`)|scalar|Cancellation, underflow and exact small answers succeed in each format.|
|(`test:integration:reduction-order-and-empty-laws`)|scalar|Increasing-index left folds refuse intermediate overflow instead of reassociating.|
|(`test:integration:scaled-norm-rational-oracle`)|scalar|Scaling contains norms of huge, tiny, mixed and empty data without direct squaring overflow.|
|(`test:integration:reduction-and-norm-refusals`)|scalar|Invalid input and unbounded norms are never converted into a finite certificate.|
