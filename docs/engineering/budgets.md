<!-- SPDX-License-Identifier: CC0-1.0 -->

# Resource policies and boundary evidence · `spec:nrz:engineering-budgets`

**Definition (Ceiling and cost)** · `def:nrz:engineering-budget-meaning`

A configured ceiling is finite admission policy, not a measured successful cost or a completeness guarantee. Digits, decimal exponents, symbolic work, attempt reservations and numerical contraction work have different charging rules; their numeric values do not define a common resource.

**Definition (Literal admission ceilings)** · `def:nrz:engineering-literal-ceilings`

The default significand-digit ceiling is (`[SCALAR-const:scalar:default-literal-digits-count-1024]`), the absolute decimal-exponent ceiling is (`[SCALAR-const:scalar:default-literal-exponent-count-4096]`), and the complete literal-work ceiling is (`[SCALAR-const:scalar:default-literal-work-count-100000]`). These pins name the actual default policy; explicit callers can choose other finite allowances.

**Definition (Literal scan charge)** · `def:nrz:engineering-literal-scan-charge`

The input-byte charge is (`[SCALAR-const:scalar:literal-scan-work-per-byte-count-8]`); it prepays bounded grammar and candidate scans before enclosure operations consume the remaining allowance. Digit, exponent and work boundary tests preserve their separate success and refusal conditions (`[SCALAR-claim:scalar:literal-resource-boundaries]`).

**Definition (Schema dependency depth)** · `def:nrz:engineering-schema-depth`

Declaration, descriptor and kind recursion use the shared active-stack ceiling (`[DOCGEN-const:docgen:declaration-depth-limit-count-128]`) and reject cycles before further descent. This bounds schema admission rather than the mathematical size of a physical problem (`[DOCGEN-claim:docgen:checks-rational-policy-and-forward-reference-cycles]`).

**Argument (Caller-owned recovery budgets)** · `arg:nrz:engineering-recovery-budget`

Accuracy has caller-supplied attempt and work allowances. Reservations charge both counters atomically before work; failed reservations preserve the prior totals, while admitted attempts retain their spend across arithmetic failure and permitted precision retry (`[ACCURACY-claim:accuracy:work-reservations-are-atomic]`), (`[ACCURACY-claim:accuracy:zero-and-integer-work-limits]`), (`[ACCURACY-claim:accuracy:recovery-resource-stops]`), (`[ACCURACY-claim:accuracy:precision-retry-cannot-reset-limits]`).

**Definition (Boundary interpretation)** · `def:nrz:engineering-budget-boundary`

An exact boundary names its complete operation, successful ceiling and immediately lower refusal. A first refused charge establishes only its attempted need, not the eventual cost of successful completion; source-derived pins and dated measurements therefore remain different evidence classes (`rule:vision:local-and-observable-budgets`).

## Open intents · `sec:nrz:engineering-budget-intents`

**Claim (Remaining owner policies and boundaries)** · `claim:nrz:engineering-later-wave-budgets`

Later documentation waves will register the bindings, matrix and CAS policy ceilings and exact boundary claims before their values enter this record; cumulative LC budget evidence belongs to symbolic lowering and numerical acceptance (`step:nrz:lc-resonance-symbolic-lowering`), (`step:nrz:lc-resonance-numerical-acceptance`).

**Claim (Shared physical symbolic allowance)** · `claim:nrz:engineering-shared-symbolic-budget`

Symbolic lowering will carry the caller's cumulative policy through original physical checking, retained source Jacobians and compatible LC consumers without resetting defaults or replacing the original model by a cheaper fixture (`step:nrz:lc-resonance-symbolic-lowering`).

**Claim (Whole-plan resource acceptance)** · `claim:nrz:engineering-plan-resource-budget`

The physics bite will retain aggregate node/retry work and shared invariant storage; the accuracy-and-benchmarks bite will compare observable error and cost under declared policies (`step:nrz:lc-resonance-physics-bite`), (`step:nrz:bite-accuracy-and-benchmarks`).
