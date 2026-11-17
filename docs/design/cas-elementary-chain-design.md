<!-- SPDX-License-Identifier: CC0-1.0 -->

# Elementary chain rules · `spec:nrz:cas-elementary-chain-design`

**Requirement (Design standing and scope)** · `req:nrz:cas-elementary-chain-contract`

Propose exact first-order scalar chain rules for the closed Real exponential, natural logarithm, sigmoid and hyperbolic tangent family of (`dec:nrz:cas-trig-series-closed-real`), under the calculus meaning of (`constr:nrz:cas-differential-models`). This design advertises no executable capability and registers no primitive. The proposed schemas remain Requirements until their independent checkers, premise contracts, bounded production and portable replay are implemented together with their evidence. The full-box identities in (`[CAS-rule:cas:enclosure-derivative-grammar]`) motivate the same formulas; an enclosure certificate is not thereby an exact symbolic derivative source (`[CAS-rule:cas:enclosure-derivative-evidence]`). For positive Real operands `diff.log@1` would resolve the real-logarithm differentiation that (`alg:nrz:cas-design-differentiation`) leaves unresolved in the first slice; general expression-valued exponents and complex branches remain unresolved.

## Premise and domain contract · `sec:nrz:cas-elementary-chain-premises`

**Requirement (Use the semantic premise bridge)** · `req:nrz:cas-elementary-chain-bridge`

Depend on (`req:nrz:cas-trig-design-premise-bridge`) as (`req:nrz:cas-trig-design-diff-sin`) and (`req:nrz:cas-trig-design-diff-cos`) do. Require these schemas to consume bridge views identified by the registered `HypothesisKind` values DomainGuard, Differentiable, Derivative and StrictSign in `hypothesis_kinds.rs`, which the enclosure derivative schema already uses (`[CAS-thm:cas:enclosure-smooth-derivative]`); these schemas introduce no kind enum of their own. Require all four views and preserve their source-admission and closure contract. No context-supplied derivative expression is admitted.

For a subject s=f(u), declared Real coordinate x and exact domain U in context Γ, require the same original u, x, U, full Γ and scope in every applicable premise. The common ordered slots are DomainGuard(u,U), Differentiable(u,x,U), Derivative(u,x,v,U). Differentiable has order one. One checked child DerivativeOn application may feed both the second and third slots through their distinct semantic projections; this shares proof authority without inventing a regularity assumption. The first slot may use the bridge's checked derivative-definedness projection with every original guard retained. Logarithm appends StrictSign(u,Positive,U) as slot four. The registered StrictSign kind also includes negative signs, so kind matching alone is insufficient (`[CAS-assum:cas:hyp-strict-sign]`).

**Requirement (Real domains and definedness)** · `req:nrz:cas-elementary-chain-domains`

Use the existing unary `Exponential`, `NaturalLog`, `Sigmoid` and `Tanh` operators, their checked Real operand/result sorts and Real placements. An opaque Call is not one of these operators; a Complex-sorted operand refuses even if its value is real. No implicit retagging or Rational-to-Real placement is added (`dec:nrz:cas-trig-series-closed-real`), (`dec:nrz:cas-trig-design-operators`), (`def:nrz:cas-symbol-sorts-declarations`). The derivative domain follows the existing classical Real-coordinate contract, with all original guards; this design adds neither boundary derivatives nor a one-sided convention (`alg:nrz:cas-design-differentiation`).

Each chain checker must check its raw output's definedness from its ordered premises and the registered totality laws below; the subject's definedness comes from those laws, never from the displayed derivative formula. Exponential, sigmoid and tanh are smooth on every defined Real input. Logarithm is smooth only for positive Real input; its checked positive sign also supplies nonzero definedness for the reciprocal of that same u. Preserve the domain and all conditions of those premises rather than enlarging the domain or inferring positivity from the displayed formula. StrictSign must come from a source admitted by the bridge, including registered `real.sign` conclusions or checked strict predicates; NonNegative, NonZero, a numerical enclosure or a bare asserted sign is not a substitute (`[CAS-thm:cas:primitive-real-sign]`). An unavailable sign derivation leaves production Unresolved; this design requires no extension of `real.sign`. Without a separately supplied checked positive premise, log(exp(x)) and log(1+exp(x)) remain Unresolved: `real.sign` has no rule establishing positivity of exp(u) or sigmoid(u). Such sign rules are natural follow-up work, with no contract decided here.

**Requirement (Registered elementary definedness)** · `req:nrz:cas-elementary-chain-definedness`

Supply each subject's definedness through a registered law yielding DomainGuard for f(u), following (`req:nrz:cas-trig-design-definedness`) and (`[CAS-thm:cas:primitive-trig-defined]`), as `trig.defined@1` does for the inner sine of (`req:nrz:cas-trig-design-nested-derivative`). Propose T `exp.defined@1`, a closed schema restricted to Exponential, Sigmoid and Tanh, with the sole ordered premise DomainGuard(u,U). Propose T `log.defined@1` for NaturalLog(u), with ordered premises DomainGuard(u,U) and StrictSign(u,Positive,U). Read slot two through the bridge's StrictSign view, with the same admitted sources and positive-sign discriminator as slot four of `diff.log@1`. Each concludes the existing PredicateFact of Defined(f(u)) on U in the identical context and scope, preserving every premise hypothesis and residual. The checker reconstructs the closed operator and original operand, rejects other operators and mismatched frames, and accepts neither substitutions nor a certificate.

The scalar condition walker leaves these operators, like the trigonometric ones, as Defined obligations. Consuming a registered totality law at derivative endpoints is the integration the nested-sine fixture already requires of the bridge; these rules reuse it, including output equality transport. They do not grant a blanket definedness exemption or infer totality from an unchecked derivative. Ordinary coverage requests, which cite `trig.defined@1` for chains of closed trigonometric nodes, are not extended to these operators by this design.

## Four checked schemas · `sec:nrz:cas-elementary-chain-schemas`

**Requirement (Closed derivative judgments)** · `req:nrz:cas-elementary-chain-schema-contract`

Propose four class-T primitives, explicit mathematical chain theorems with independently checked premises, under (`def:nrz:cas-solving-design-registry`) and (`rule:nrz:cas-design-closed-registration`). Each concludes DerivativeOn(s,x,w,U) in the identical frame, with w reconstructed below. It does not conclude equality of s and w. No schema takes a certificate, substitution map, rewrite path or caller-defined function law; its ordered ordinary proof references provide the semantic premises. The proposed raw graph shapes deliberately precede simplification.

**Table (Elementary schemas and raw outputs)** · `tab:nrz:cas-elementary-chain-rules`

|Schema|Subject s|Ordered premises|Raw output w|
|------|---------|----------------|------------|
|`diff.exp@1`|Exponential(u)|DomainGuard, Differentiable, Derivative as specified above|Mul(s,v)|
|`diff.log@1`|NaturalLog(u)|The common three, then StrictSign(u,Positive,U)|Mul(Div(1,u),v)|
|`diff.sigmoid@1`|Sigmoid(u)|The common three|Mul(Mul(s,Sub(1,s)),v)|
|`diff.tanh@1`|Tanh(u)|The common three|Mul(Sub(1,Mul(s,s)),v)|

Every displayed 1 is an exact rational literal in the same Real frame. Products, subtraction and division are ordered binary graph constructors. The sigmoid formula uses the logistic sigmoid meaning of (`[CAS-rule:cas:enclosure-sigmoid-compose]`); the tanh formula follows (`[CAS-rule:cas:enclosure-derivative-grammar]`). Their raw outputs introduce no division, range premise or separate exponential expansion. The logarithm's raw reciprocal makes the operand-domain obligation explicit; v/u is a later checked equality transport, not an alternative unchecked shape.

**Requirement (Subject and operand identity in replay)** · `req:nrz:cas-elementary-chain-identity`

For exponential, compare the first operand of the raw output directly with the checked derivative subject s. For sigmoid, compare both operand references to s, in the leading factor and in Sub(1,s), with that subject. For tanh, compare both factors in the inner Mul(s,s) with that subject. The checker must not reconstruct f(u) and compare a spelling, a numerical value or an algebraically equivalent expression in place of these reference checks. For logarithm, compare the denominator of Div(1,u) directly with the subject's original operand edge zero, and compare the positive-sign subject with that same u; using s itself as denominator is invalid. In every row the final v is exactly the child derivative's output, with its coordinate and complete frame checked independently.

Identity is mathematical graph identity under (`def:nrz:cas-design-interning`), not source-occurrence identity or a serialized live handle. An identical complete interning key may return s and is valid because it is the same node; construction history is not checked. Changed declarations, operands, scopes, semantic headers or placements must not pass as lookalikes. Preserve distinct original occurrences and complete walks (`rule:nrz:cas-design-occurrences`). Fresh replay checks the same sharing in the destination graph against the exact expected derivative judgment (`rule:nrz:cas-design-expected-claim`), through the bridge's ordinary dependency closure.

**Requirement (Production, simplification and closure)** · `req:nrz:cas-elementary-chain-production`

Extend the executable differentiation slice of (`dec:nrz:cas-trig-design-differentiation`) by exactly these four chain schemas, only after the bridge is executable; operators outside the slice so extended keep their pending typed derivative requests (`alg:nrz:cas-design-differentiation`). Reuse the existing postorder production and memoization and the bridge's semantic projections, closure retention, domain restriction and strict equality conversion. Gather the registered totality applications alongside child derivatives. Missing views or unsupported child derivatives leave the containing request Unresolved. In particular exp(1/x) retains x≠0 even if simplification hides the reciprocal.

Check the raw output and direct-reference obligations before simplification and derivative transport under (`alg:nrz:cas-design-differentiation`). Zero child derivatives do not remove operand definedness or logarithm positivity. No new simplifier or transcendental equality law is proposed; if simplification is unavailable, retain the checked raw output.

## Worked acceptance cases · `sec:nrz:cas-elementary-chain-examples`

**Example (Declared-coordinate exponential)** · `ex:nrz:cas-elementary-chain-coordinate`

Let σ be the declared Real differentiation coordinate, and let ω and d be Real declarations held constant with respect to σ, with d>0 retained in the context. Set a=Div(ω,d), u=Mul(a,σ) and p=Exponential(u). The existing variable, quotient and product rules establish D_σ a=0, since ω and d are other declared variables with derivative zero, and D_σ u=a after checked rational simplification (`alg:nrz:cas-design-differentiation`); the original d≠0 guard remains justified by the retained positive premise. Feed that checked derivative to both regularity and derivative slots, and its original definedness to DomainGuard. Propose `diff.exp@1` to establish D_σ p=Mul(p,a), with p the subject's own node. Checked scalar commutativity and derivative transport give D_σ p=(ω/d)p on the same domain and under exactly the retained assumptions. No gradient of an unknown σ is concluded.

Require fresh replay against independently constructed expected raw and transported claims. A concrete control with ω and d replaced by the literals 2 and 3 keeps a=Div(2,3), uses checked child-output transport to a, has no hypothesis or residual and expects the raw D_σ p=Mul(p,Div(2,3)); transport to the literal 2/3 is a separate checked equality. The symbolic d>0 control remains qualified and must not satisfy an unconditional demand. A nonzero coordinate-dependent operand and a sign-flipped coefficient distinguish the formulas. Replacing d by the literal 0 refutes the original d≠0 obligation before cancellation, including when ω is 0, and preserves that domain reason. Replacing σ by an opaque unknown function without a checked derivative source leaves production Unresolved.

**Example (Family and nested controls)** · `ex:nrz:cas-elementary-chain-family`

Require symbolic expected graphs for all four rules with a nonconstant polynomial u and nonzero v, constructed independently of production. For log(x), supply a checked positive premise on the exact positive domain and retain its closure; pair it with nonnegative-only, nonzero-only, negative and missing-sign controls. For exp(exp(x)), the checked inner derivative supplies both outer regularity and derivative views, and `exp.defined@1` supplies the inner exponential's DomainGuard; the raw output must share each level's own subject, never swap inner and outer exponentials. For exp(log(x)) with a checked positive premise for x, `log.defined@1` supplies the outer DomainGuard from DomainGuard(x) and that premise. Use sigmoid(1/x) and tanh(1/x) to retain the original reciprocal guard through repeated subject references. A checked sign premise about another operand cannot discharge a logarithm guard even when both operands share a display name.

## Verification and outcomes · `sec:nrz:cas-elementary-chain-verification`

**Requirement (Four test layers and independent oracles)** · `req:nrz:cas-elementary-chain-tests`

Follow (`req:nrz:cas-trig-design-test-layers`): place each assertion in the outermost existing test layer able to express it, with conformance and integration modules beneath the single public test target. Reuse existing shared setup in `src/testing/`; inline tests use local setup. Public modules live in `tests/conformance/elementary_chain.rs` and `tests/integration/elementary_chain.rs` beneath `tests/public.rs`, crate tests in `src/tests/elementary_chain.rs`; preserve location-derived test inventory labels. Supply the following evidence for all six schemas, including direct supplied checking and fresh expected-claim replay.

**Table (Evidence by location)** · `tab:nrz:cas-elementary-chain-test-locations`

|Layer|Required evidence|
|-----|-----------------|
|Inline private unit|Exact ordered shape reconstruction, positive-sign discriminator, slot arities and malformed private records.|
|Crate tests in `src/tests/`|Subject sharing and original occurrences, semantic projections, endpoint definedness integration, strict scalar/scoped conversion and atomic rollback snapshots.|
|Public conformance in `tests/conformance/`|All four formulas and both definedness laws with their accepted-control mutations; Real admission; conditional versus discharged results; schema/version and resource boundaries.|
|Public integration in `tests/integration/`|Declared-coordinate fixture, nested exponential, retained inner guards, complete portable dependency closures, cold production, warm authenticated reuse and fresh-store replay.|

Build expected graph terms, premise frames and hypothesis/residual sets independently of the producer's formula builder, simplifier and cache. Hand-derived symbolic formulas are the primary oracle; numerical samples cannot prove a chain law. Pin hand-authored portable fixtures for new schema references and shared subjects, retaining an unchanged accepted witness beside each mutation. Preserve existing enclosure and trigonometric fixtures without treating them as implementations of these schemas.

**Table (Mutation controls)** · `tab:nrz:cas-elementary-chain-mutations`

|Mutation or missing capability|Required result|
|------------------------------|---------------|
|Remove a regularity slot or substitute another operand's child derivative|Invalid supplied premise join; producer with no checked source stays Unresolved.|
|Change coordinate, domain, full context, hypothesis slot or residual closure|Reject replay or fail exact expected-claim comparison; no weakening by syntax sharing.|
|Replace one subject reference in exp, sigmoid or tanh by a different lookalike|Reject the direct subject-reference check, including when only one of the two references is replaced.|
|Replace log's reciprocal denominator or positive-sign subject|Reject exact original-operand identity; retain a passing unchanged control.|
|Flip v's sign, change exact one, replace subtraction by addition or drop a factor|Reject reconstructed output on a discriminating nonzero control.|
|Supply NonNegative, NonZero or a negative StrictSign to `diff.log@1` or `log.defined@1`|Reject the positive slot; no positivity inferred from a function name.|
|Use an equal-spelled opaque call or a Complex operand|Opaque syntax gains no rule; invalid Real admission refuses before chain checking.|
|Use `exp.defined` for log, omit log positivity, or change a totality law's original operand/domain|Reject the closed discriminator or premise join; retain an unchanged accepted totality control.|
|Produce without any required bridge view|Unresolved whole derivative, no partial theorem publication.|
|Convert a derivative, sign or differentiability view to equality|Strict equality conversion refuses.|
|Use one checked child derivative in both regularity and derivative slots|Accept with the same closure; changing either slot's frame refuses.|
|Strip x≠0 from a reciprocal child, or hide an undefined child beneath zero|Reject lost domain evidence before publication, even if the final formula looks total.|
|Reuse a conditional result without its context assumption|Reject reuse or expected claim; never satisfy an unconditional demand.|
|Change a schema version or portable dependency reference|Unknown version is Unsupported; malformed or changed known references are invalid.|

**Table (Stopping outcomes)** · `tab:nrz:cas-elementary-chain-outcomes`

|Outcome category|Boundary|
|----------------|--------|
|Unresolved|Valid request lacks a checked child derivative, bridge conversion or positive-log premise; retain exact pending obligations.|
|Unsupported|Unavailable schema version or an operation outside the delivered fragment; no newest-version substitution.|
|Invalid|Malformed Real node, false supplied premise join, altered raw shape or forged expected-claim witness.|
|Refused|A valid request cannot meet its demanded unconditional or whole-domain contract; preserve a proved domain obstruction such as log at a nonpositive input.|
|Exhausted|Insufficient charged resource allowance; retain spent work and publish no partial claim.|

These are outcome categories, not new interchangeable API variants; retain underlying faults and the distinction between unavailable evidence and a refuted domain (`def:nrz:engineering-refusal-meaning`), (`tab:nrz:cas-trig-design-outcomes`). Output-envelope exhaustion after successful checking preserves the existing optional NotExported outcome.

**Requirement (Measured complete-operation boundaries)** · `req:nrz:cas-elementary-chain-work`

Apply (`req:nrz:cas-trig-design-test-work`) to complete production, supplied checking, transport, retained reuse and fresh expected-claim replay, including both totality laws and all four chain rules. Measure cold and warm operations separately and pin successful Work N and refusal at N−1 with cumulative spend and simultaneously retained premise closures. Preserve the cited resource, rollback and publication obligations; include six-entry registry and policy growth when recalibrating affected literals. No numerical costs, new limits or ceiling increases are proposed.

## Registration and landing · `sec:nrz:cas-elementary-chain-landing`

**Requirement (Atomic registration and portable compatibility)** · `req:nrz:cas-elementary-chain-registration`

Extend `rule.rs` and `formal_register.rs` with six class-T keys, Derivative or Predicate conclusion kinds and the specified ordered premises. These schemas introduce no new operator, statement or certificate payload. Reuse existing operator encodings; unlike (`dec:nrz:cas-trig-design-wire`), no operator tags need reservation. Register the chain rules' premise shapes against the implemented bridge, including the logarithm's four-slot contract, and verify their portable representation under (`req:nrz:cas-trig-design-premise-bridge`) rather than allocate numeric view discriminators here; the totality laws retain ordinary ordered proof references, one for `exp.defined@1` and two for `log.defined@1`. Like `trig.defined@1`, `exp.defined@1` reads an original definedness guard; `log.defined@1` reads its second slot through the bridge's StrictSign view, using the same sources and positive-sign discriminator as slot four of `diff.log@1`. Preserve old meanings and canonical fixtures, and pin new-schema golden bytes. Dispatch, independent checking and fresh replay land atomically under (`rule:nrz:cas-design-closed-registration`).

Update the complete positive weight table and logical policy version/minimum in `cost.rs`, schema ordering and all index-based lookups, manifest validation and policy/reuse fixtures. Choose the new weights explicitly in the implementing milestone under (`req:nrz:cas-trig-design-register`), (`dec:nrz:cas-design-search-policy`); this design allocates no policy version or weights ahead of the prerequisite bridge. Relative to the implemented bridge inventory, exactly six primitive entries are proposed: four chain rules and two totality laws; registered hypothesis kinds do not grow. Derive final counts and every moved Work/byte literal from that tree rather than stale documentary counts.

Extend the existing `crates/cas/src/` owners: `calculus.rs` for production, `derivative_check.rs` for derivative dispatch, `proof_check.rs` and `conditions.rs` for the bridge's totality integration, and `proof_view.rs`, `scoped_check.rs`, `scalar_model.rs` and `proof_model.rs` for semantic premises. Place the four chain-rule and two totality checkers in a new private `exp_log.rs` module with implementation beneath `exp_log/`, alongside `trig.rs` and `trig/`; follow `trig/defined.rs` for totality checking. Registration and replay extend `rule.rs`, `rule_check.rs`, `formal_register.rs`, `cost.rs`, `encoding.rs`, `wire.rs`, `wire_read.rs` and `wire_statement.rs` as the bridge representation requires. The CAS guide describes only landed capabilities; this design's template, render and `data/documents.toml` entry form its documentation unit.

The implementation prerequisites are the semantic bridge (`req:nrz:cas-trig-design-premise-bridge`), `diff.sin@1` (`req:nrz:cas-trig-design-diff-sin`), `diff.cos@1` (`req:nrz:cas-trig-design-diff-cos`) and the discharged nested-sine fixture (`req:nrz:cas-trig-design-nested-derivative`), without arctangent's sign extension. `log.defined@1` lands after the bridge because its second premise consumes the StrictSign view; its positive-sign contract matches slot four of `diff.log@1`. Then each exposed elementary schema must include its registered definedness dependencies, producer, independent replay, mutation controls and resource evidence. The four-rule design is one closed-family contract; implementation order within it need not be a commit-count rule.

**Caveat (Separate owners and deferred work)** · `cav:nrz:cas-elementary-chain-nongoals`

The bridge and its sine/cosine nested fixture remain owned by (`req:nrz:cas-trig-design-premise-bridge`), (`req:nrz:cas-trig-design-diff-sin`), (`req:nrz:cas-trig-design-diff-cos`) and (`req:nrz:cas-trig-design-nested-derivative`); they are prerequisites, not redesigned here. Arctangent and its sign-producer/projection extension are outside this design; their owner is (`req:nrz:cas-trig-design-diff-atan`). Unknown differentiable function sources remain a separate design question under (`constr:nrz:cas-differential-models`) and (`tab:nrz:cas-design-primitives`); no context-assumed Derivative source or new opaque-function schema is assumed here. Consumer physics remains outside the CAS under (`cav:nrz:cas-trig-series-exclusions`), and dimensionless-exponent validation belongs to (`req:vision:algebraic-unit-checks`). General powers, additional elementary identities, new sign rules, boundary calculus, integration and analytic source lifting are deferred to their respective calculus, sign or series designs. None is inferred from the four formulas.

## Decisions · `sec:nrz:cas-elementary-chain-decisions`

**Decision (One key per function)** · `dec:nrz:cas-elementary-chain-keys`

Choose separate version-one keys for the four chain rules rather than an operator-discriminated derivative schema, as (`req:nrz:cas-trig-design-diff-sin`) and (`req:nrz:cas-trig-design-diff-cos`) do: each premise contract and unsupported version stays explicit while the family forms one contract.

**Decision (Raw shapes and reused nodes)** · `dec:nrz:cas-elementary-chain-shapes`

Choose (`tab:nrz:cas-elementary-chain-rules`) and (`req:nrz:cas-elementary-chain-identity`) over recomputed or exponential-expanded outputs. For logarithm choose Mul(Div(1,u),v) over Div(v,u), mirroring the enclosure reciprocal factor (`[CAS-rule:cas:enclosure-derivative-grammar]`) and exposing the operand's nonzero obligation; equivalent outputs require checked transport.

**Decision (Separate registered totality laws)** · `dec:nrz:cas-elementary-chain-totality`

Choose the two laws of (`req:nrz:cas-elementary-chain-definedness`) over definedness inferred inside chain checkers or a blanket condition-walker exemption. This follows the registered `trig.defined` precedent and keeps domain evidence independently replayable. Group the three everywhere-defined Real functions under one closed discriminator; give logarithm its own key because its positive-sign premise differs. A single four-operator totality key would instead need operator-dependent premise arity.
