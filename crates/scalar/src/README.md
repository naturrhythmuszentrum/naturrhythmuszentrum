<!-- SPDX-License-Identifier: CC0-1.0 -->

# Scalar src evidence · `guide:scalar:src-evidence`

**Rule (Sealed format identities)** · `rule:scalar:format-identities`

A sealed scalar implementation identifies its own primitive representation; widening, narrowing and arithmetic-context admission must retain that distinction rather than accepting a caller-selected tag.

**Rule (Exact finite identities)** · `rule:scalar:finite-identities`

The scalar and finite wrappers preserve canonical positive zero as the additive identity and exact unity as the multiplicative identity; only proved identities authorize the arithmetic shortcuts that retain singleton enclosures.

## Unit test matrix · `tab:scalar:unit-test-matrix`

**Table (Unit test matrix)**

|Test|Area|Claim|
|----|----|-----|
|(`test:unit:environment-admission-refuses`)|scalar|Private admission cannot bypass either target or environment refusal.|
|(`test:unit:prepared-exp-validation-order`)|scalar|Operand checks keep their typed order, including zero and subnormal nominal interiors. # Panics Panics if shared validation changes a refusal or a zero bypasses admission.|
|(`test:unit:prepared-exp-basis-readings`)|scalar|Structural admission alone does not establish monotonicity or finite nominal exports. # Panics Panics if operand admission or composition changes an arithmetic refusal.|
|(`test:unit:prepared-exp-competing-failures`)|scalar|Basis inversion takes precedence over a competing nonfinite composition. # Panics Panics if composition runs before the complete basis reading checks.|
|(`test:unit:prepared-exp-final-target`)|scalar|Final target checks apply to exp's addition, independently of auxiliary bits. A conditional degree-one program is enough to expose addition spacing: near one the checked addition has nonzero width even when expm1 is tiny. # Panics Panics if exact zero bypasses validation or final addition loses outward rounding.|
|(`test:unit:prepared-expm1-program-admission`)|scalar|Structural admission, coefficient capacity and environment checks precede any zero result. # Panics Panics if an invalid inventory or environment reaches native evaluation.|
|(`test:unit:prepared-sigmoid-admission`)|scalar|Original admission precedes transformation and exact zero; correspondence binds the domain and policy. # Panics Panics if a typed admission changes or a nonzero operand becomes an exact singleton.|
|(`test:unit:prepared-sigmoid-basis-failure`)|scalar|Basis validation preserves its cause and actual transformed numerical request. # Panics Panics if a conditional basis failure is reported as final-target impossibility.|
|(`test:unit:prepared-sigmoid-positive-denominator`)|scalar|Conditional malformed images require a nonnegative exponential and a strictly positive denominator. # Panics Panics if a negative denominator reaches division or is confused with final target failure.|
