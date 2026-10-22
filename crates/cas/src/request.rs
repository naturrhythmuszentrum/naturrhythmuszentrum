// SPDX-License-Identifier: AGPL-3.0-only
//! Public request vocabulary and checked service contracts.
//!
//! # Requests and retained limits
//!
//! The root exports `Cas`, handles/references, `Rational` and structured faults. Construction lives in `syntax`, domains/headers/contexts in `domain`, occurrence coverage in `source`, tasks/results/search in `request`, judgments/witnesses in `proof`, and accounting in `limits`. A `Request` pairs each occurrence root with its matching receipt in `CoveredInput` and retains explicit witness premises. The retained `Draft::Request` boxes its `Limits` payload to bound draft size; the executable `Request` retains value-owned limits.
//!
//! # Bounded search, policy and outcomes
//!
//! `Equal` and `Simplify` share two ordered queues, positive cost/progress quotas and a resumable finite rule/position cursor. The default epoch takes four cost visits and one progress visit. Matching, copying, container comparisons, arithmetic and conditions consume monotone ledgers. Structural expansion, exact coefficient collection and monomial cancellation propose ordinary proof steps; the kernel reconstructs every admitted edge. Existing proofs and policy-compatible shortcuts are expanded to their older primitive dependencies before reuse. The difference-quotient fixture requires an explicit larger intermediate before collection reaches its target, retaining the increment's nonzero guard. `Expand` materializes the bounded polynomial coefficient expression and checks the resulting finite-jet edge.
//!
//! Each label retains cost, peak unfolded size, cumulative positive expansion credits, proof depth, witness footprint and proof/domain support. Dominance requires all resource dimensions to improve without increasing support, and never crosses context or residual classes. Exact ties use canonical proof bytes. The best result stays within the initial residual class; a policy requiring discharge cannot hide an initial unknown guard. An equality target must be reached exactly before a successful result is returned. An incomplete frontier or refused candidate never proves that no other route exists. These are heuristic routes, with no global shortest-path claim.
//!
//! `run_with_policy` accepts a `SearchPolicy` containing a complete positive `CostPolicy` table and conditional-result permission. Every immutable table is stored in the graph with its key, version and full primitive manifest. Reusing an identity/version with different contents is invalid. Default weights use `logical` version 10 with the unchanged 59 weights and the 21 current solving weights, sorted by ASCII key. Logical versions below 10 and modified logical v10 tables are rejected; custom policies use their own identity/version; custom weights are at most 1,000,000. Logical transition cost and expanded primitive count are reported separately. `certify_frozen_cost` projects only checked equality applications in the exact context/residual class, pins its manifest and table, and settles every reachable endpoint by bounded positive-weight Dijkstra. Its sealed certificate states a shortest cost or finite-snapshot unreachability. New applications affect only a new manifest; exhaustion yields no certificate. Raw operands, source links and proof-dependency links are never cost edges.
//!
//! A request uses distinct generation and checker work ledgers; every candidate check shares the same checker allowance. A best result additionally undergoes fresh-store canonical byte replay before return. Higher derivative levels replay the necessary proof roots, whose checked dependency closures also certify their retained subproofs. An unrelated nonexistence fact needs its own replayed root; all replay work shares the original request allowance. Each ledger memoizes successful exact region partitions by complete portable syntax, ordered roots, the optional boundary cut and active ceilings. Fresh receiving stores can reuse those decisions; every supplied certificate and asserted relation is still checked. The cache reserves staged payload, and key construction, lookup and copying remain metered. `SearchResult` reports both usages, its exact endpoint trace and full policy. Its optional `NRZRUN04` envelope separates costs, effective limits, counters and trace from the authoritative `NRZCAS04` witnesses, including every retained calculus component. If the optional response cannot fit, `response` is absent (`NotExported`) while the sealed in-store witness remains available for a later metered export. Minimal outcome metadata does not require optional output capacity.
//!
//! `Cas::run` executes bounded `Equal`, `Simplify`, `Expand`, `Differentiate`, `Jacobian`, `SquareFree`, `ElaborateCases`, `Select`, `Eliminate`, `Merge`, `Coverage`, `Integrate`, `Sum`, `Limit`, `Series`, `EmbedRegular`, `Pair`, polynomial `SmoothMultiply` and boundary ownership, jet, jump and explicitly ordered point derivative requests after validating complete matching occurrence receipts. `Proved` and `Conditional` contain independently checked results; the latter retains unresolved domain guards. Contradicted definedness and a proved empty requested function domain return `Refused` with `RefutedDomain` or `EmptyDomain`. Native unsupported premises return `PendingPremise`, and undecided coverage returns `UnknownTotality`; both carry any independently replayed partial facts. A checked point obstruction returns `Nonexistent`. Mixed whole-domain derivative results return `Partial` with a sealed `DerivativeMap`. Other represented tasks remain `Unresolved`, and excluded executable fields return `Unsupported`. Malformed inputs return `Invalid`; spent limits return `Exhausted` with only a previously replayed best result when available, stored in `Option<Box<SearchResult>>`. `canonical_snapshot` is a deterministic graph diagnostic, not a mathematical witness or import format.

pub use crate::cost::CostPolicy;
pub use crate::frozen::FrozenCertificate;
pub use crate::request_model::{
    ContextConsistency, CoveredInput, Outcome, RefusalReason, Request, UnsupportedReason,
};
pub use crate::search::{SearchPolicy, SearchResult, SearchStatistics};
pub use crate::task::{ActionMode, BoundaryFact, Task};

pub use crate::derivative_model::{BoundaryFacts, DerivativeMap, PendingCell, ResultPayload};

pub use crate::split_model::SplitRange;

pub use crate::germ_model::{GermAt, SeriesAt};

pub use crate::pullback_model::PullbackKind;

pub use crate::solving::{
    ExclusionPolicy, SolveAnswer, SolveBlocker, SolveForm, SolveGoal, SolveReason, SolveSpec,
};
