// SPDX-License-Identifier: AGPL-3.0-only
//! Conditional positive-root family law with durable receipts and independent portable replay.
mod check;
mod claim;
mod service;
mod wire;
pub use service::{Certificate, Receipt};
#[cfg(test)]
mod tests;

use crate::budget::{Budget, BudgetKind};
use crate::{Cas, Fault, Handle, InvalidCode, Location};

/// The local accounting boundary, not a native execution phase.
const AT: Location = Location::new("LC root family checker");
/// Closed private recipe identity; not a primitive manifest entry.
const RECIPE: &str = "lc-positive-root-composition";

/// Complete unsimplified original request; all constraints remain conditional premises.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    /// Original context, including its admissible region and conditional fibres.
    pub context: Handle,
    /// Original ordered inputs, including unused and repeated positions.
    pub inputs: Vec<Handle>,
    /// Original ordered Boolean constraints, including repeats.
    pub constraints: Vec<Handle>,
    /// Original L and C positions, without commutative normalization.
    pub positions: [usize; 2],
    /// `P_lower`, `P_upper`, `t_lower`, `t_upper`, `a_lower` and `a_upper`.
    pub endpoints: [Handle; 6],
    /// Original ordered product, strict half-power, reciprocal and tau*tau-P.
    pub targets: [Handle; 4],
    /// Exact arithmetic interpretation; only revision one is defined.
    pub arithmetic: u32,
}

/// Formal terms of the closed implication, resolved only against its original request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Term {
    /// Exact rational zero.
    Zero,
    /// Original selected L input.
    L,
    /// Original selected C input.
    C,
    /// Original product.
    Product,
    /// Original strict-real positive root.
    Root,
    /// Original reciprocal of that same root.
    Reciprocal,
    /// Original reverse polynomial, never the root target.
    Reverse,
    /// One of the six quantified endpoint parameters.
    Endpoint(u8),
    /// Square of a root endpoint parameter.
    Square(u8),
    /// Reciprocal of a root endpoint parameter; positivity of the upper endpoint
    /// follows from the preceding product and squared-root premises.
    Inverse(u8),
}

/// Explicit strictness is part of the mathematical claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Relation {
    /// Strict increasing order.
    Less,
    /// Nonstrict increasing order.
    AtMost,
    /// Exact equality.
    Equal,
}

/// An ordered relation in the universally quantified implication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Predicate(Term, Relation, Term);

/// Independent native realization obligations, preserved rather than proved here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Native {
    /// The native Product result encloses its original exact target.
    ProductTotal,
    /// The native root result encloses its original exact target.
    RootTotal,
    /// The native reciprocal result encloses its original exact target.
    ReciprocalTotal,
    /// Only the final reciprocal operation's local error is interpreted.
    ReciprocalLocalOperation,
}

/// Fixed tags identifying the reviewed ordered-field argument of the closed schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Step {
    /// Positive factors give a positive product.
    ProductPositive,
    /// The original strict half-power selects the unique positive root.
    RootUnique,
    /// Squaring preserves order on the nonnegative half-line.
    RootBracket,
    /// Reciprocal reverses order on strictly positive arguments.
    ReciprocalOrder,
    /// The supplied outer endpoints contain the reciprocal interval.
    ReciprocalBracket,
}

/// An untrusted local proposal, deliberately distinct from every existing receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Candidate {
    /// Closed schema name, checked independently of all other fields.
    recipe: &'static str,
    /// Mathematical revision; unknown versions refuse.
    version: u32,
    /// Complete original graph references and ordered constraints.
    pub(crate) original: Request,
    /// Universally quantified distinct original inputs followed by endpoint parameters.
    universal: Vec<Handle>,
    /// Exact ordered conditional mathematical premises.
    premises: [Predicate; 11],
    /// Native enclosure and local-error obligations remain antecedents.
    native: [Native; 4],
    /// Exact ordered conditional conclusions.
    conclusions: [Predicate; 7],
    /// All five reviewed expansion tags; no receipt key replaces their inventory.
    expansion: [Step; 5],
}

/// Borrowed checked closure; neither its original owner nor proposal may change while retained.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "local checker borrow is inspected by internal controls"
    )
)]
struct Checked<'a> {
    /// Keeps all shared original graphs alive once, without copying their closure.
    owner: &'a Cas,
    /// Retains every original constraint, target, parameter and checked expansion.
    candidate: &'a Candidate,
}

/// Local event categories, each charged before its operation on the original ledger.
#[derive(Clone, Copy)]
#[repr(usize)]
enum Event {
    /// Authenticate one original context, input, endpoint, constraint or target record.
    Original,
    /// Compare one ordered candidate field or source handle.
    Compare,
    /// Visit one quantifier candidate or prior entry while preserving first occurrence.
    Quantifier,
    /// Copy one handle into a newly reserved local record.
    Copy,
    /// Construct or check one compiled mathematical/native relation.
    Relation,
    /// Construct or check one expansion step.
    Expansion,
}

/// Fixed event inventory; observing it cannot affect authority or allocate a transcript.
#[derive(Debug, Default, Eq, PartialEq)]
struct Events([u64; 6]);

/// Charge first, then record a completed event; exhaustion never refunds prior visits.
fn event(budget: &mut Budget, events: &mut Events, kind: Event) -> Result<(), Fault> {
    budget.work(1, AT)?;
    let count = &mut events.0[kind as usize];
    *count = count
        .checked_add(1)
        .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?;
    Ok(())
}

/// Exact reserved local record inventory; original graph payload is borrowed, not duplicated.
const fn bytes(request: &Request) -> u64 {
    let handles = request
        .inputs
        .len()
        .saturating_mul(2)
        .saturating_add(request.constraints.len())
        .saturating_add(6);
    (size_of::<Candidate>() as u64)
        .saturating_add((handles as u64).saturating_mul(size_of::<Handle>() as u64))
}

/// Original caller-owned metadata remains live alongside the newly built or checked record.
const fn request_bytes(request: &Request) -> u64 {
    (size_of::<Request>() as u64).saturating_add(
        (request
            .inputs
            .capacity()
            .saturating_add(request.constraints.capacity()) as u64)
            .saturating_mul(size_of::<Handle>() as u64),
    )
}

/// Supplied proposals may retain excess capacity; charge the actual buffers before checking.
pub(crate) const fn candidate_bytes(proposed: &Candidate) -> u64 {
    let handles = proposed
        .original
        .inputs
        .capacity()
        .saturating_add(proposed.original.constraints.capacity())
        .saturating_add(proposed.universal.capacity());
    (size_of::<Candidate>() as u64)
        .saturating_add((handles as u64).saturating_mul(size_of::<Handle>() as u64))
}

/// Bound input lengths before any caller-controlled traversal or allocation.
fn capacity(request: &Request, budget: &Budget) -> Result<(), Fault> {
    budget.check(BudgetKind::MatrixEntries, request.inputs.len() as u64, AT)?;
    budget.check(
        BudgetKind::MatrixEntries,
        request.constraints.len() as u64,
        AT,
    )?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes(request), AT)
}

/// Structural or mathematical mismatch cannot be confused with an exhausted budget.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedCertificate, AT)
}

/// Closed conditional family schema; this is not a ground enclosure or feasibility claim.
pub const SCHEMA: crate::proof::LemmaSchema = crate::proof::LemmaSchema {
    name: RECIPE,
    label: "thm:cas:lc-positive-root-composition",
    version: 1,
    standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
    statement: "Under the complete original context and conditional premises, the ordered positive product, its original strict-real square root and reciprocal satisfy the retained root and reciprocal brackets and reverse polynomial.",
    premises: "Original `DomainGuard` and Boolean constraints; `StrictSign` for L, C, product lower and root lower endpoints; ordered square and inverse endpoint inequalities; three native total Enclosure obligations and reciprocal `LocalOperation` interpretation.",
};

impl Request {
    /// Original positions in the retained graph dependency inventory, without deduplication.
    pub(crate) fn handles(&self) -> impl Iterator<Item = &Handle> {
        std::iter::once(&self.context)
            .chain(&self.inputs)
            .chain(&self.constraints)
            .chain(&self.endpoints)
            .chain(&self.targets)
    }
}
