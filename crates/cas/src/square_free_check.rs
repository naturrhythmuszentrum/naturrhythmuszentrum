// SPDX-License-Identifier: AGPL-3.0-only
//! Independent checking of exact multiplicity evidence, with no producer gcd execution.

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::euclidean::{check as gcd, require_equal};
use crate::graph::{Cas, LogicalId, size};
use crate::model::{Field, Operator};
use crate::multiplicity::SquareFreeCertificate;
use crate::number::Rational;
use crate::proof_model::{ProofStep, Relation};
use crate::univariate::Polynomial;

/// Multiplicity checker location.
const AT: Location = Location::new("square-free checking");

/// Independently reconstruct the polynomial, every certified gcd and the claimed output shape.
pub(crate) fn check(
    cas: &Cas,
    context: LogicalId,
    step: &ProofStep,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Relation::SquareFree {
        variable,
        field: Field::Rational,
    } = &step.relation
    else {
        return Err(invalid());
    };
    if !step.premises.is_empty() {
        return Err(invalid());
    }
    let certificate = step.certificate.as_ref().ok_or_else(invalid)?;
    let retained = certificate.payload(budget)?;
    let previous = budget.reserve_temporary(retained, AT)?;
    let result = (|| {
        let input = crate::square_free::coefficients(
            cas,
            context,
            cas.checked(&step.left)?,
            variable,
            budget,
        )?;
        if input.is_zero() || certificate.scalar.is_zero() {
            return Err(invalid());
        }
        input.prepay(budget)?;
        if input.leading()? != &certificate.scalar {
            return Err(invalid());
        }
        let inverse = Rational::one().div(&certificate.scalar, budget)?;
        let monic = input.scale(&inverse, budget)?;
        rounds(&monic, certificate, budget)?;
        layer_properties(certificate, budget)?;
        product(&input, certificate, budget)?;
        output(
            cas,
            context,
            cas.checked(&step.right)?,
            variable,
            certificate,
            budget,
        )
    })();
    budget.restore_limits(previous);
    result
}

/// Replay exact quotient recurrence and compare every emitted nonconstant layer in order.
fn rounds(
    monic: &Polynomial,
    certificate: &SquareFreeCertificate,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let derivative = monic.derivative(budget)?;
    let mut common = gcd(monic, &derivative, &certificate.initial_gcd, budget)?;
    let mut remaining = Polynomial::new(&certificate.initial_gcd.left_quotient, budget)?;
    let mut layer_position = 0_usize;
    for (position, evidence) in certificate.rounds.iter().enumerate() {
        if remaining.is_one() {
            return Err(invalid());
        }
        let multiplicity = size(position, AT)?.checked_add(1).ok_or_else(invalid)?;
        budget.check(BudgetKind::PolynomialDegree, multiplicity, AT)?;
        if multiplicity > size(monic.degree().ok_or_else(invalid)?, AT)? {
            return Err(invalid());
        }
        let divisor = gcd(&remaining, &common, evidence, budget)?;
        let factor = Polynomial::new(&evidence.left_quotient, budget)?;
        common = Polynomial::new(&evidence.right_quotient, budget)?;
        remaining = divisor;
        if !factor.is_one() {
            let layer = certificate.layers.get(layer_position).ok_or_else(invalid)?;
            if layer.multiplicity != multiplicity {
                return Err(invalid());
            }
            let claimed = Polynomial::new(&layer.coefficients, budget)?;
            require_equal(&factor, &claimed, budget)?;
            layer_position = layer_position.checked_add(1).ok_or_else(invalid)?;
        }
    }
    if !remaining.is_one() || !common.is_one() || layer_position != certificate.layers.len() {
        return Err(invalid());
    }
    Ok(())
}

/// A factor's derivative gcd and every earlier/later factor gcd must be exactly one.
fn layer_properties(certificate: &SquareFreeCertificate, budget: &mut Budget) -> Result<(), Fault> {
    if certificate.square_free.len() != certificate.layers.len() {
        return Err(invalid());
    }
    let mut pairs = certificate.coprime.iter();
    let mut previous = 0_u64;
    for (position, layer) in certificate.layers.iter().enumerate() {
        let value = Polynomial::new(&layer.coefficients, budget)?;
        if value.degree().is_none_or(|degree| degree == 0)
            || *value.leading()? != Rational::one()
            || layer.multiplicity <= previous
        {
            return Err(invalid());
        }
        previous = layer.multiplicity;
        let derivative = value.derivative(budget)?;
        let evidence = certificate.square_free.get(position).ok_or_else(invalid)?;
        if !gcd(&value, &derivative, evidence, budget)?.is_one() {
            return Err(invalid());
        }
        for other in certificate
            .layers
            .iter()
            .skip(position.checked_add(1).ok_or_else(invalid)?)
        {
            let other = Polynomial::new(&other.coefficients, budget)?;
            let evidence = pairs.next().ok_or_else(invalid)?;
            if !gcd(&value, &other, evidence, budget)?.is_one() {
                return Err(invalid());
            }
        }
    }
    if pairs.next().is_some() {
        return Err(invalid());
    }
    Ok(())
}

/// Direct product reconstruction is checked in addition to the recurrence and gcd identities.
fn product(
    input: &Polynomial,
    certificate: &SquareFreeCertificate,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut product = Polynomial::new(std::slice::from_ref(&certificate.scalar), budget)?;
    for layer in &certificate.layers {
        let factor = Polynomial::new(&layer.coefficients, budget)?;
        product = product.mul(&factor.pow(layer.multiplicity, budget)?, budget)?;
    }
    require_equal(input, &product, budget)
}

/// Bind the certified factors to the exact positive-power product in the theorem endpoint.
fn output(
    cas: &Cas,
    context: LogicalId,
    mut root: LogicalId,
    variable: &str,
    certificate: &SquareFreeCertificate,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for layer in certificate.layers.iter().rev() {
        budget.work(1, AT)?;
        let term = cas.state.expression(root)?;
        if term.operator != Operator::Mul {
            return Err(invalid());
        }
        let power = *term.operands.get(1).ok_or_else(invalid)?;
        root = *term.operands.first().ok_or_else(invalid)?;
        let power = cas.state.expression(power)?;
        let exponent = i64::try_from(layer.multiplicity).map_err(|_| invalid())?;
        if power.operator != Operator::IntegerPower(exponent) {
            return Err(invalid());
        }
        let factor = *power.operands.first().ok_or_else(invalid)?;
        let actual = crate::square_free::coefficients(cas, context, factor, variable, budget)?;
        let expected = Polynomial::new(&layer.coefficients, budget)?;
        require_equal(&actual, &expected, budget)?;
    }
    let Operator::Rational(scalar) = &cas.state.expression(root)?.operator else {
        return Err(invalid());
    };
    scalar.check(budget, AT)?;
    if scalar == &certificate.scalar {
        Ok(())
    } else {
        Err(invalid())
    }
}

/// No failed multiplicity claim is promoted to a proof.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Rule, AT)
}
