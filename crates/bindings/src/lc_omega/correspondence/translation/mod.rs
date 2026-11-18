// SPDX-License-Identifier: AGPL-3.0-only
//! Closed rational adapter, preserving every original position and predicate.

mod draft;
mod pool;
mod request;

use super::{AT, Result, accounting};
use crate::{AdmissionError, LcInput, LcQuantity};
use draft::Builder;
use nrz_cas::{
    limits::Budget,
    syntax::{Branch, Operator, PredicateKind},
};

/// Fresh universally quantified bracket parameters, never evaluated interval bounds.
const ENDPOINTS: [&str; 6] = [
    "P_lower", "P_upper", "t_lower", "t_upper", "a_lower", "a_upper",
];
/// Local draft references are u32, leaving this bit free on the supported target.
const EXACT: usize = 1_usize << (usize::BITS - 1);

/// Translation metadata borrows no candidate receipt or portable bytes.
pub(super) struct Translation {
    /// Complete original syntax, still distinct at draft level.
    builder: Builder,
    /// Input coordinate and constraint root indices, reusing the endpoint buffer.
    indices: Vec<usize>,
    /// Six fresh endpoint draft indices.
    endpoints: [u32; 6],
    /// Product, root, reciprocal and reverse equation.
    targets: [u32; 4],
}

/// Four possible physical coordinates, in first-occurrence order.
struct Coordinates {
    /// Only the initialized prefix is inspected.
    order: [LcQuantity; 4],
    /// Number of distinct coordinates.
    len: usize,
    /// Direct lookup by closed quantity tag after the charged first-occurrence scan.
    indices: [u32; 4],
}

/// Refuse unsupported physical coordinates without normalizing their meanings.
const fn tag(quantity: LcQuantity) -> Result<usize> {
    match quantity {
        LcQuantity::Inductance => Ok(0),
        LcQuantity::Capacitance => Ok(1),
        LcQuantity::InitialPhase => Ok(2),
        LcQuantity::Energy => Ok(3),
        _ => Err(AdmissionError::UnsupportedRecipe),
    }
}

/// Portable names contain no allocation or process identity.
fn name(quantity: LcQuantity) -> Result<&'static str> {
    Ok(["L", "C", "phase", "E"][tag(quantity)?])
}

/// Charge each original row and the exact first-occurrence comparison schedule.
fn coordinates(inputs: &[LcInput], budget: &mut Budget) -> Result<Coordinates> {
    let mut result = Coordinates {
        order: [LcQuantity::Inductance; 4],
        len: 0,
        indices: [0; 4],
    };
    for input in inputs {
        accounting::original(1, budget)?;
        let tag = tag(input.quantity())?;
        let mut found = None;
        for (index, previous) in result.order[..result.len].iter().enumerate() {
            accounting::original(1, budget)?;
            if *previous == input.quantity() {
                found = Some(index);
                break;
            }
        }
        let index = found.unwrap_or(result.len);
        if found.is_none() {
            result.order[index] = input.quantity();
            result.len += 1;
        }
        result.indices[tag] =
            u32::try_from(3 + index).map_err(|_| AdmissionError::ResourceLimit)?;
    }
    Ok(result)
}

/// Assemble only supported original syntax on the caller's cumulative ledger.
pub(super) fn build(inputs: &[LcInput], budget: &mut Budget) -> Result<Translation> {
    let coordinates = coordinates(inputs, budget)?;
    let mut pool = budget.with_work_phase("LC omega correspondence", "expression", |budget| {
        pool::build(inputs, budget)
    })?;
    let count = 3 + coordinates.len + 6 + pool.values.len() + pool.predicate_nodes + 5;
    let mut builder = Builder::new(count, pool.bytes()?, budget)?;
    header(&mut builder, &coordinates, budget)?;
    let first_literal =
        u32::try_from(builder.drafts.len()).map_err(|_| AdmissionError::ResourceLimit)?;
    for value in pool.values.drain(..) {
        builder.external -= accounting::rational(&value);
        builder.node(Operator::Rational(value), &[], false, budget)?;
    }
    drop(pool.values);
    builder.external = accounting::vector::<usize>(pool.endpoints.capacity())?;
    predicates_from_originals(
        &mut builder,
        inputs,
        &coordinates,
        &mut pool.endpoints,
        first_literal,
        budget,
    )?;
    let targets = targets(&mut builder, &coordinates, first_literal, budget)?;
    let start = u32::try_from(3 + coordinates.len).map_err(|_| AdmissionError::ResourceLimit)?;
    let endpoints = [0_u32, 1, 2, 3, 4, 5].map(|offset| start + offset);
    Ok(Translation {
        builder,
        indices: pool.endpoints,
        endpoints,
        targets,
    })
}

/// Header and free-coordinate names share only their portable spelling, not string allocations.
fn header(builder: &mut Builder, coordinates: &Coordinates, budget: &mut Budget) -> Result<()> {
    let mut names = [""; 10];
    for (slot, quantity) in names.iter_mut().zip(&coordinates.order[..coordinates.len]) {
        *slot = name(*quantity)?;
    }
    names[coordinates.len..coordinates.len + 6].copy_from_slice(&ENDPOINTS);
    let names = &names[..coordinates.len + 6];
    builder.header(names, budget)?;
    for name in names {
        let name = builder.name(name, budget)?;
        builder.node(Operator::Free(name), &[], false, budget)?;
    }
    Ok(())
}

/// Every row emits its own tree and replaces consumed endpoint indices with request indices.
fn predicates_from_originals(
    builder: &mut Builder,
    inputs: &[LcInput],
    coordinates: &Coordinates,
    indices: &mut [usize],
    literal: u32,
    budget: &mut Budget,
) -> Result<()> {
    for (input, pair) in inputs.iter().zip(indices.as_chunks_mut::<2>().0) {
        let coordinate = coordinates.indices[tag(input.quantity())?];
        let lower = literal + u32::try_from(pair[0]).map_err(|_| AdmissionError::ResourceLimit)?;
        let upper =
            literal + u32::try_from(pair[1] & !EXACT).map_err(|_| AdmissionError::ResourceLimit)?;
        let predicate = if pair[1] & EXACT != 0 {
            builder.node(
                Operator::Predicate(PredicateKind::Equal),
                &[coordinate, lower],
                true,
                budget,
            )?
        } else {
            interval(builder, coordinate, lower, upper, budget)?
        };
        budget.with_retained_payload(builder.bytes()?, AT, |budget| {
            accounting::expression(2, budget)
        })?;
        pair[0] = coordinate as usize;
        pair[1] = predicate as usize;
    }
    Ok(())
}

/// Literal closed support keeps the lower and upper subtraction orders.
fn interval(
    builder: &mut Builder,
    coordinate: u32,
    lower: u32,
    upper: u32,
    budget: &mut Budget,
) -> Result<u32> {
    let lower = builder.node(Operator::Sub, &[coordinate, lower], false, budget)?;
    let lower = builder.node(
        Operator::Predicate(PredicateKind::NonNegative),
        &[lower],
        true,
        budget,
    )?;
    let upper = builder.node(Operator::Sub, &[upper, coordinate], false, budget)?;
    let upper = builder.node(
        Operator::Predicate(PredicateKind::NonNegative),
        &[upper],
        true,
        budget,
    )?;
    builder.node(
        Operator::Predicate(PredicateKind::And),
        &[lower, upper],
        true,
        budget,
    )
}

/// The four registered targets share the same product and strict-real root.
fn targets(
    builder: &mut Builder,
    coordinates: &Coordinates,
    literal: u32,
    budget: &mut Budget,
) -> Result<[u32; 4]> {
    let product = builder.node(Operator::Mul, &coordinates.indices[..2], false, budget)?;
    let root = builder.node(
        Operator::GeneralPower(Branch::StrictReal),
        &[product, literal + 1],
        false,
        budget,
    )?;
    let reciprocal = builder.node(Operator::Div, &[literal, root], false, budget)?;
    let square = builder.node(Operator::Mul, &[root, root], false, budget)?;
    let reverse = builder.node(Operator::Sub, &[square, product], false, budget)?;
    Ok([product, root, reciprocal, reverse])
}
