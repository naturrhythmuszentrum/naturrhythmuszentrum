// SPDX-License-Identifier: AGPL-3.0-only
//! Public limits vocabulary and checked service contracts.
//!
//! # Solving ceilings and cumulative ledgers
//!
//! The 21 solving ceilings have matching `BudgetKind`, `LimitsOverride` and `BudgetUsage` fields. `Budget::solving` fixes each resource's cumulative or peak counting rule and retains attempted usage on refusal. Admission checks denominator factors before deduplication, staged maps/branches, tower depth and degree products, family dimensions and selection slots. `Limits::blind_cubic()` constructs exactly the finite profile pinned in the solving design's blind-cubic overrides table; it changes no other defaults and never retries with higher ceilings. Store and request limits must both admit the explicit profile.
//!
//! All six symbolic boundary fixtures run together under that unchanged profile. Each original request shares one cumulative generation ledger across its full branch tree and failed attempts, and one checker ledger across coverage and presentation; independent requests do not share a summed ledger. Portable complete-answer acceptance uses `Cas::replay_answer` with a separate cumulative receiving ledger. Standalone `replay` remains available for individual claims; replaying both standalone witnesses repeats their shared closure. Existing exact resource boundaries remain unchanged.
//!
//! `Limits` names every storage, depth, bit, work and future algorithm ceiling; `LimitsOverride` resolves optional request changes. A `Budget` is an explicit monotone ledger. Every active limit, including work and replay work, is the componentwise minimum of the requested/default allowance and the permanent store ceiling; a request cannot raise that ceiling. Transactions roll back failed graph edits. Conservative estimates may refuse before the nominal vertex or bit cap is reached. Allocator failure is returned where the underlying API offers fallible reservation; environmental allocator abort behavior has the limitation stated in `docs/design/cas-design.md`.

pub use crate::budget::{Budget, BudgetKind, BudgetUsage, Limits, LimitsOverride};
