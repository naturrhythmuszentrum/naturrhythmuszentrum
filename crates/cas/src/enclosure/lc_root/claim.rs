// SPDX-License-Identifier: AGPL-3.0-only
//! Expected original admission and untrusted proposal construction.

use super::{
    AT, Candidate, Event, Events, Native, Predicate, RECIPE, Relation, Request, Step, Term, bytes,
    capacity, event, invalid, request_bytes,
};
use crate::budget::Budget;
use crate::graph::{Expression, LogicalId};
use crate::model::{AtomicSort, Branch, Field, Operator, Sort};
use crate::{Cas, Fault, Handle, InvalidCode};
use Relation::{AtMost, Equal, Less};
use Term::{C, Endpoint as E, Inverse, L, Product, Reciprocal, Reverse, Root, Square, Zero};

/// Construct only an untrusted implication; the independent checker grants the private borrow.
pub(super) fn construct(
    cas: &Cas,
    request: &Request,
    budget: &mut Budget,
    events: &mut Events,
) -> Result<Candidate, Fault> {
    capacity(request, budget)?;
    budget.with_retained_payload(
        bytes(request).saturating_add(request_bytes(request)),
        AT,
        |budget| {
            validate(cas, request, budget, events)?;
            let universal = quantify(request, budget, events)?;
            let original = copy(request, budget, events)?;
            for _ in 0..22 {
                event(budget, events, Event::Relation)?;
            }
            for _ in 0..5 {
                event(budget, events, Event::Expansion)?;
            }
            Ok(Candidate {
                recipe: RECIPE,
                version: 1,
                original,
                universal,
                premises: [
                    Predicate(Zero, Less, L),
                    Predicate(Zero, Less, C),
                    Predicate(Zero, Less, E(0)),
                    Predicate(E(0), AtMost, Product),
                    Predicate(Product, AtMost, E(1)),
                    Predicate(Zero, Less, E(2)),
                    Predicate(Zero, AtMost, E(3)),
                    Predicate(Square(2), AtMost, E(0)),
                    Predicate(E(1), AtMost, Square(3)),
                    Predicate(E(4), AtMost, Inverse(3)),
                    Predicate(Inverse(2), AtMost, E(5)),
                ],
                native: [
                    Native::ProductTotal,
                    Native::RootTotal,
                    Native::ReciprocalTotal,
                    Native::ReciprocalLocalOperation,
                ],
                conclusions: [
                    Predicate(Zero, Less, Product),
                    Predicate(Zero, Less, Root),
                    Predicate(Reverse, Equal, Zero),
                    Predicate(E(2), AtMost, Root),
                    Predicate(Root, AtMost, E(3)),
                    Predicate(E(4), AtMost, Reciprocal),
                    Predicate(Reciprocal, AtMost, E(5)),
                ],
                expansion: [
                    Step::ProductPositive,
                    Step::RootUnique,
                    Step::RootBracket,
                    Step::ReciprocalOrder,
                    Step::ReciprocalBracket,
                ],
            })
        },
    )
}

/// Charge and fallibly copy only local handle records; no original graph is cloned.
fn handles(
    source: &[Handle],
    budget: &mut Budget,
    events: &mut Events,
) -> Result<Vec<Handle>, Fault> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(source.len())
        .map_err(|_| Fault::allocation(AT))?;
    for handle in source {
        event(budget, events, Event::Copy)?;
        result.push(handle.clone());
    }
    Ok(result)
}

/// Copy the complete original inventory without canonicalizing its positions or constraints.
fn copy(request: &Request, budget: &mut Budget, events: &mut Events) -> Result<Request, Fault> {
    let inputs = handles(&request.inputs, budget, events)?;
    let constraints = handles(&request.constraints, budget, events)?;
    for _ in 0..11 {
        event(budget, events, Event::Copy)?;
    }
    Ok(Request {
        context: request.context.clone(),
        inputs,
        constraints,
        positions: request.positions,
        endpoints: request.endpoints.clone(),
        targets: request.targets.clone(),
        arithmetic: request.arithmetic,
    })
}

/// Preserve first-occurrence input order, then append the six distinct endpoint binders.
fn quantify(
    request: &Request,
    budget: &mut Budget,
    events: &mut Events,
) -> Result<Vec<Handle>, Fault> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(request.inputs.len().saturating_add(6))
        .map_err(|_| Fault::allocation(AT))?;
    for handle in request.inputs.iter().chain(&request.endpoints) {
        event(budget, events, Event::Quantifier)?;
        let mut present = false;
        for prior in &result {
            event(budget, events, Event::Quantifier)?;
            if prior == handle {
                present = true;
                break;
            }
        }
        if !present {
            event(budget, events, Event::Copy)?;
            result.push(handle.clone());
        }
    }
    Ok(result)
}

/// Authenticate all original records and inspect the unsimplified target on every check.
pub(super) fn validate(
    cas: &Cas,
    request: &Request,
    budget: &mut Budget,
    events: &mut Events,
) -> Result<(), Fault> {
    if request.arithmetic != 1 {
        return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
    }
    event(budget, events, Event::Original)?;
    let context = cas.state.context(cas.checked(&request.context)?)?;
    event(budget, events, Event::Original)?;
    if !cas.state.region(context.admissible)?.scope.is_empty() {
        return Err(invalid());
    }
    let header = context.header_id;
    if request
        .positions
        .iter()
        .any(|position| *position >= request.inputs.len())
    {
        return Err(invalid());
    }
    for input in request.inputs.iter().chain(&request.endpoints) {
        let record = node(cas, cas.checked(input)?, header, budget, events)?;
        if !matches!(record.operator, Operator::Free(_)) {
            return Err(invalid());
        }
    }
    for (index, endpoint) in request.endpoints.iter().enumerate() {
        for other in request.inputs.iter().chain(&request.endpoints[..index]) {
            event(budget, events, Event::Compare)?;
            if endpoint == other {
                return Err(invalid());
            }
        }
    }
    for constraint in &request.constraints {
        event(budget, events, Event::Original)?;
        let value = cas.state.expression(cas.checked(constraint)?)?;
        if value.header != header
            || !value.scope.is_empty()
            || value.sort != Sort::Value(AtomicSort::Boolean)
        {
            return Err(invalid());
        }
    }
    targets(cas, request, header, budget, events)
}

/// Check a scalar original node directly rather than trusting cached enclosure admission flags.
fn node<'a>(
    cas: &'a Cas,
    id: LogicalId,
    header: LogicalId,
    budget: &mut Budget,
    events: &mut Events,
) -> Result<&'a Expression, Fault> {
    event(budget, events, Event::Original)?;
    let value = cas.state.expression(id)?;
    if value.header != header
        || !value.scope.is_empty()
        || value.sort != Sort::Value(AtomicSort::Scalar(Field::Real))
    {
        return Err(invalid());
    }
    Ok(value)
}

/// Keep the root expression and its reverse polynomial as distinct original targets.
fn targets(
    cas: &Cas,
    request: &Request,
    header: LogicalId,
    budget: &mut Budget,
    events: &mut Events,
) -> Result<(), Fault> {
    let [product, root, reciprocal, reverse] = request.targets.each_ref().map(|h| cas.checked(h));
    let (product, root, reciprocal, reverse) = (product?, root?, reciprocal?, reverse?);
    let value = node(cas, product, header, budget, events)?;
    let l = cas.checked(&request.inputs[request.positions[0]])?;
    let c = cas.checked(&request.inputs[request.positions[1]])?;
    if value.operator != Operator::Mul || value.operands != [l, c] {
        return Err(invalid());
    }
    let value = node(cas, root, header, budget, events)?;
    if value.operator != Operator::GeneralPower(Branch::StrictReal)
        || value.operands.len() != 2
        || value.operands[0] != product
    {
        return Err(invalid());
    }
    let exponent = node(cas, value.operands[1], header, budget, events)?;
    if !matches!(&exponent.operator, Operator::Rational(n) if n.is_half()) {
        return Err(invalid());
    }
    let value = node(cas, reciprocal, header, budget, events)?;
    if value.operator != Operator::Div || value.operands.len() != 2 || value.operands[1] != root {
        return Err(invalid());
    }
    let one = node(cas, value.operands[0], header, budget, events)?;
    if !matches!(&one.operator, Operator::Rational(n) if n == &crate::Rational::one()) {
        return Err(invalid());
    }
    let value = node(cas, reverse, header, budget, events)?;
    if value.operator != Operator::Sub || value.operands.len() != 2 || value.operands[1] != product
    {
        return Err(invalid());
    }
    let square = node(cas, value.operands[0], header, budget, events)?;
    if square.operator != Operator::Mul || square.operands != [root, root] {
        return Err(invalid());
    }
    Ok(())
}
