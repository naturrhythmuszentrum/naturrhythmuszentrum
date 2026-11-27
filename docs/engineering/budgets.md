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

Output atomicity is independent of attempt accounting: an admitted failed pi or coefficient attempt retains its consumed Work and steps. Accuracy has caller-supplied attempt and work allowances. Reservations charge both counters atomically before work; failed reservations preserve the prior totals, while admitted attempts retain their spend across arithmetic failure and permitted precision retry (`[ACCURACY-claim:accuracy:work-reservations-are-atomic]`), (`[ACCURACY-claim:accuracy:zero-and-integer-work-limits]`), (`[ACCURACY-claim:accuracy:recovery-resource-stops]`), (`[ACCURACY-claim:accuracy:precision-retry-cannot-reset-limits]`).

**Definition (Attempt spend, staging and retained output)** · `def:nrz:engineering-attempt-resources`

Consumed attempt Work and steps are monotone counters: admission prepays each bounded schedule, including schedules that later refuse. A refused reservation consumes nothing, but it cannot refund earlier admitted schedules. Staging is a simultaneous capacity reservation for scratch and a candidate result; releasing it on return does not reduce consumed Work. Retained output is published ownership, admitted against its own byte policy and committed only with the complete result or cache entry. An output rollback releases the candidate, not its computation. A staging high-water statistic is historical evidence, not live occupancy or a credit for subsequent work.

**Rule (Nested attempt reconciliation)** · `rule:nrz:engineering-attempt-reconciliation`

A child ledger is bounded by the original parent's remaining Work before execution. Every child outcome exposes consumed Work, including arithmetic, unsupported-factor and late precision refusals. The owner reconciles that receipt exactly once before propagating either success or failure; it cannot use a success value as the only usage receipt. No other parent debit may intervene while a bounded child is running, so receipt reconciliation fits the already admitted remaining allowance. Scalar pi Work includes initialization, endpoint quantization, displacement and paired refinement; matrix adds one prepaid reciprocal schedule. Exact recognition and spelling Work, literal scan/decode Work, coefficient multiplication/displacement and cache lookup each debit the same original matrix caller once. Local scalar refinement and exact arithmetic step ceilings remain additional bounds; their schedules are already represented in parent Work and are not charged a second time as Work. Byte checks and publication remain separate from these receipts.

**Rule (One recovery ledger)** · `rule:nrz:engineering-parent-retries`

A refinement or recovery policy reserves a finite attempt count before retrying, retains one parent Work ledger across every outcome and precision choice, and derives every child allowance from that parent's remaining balance. It must not replace the parent with a fresh default budget or count only successful children. The scalar pi loop additionally reserves one paired step and its Work together before touching either series. Zero-work preflight refusals may remain uncharged; a finite attempt policy also bounds repetitions of those refusals. There is no automatic retry in pi owner or coefficient construction. These obligations apply to future LC recovery scheduling as well as existing caller-owned accuracy attempts (`arg:nrz:engineering-recovery-budget`).

**Definition (Boundary interpretation)** · `def:nrz:engineering-budget-boundary`

An exact boundary names its complete operation, successful ceiling and immediately lower refusal. A first refused charge establishes only its attempted need, not the eventual cost of successful completion; source-derived pins and dated measurements therefore remain different evidence classes (`rule:vision:local-and-observable-budgets`).

## Open intents · `sec:nrz:engineering-budget-intents`

**Claim (Remaining owner policies and boundaries)** · `claim:nrz:engineering-remaining-budgets`

The remaining owner documentation will register the policy ceilings and exact boundary claims before their values enter this record; cumulative LC budget evidence belongs to symbolic lowering and numerical acceptance (`step:nrz:lc-resonance-symbolic-lowering`), (`step:nrz:lc-resonance-numerical-acceptance`).

**Claim (Shared physical symbolic allowance)** · `claim:nrz:engineering-shared-symbolic-budget`

Symbolic lowering will carry the caller's cumulative policy through original physical checking, retained source Jacobians and compatible LC consumers without resetting defaults or replacing the original model by a cheaper fixture (`step:nrz:lc-resonance-symbolic-lowering`).

**Claim (Whole-plan resource acceptance)** · `claim:nrz:engineering-plan-resource-budget`

The physics increment will retain aggregate node/retry work and shared invariant storage; the accuracy-and-benchmarks increment will compare observable error and cost under declared policies (`step:nrz:lc-resonance-physics-increment`), (`step:nrz:increment-accuracy-and-benchmarks`).
