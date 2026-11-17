// SPDX-License-Identifier: AGPL-3.0-only
//! Independent expected-claim checking; no producer call or receipt-key authority.

use super::{
    AT, Candidate, Checked, Event, Events, Native, Predicate, RECIPE, Relation, Request, Step,
    Term, candidate_bytes, capacity, event, invalid, request_bytes,
};
use crate::budget::Budget;
use crate::{Cas, Fault, InvalidCode};

/// Reauthenticate the expected original graph and retain a read-only checked closure.
pub(super) fn verify<'a>(
    cas: &'a Cas,
    expected: &Request,
    proposed: &'a Candidate,
    budget: &mut Budget,
    events: &mut Events,
) -> Result<Checked<'a>, Fault> {
    capacity(expected, budget)?;
    capacity(&proposed.original, budget)?;
    let live = request_bytes(expected)
        .saturating_add(candidate_bytes(proposed))
        .saturating_add(size_of::<Checked<'_>>() as u64);
    budget.with_retained_payload(live, AT, |budget| {
        if proposed.recipe != RECIPE {
            return Err(invalid());
        }
        if proposed.version != 1 {
            return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
        }
        super::claim::validate(cas, expected, budget, events)?;
        originals(expected, &proposed.original, budget, events)?;
        universal(expected, proposed, budget, events)?;
        relations(proposed, budget, events)?;
        expansion(proposed, budget, events)?;
        Ok(Checked {
            owner: cas,
            candidate: proposed,
        })
    })
}

/// Compare the complete request, preserving repeated inputs and every unused constraint.
fn originals(
    expected: &Request,
    proposed: &Request,
    budget: &mut Budget,
    events: &mut Events,
) -> Result<(), Fault> {
    event(budget, events, Event::Compare)?;
    if expected.inputs.len() != proposed.inputs.len()
        || expected.constraints.len() != proposed.constraints.len()
    {
        return Err(invalid());
    }
    for (left, right) in std::iter::once((&expected.context, &proposed.context))
        .chain(expected.inputs.iter().zip(&proposed.inputs))
        .chain(expected.constraints.iter().zip(&proposed.constraints))
        .chain(expected.endpoints.iter().zip(&proposed.endpoints))
        .chain(expected.targets.iter().zip(&proposed.targets))
    {
        event(budget, events, Event::Compare)?;
        if left != right {
            return Err(invalid());
        }
    }
    for (left, right) in expected.positions.iter().zip(proposed.positions) {
        event(budget, events, Event::Compare)?;
        if *left != right {
            return Err(invalid());
        }
    }
    event(budget, events, Event::Compare)?;
    if expected.arithmetic != proposed.arithmetic {
        return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
    }
    Ok(())
}

/// Reconstruct binder order independently from original positions, without allocating a proposal.
fn universal(
    expected: &Request,
    proposed: &Candidate,
    budget: &mut Budget,
    events: &mut Events,
) -> Result<(), Fault> {
    let mut cursor = 0;
    for (index, input) in expected.inputs.iter().enumerate() {
        event(budget, events, Event::Quantifier)?;
        let mut repeated = false;
        for earlier in &expected.inputs[..index] {
            event(budget, events, Event::Quantifier)?;
            if earlier == input {
                repeated = true;
                break;
            }
        }
        if !repeated {
            event(budget, events, Event::Compare)?;
            if proposed.universal.get(cursor) != Some(input) {
                return Err(invalid());
            }
            cursor += 1;
        }
    }
    for endpoint in &expected.endpoints {
        event(budget, events, Event::Quantifier)?;
        event(budget, events, Event::Compare)?;
        if proposed.universal.get(cursor) != Some(endpoint) {
            return Err(invalid());
        }
        cursor += 1;
    }
    event(budget, events, Event::Compare)?;
    if cursor != proposed.universal.len() {
        return Err(invalid());
    }
    Ok(())
}

/// Enumerate checker-side ordered schema slots; proposed strictness and terms never choose the law.
fn relations(proposed: &Candidate, budget: &mut Budget, events: &mut Events) -> Result<(), Fault> {
    use Relation::{AtMost, Equal, Less};
    use Term::{C, Endpoint as E, Inverse, L, Product, Reciprocal, Reverse, Root, Square, Zero};
    for (index, predicate) in proposed.premises.iter().enumerate() {
        event(budget, events, Event::Relation)?;
        let expected = match index {
            0 => Predicate(Zero, Less, L),
            1 => Predicate(Zero, Less, C),
            2 => Predicate(Zero, Less, E(0)),
            3 => Predicate(E(0), AtMost, Product),
            4 => Predicate(Product, AtMost, E(1)),
            5 => Predicate(Zero, Less, E(2)),
            6 => Predicate(Zero, AtMost, E(3)),
            7 => Predicate(Square(2), AtMost, E(0)),
            8 => Predicate(E(1), AtMost, Square(3)),
            9 => Predicate(E(4), AtMost, Inverse(3)),
            10 => Predicate(Inverse(2), AtMost, E(5)),
            _ => return Err(invalid()),
        };
        if *predicate != expected {
            return Err(invalid());
        }
    }
    for (index, obligation) in proposed.native.iter().enumerate() {
        event(budget, events, Event::Relation)?;
        let expected = match index {
            0 => Native::ProductTotal,
            1 => Native::RootTotal,
            2 => Native::ReciprocalTotal,
            3 => Native::ReciprocalLocalOperation,
            _ => return Err(invalid()),
        };
        if *obligation != expected {
            return Err(invalid());
        }
    }
    for (index, predicate) in proposed.conclusions.iter().enumerate() {
        event(budget, events, Event::Relation)?;
        let expected = match index {
            0 => Predicate(Zero, Less, Product),
            1 => Predicate(Zero, Less, Root),
            2 => Predicate(Reverse, Equal, Zero),
            3 => Predicate(E(2), AtMost, Root),
            4 => Predicate(Root, AtMost, E(3)),
            5 => Predicate(E(4), AtMost, Reciprocal),
            6 => Predicate(Reciprocal, AtMost, E(5)),
            _ => return Err(invalid()),
        };
        if *predicate != expected {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Match the five tags of the reviewed closed schema; its ordered-field argument
/// supplies soundness, rather than a general inference checker or ground result.
fn expansion(proposed: &Candidate, budget: &mut Budget, events: &mut Events) -> Result<(), Fault> {
    for (index, step) in proposed.expansion.iter().enumerate() {
        event(budget, events, Event::Expansion)?;
        let expected = match index {
            0 => Step::ProductPositive,
            1 => Step::RootUnique,
            2 => Step::RootBracket,
            3 => Step::ReciprocalOrder,
            4 => Step::ReciprocalBracket,
            _ => return Err(invalid()),
        };
        if *step != expected {
            return Err(invalid());
        }
    }
    Ok(())
}
