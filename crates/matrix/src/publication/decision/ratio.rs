// SPDX-License-Identifier: AGPL-3.0-only
//! Original quotient inspection and outward containment use separate exact/numerical routes.

use super::{AT, LiveDecisionError};
use crate::FrozenDecisionInputs;
use nrz_bindings::readings::algebra::{Cas, Rational, proof::VerifiedWitness, syntax::Operator};
use nrz_bindings::{Budget, Declaration};
use nrz_scalar::{
    ArithmeticContext, ArithmeticFailure, Enclosure, Finite, LiteralLimits, NumericLiteral, Scalar,
};

/// Match the original physical quotient before inspecting any captured floating coordinates.
pub(super) fn check<P: Scalar>(
    frozen: &FrozenDecisionInputs<P>,
    graph: &Cas,
    witness: &VerifiedWitness,
    arithmetic: &ArithmeticContext,
    literals: LiteralLimits,
    budget: &mut Budget,
) -> Result<(), LiveDecisionError> {
    let declared = frozen
        .operands()
        .iter()
        .all(|operand| operand.input().coverage == crate::Coverage::DeclaredBounds);
    if !declared {
        return Err(LiveDecisionError::Recipe);
    }
    let order = original(frozen, budget)?;
    let (_, certificate) = graph.integer_selection_evidence(witness, budget)?;
    let lower = endpoint::<P>(&certificate.lower, arithmetic, literals, budget)?;
    let upper = endpoint::<P>(&certificate.upper, arithmetic, literals, budget)?;
    budget.work(32, AT)?;
    let [numerator, denominator] = order.map(|slot| frozen.operands()[slot].coordinates());
    let quotient =
        enclosure(numerator, arithmetic)?.div(enclosure(denominator, arithmetic)?, arithmetic)?;
    // Compare with inward sides of the lowered exact endpoints: outward rounding
    // must never enlarge the accepted certificate box.
    if quotient.lower() < lower.upper() || quotient.upper() > upper.lower() {
        return Err(LiveDecisionError::Enclosure);
    }
    Ok(())
}

/// The supported original is exactly two distinct physical constant declarations divided.
fn original<P: Scalar>(
    frozen: &FrozenDecisionInputs<P>,
    budget: &mut Budget,
) -> Result<[usize; 2], LiveDecisionError> {
    budget.work(4, AT)?;
    let association = frozen.association();
    if association.operands().len() != 2 {
        return Err(LiveDecisionError::Recipe);
    }
    let source = association.case().source();
    let Some(Declaration::Derived(observable)) = source.owner().declarations().get(source.name())
    else {
        return Err(LiveDecisionError::Recipe);
    };
    let graph = source.owner().cas()?;
    if graph.expression(&observable.expression.syntax)?.0 != &Operator::Div {
        return Err(LiveDecisionError::Recipe);
    }
    let operands = graph.operands(&observable.expression.syntax, budget)?;
    let [first, second] = operands.as_slice() else {
        return Err(LiveDecisionError::Recipe);
    };
    let mut order = [0; 2];
    for (slot, node) in [first, second].into_iter().enumerate() {
        let (Operator::Free(name), _) = graph.expression(node)? else {
            return Err(LiveDecisionError::Recipe);
        };
        order[slot] = association
            .operands()
            .iter()
            .position(|operand| operand.name() == name)
            .ok_or(LiveDecisionError::Recipe)?;
    }
    if order[0] == order[1] {
        return Err(LiveDecisionError::Recipe);
    }
    Ok(order)
}

/// Decode only a certified exact endpoint; precharge the explicit bounded attempt on refusal too.
fn endpoint<P: Scalar>(
    exact: &Rational,
    arithmetic: &ArithmeticContext,
    literals: LiteralLimits,
    budget: &mut Budget,
) -> Result<Enclosure<P>, LiveDecisionError> {
    let text = exact.exact_string(budget)?;
    budget.work(literals.max_work(), AT)?;
    Ok(NumericLiteral::new(&text, literals)?
        .decode::<P>(arithmetic)?
        .enclosure())
}

/// The numerical error is already included in the total radius and is never added twice.
fn enclosure<P: Scalar>(
    coordinates: [P; 3],
    arithmetic: &ArithmeticContext,
) -> Result<Enclosure<P>, ArithmeticFailure> {
    let [center, total, _] = coordinates;
    let radius = Finite::new(total)?;
    Enclosure::singleton(Finite::new(center)?)
        .add(Enclosure::new(radius.negated().get(), total)?, arithmetic)
}
