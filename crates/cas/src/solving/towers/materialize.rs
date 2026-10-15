// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical scratch materialization inside the caller's private graph transaction.

use super::dense::Dense;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Draft, Handle, Reference};
use crate::model::{AtomicSort, Field};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::model::{Element, Value};
use crate::solving::coefficients::polynomial::Polynomial;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::scalar::Scalar;
use crate::solving::coefficients::storage::{AT, add, invalid, product, vector};
use crate::solving::{
    CoefficientValueData, CoefficientValueObject, PolynomialData, PolynomialTerm, SolveDraft,
};

/// Publish one canonical syntax object only in the caller's private candidate.
pub(crate) fn admit(
    cas: &mut Cas,
    draft: SolveDraft,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    cas.admit(&[Draft::Solve(draft)], budget)?
        .into_iter()
        .next()
        .ok_or_else(crate::solving::coefficients::storage::invalid)
}

/// Ground scalar payload copying uses the exact numeric ledger.
fn scalar(value: &Scalar, budget: &mut Budget) -> Result<CoefficientValueData<Reference>, Fault> {
    Ok(match value {
        Scalar::Rational(value) => CoefficientValueData::Rational {
            value: crate::solving::coefficients::scalar::rational_copy(value, budget)?,
        },
        Scalar::Gaussian(value) => CoefficientValueData::Gaussian {
            real: value.real(budget)?,
            imaginary: value.imaginary(budget)?,
        },
    })
}

/// Compute retained key bytes before copying any variable metadata.
fn key_bytes(field: &Field) -> Result<u64, Fault> {
    match field {
        Field::Named(key) => crate::graph::size(key.len(), AT),
        _ => Ok(0),
    }
}

/// Named keys use a fallible reservation after their complete payload preflight.
fn field_copy(field: &Field, budget: &mut Budget) -> Result<Field, Fault> {
    if let Field::Named(key) = field {
        budget.work(key_bytes(field)?, AT)?;
        let mut copied = String::new();
        copied
            .try_reserve_exact(key.len())
            .map_err(|_| Fault::allocation(AT))?;
        copied.push_str(key);
        Ok(Field::Named(copied))
    } else {
        Ok(field.clone())
    }
}

/// Copy variable fields with their complete retained key lengths preflighted.
pub(crate) fn fields(value: &[Field], budget: &mut Budget) -> Result<Vec<Field>, Fault> {
    let mut bytes = product(
        crate::graph::size(value.len(), AT)?,
        crate::graph::size(std::mem::size_of::<Field>(), AT)?,
    )?;
    budget.work(crate::graph::size(value.len(), AT)?, AT)?;
    for field in value {
        bytes = add(bytes, key_bytes(field)?)?;
    }
    budget.check(crate::budget::BudgetKind::StagedPayloadBytes, bytes, AT)?;
    let mut result = vector(value.len(), budget)?;
    for field in value {
        result.push(field_copy(field, budget)?);
    }
    Ok(result)
}

/// An admitted extension has exactly one scalar generator in its polynomial frame.
pub(crate) fn generator(
    value: &[AtomicSort],
    budget: &mut Budget,
) -> Result<Vec<AtomicSort>, Fault> {
    let [AtomicSort::Scalar(field)] = value else {
        return Err(invalid());
    };
    let bytes = add(
        crate::graph::size(std::mem::size_of::<AtomicSort>(), AT)?,
        key_bytes(field)?,
    )?;
    budget.check(crate::budget::BudgetKind::StagedPayloadBytes, bytes, AT)?;
    let mut result = vector(1, budget)?;
    result.push(AtomicSort::Scalar(field_copy(field, budget)?));
    Ok(result)
}

/// A fraction polynomial uses the source's exact ordered coordinate frame.
fn sparse(
    cas: &mut Cas,
    value: &Polynomial,
    base: &Handle,
    variables: &[Field],
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let mut terms = vector(value.terms.len(), budget)?;
    for term in &value.terms {
        let coefficient = admit(
            cas,
            SolveDraft::CoefficientValue(CoefficientValueObject {
                domain: Reference::Existing(base.clone()),
                value: scalar(&term.coefficient, budget)?,
            }),
            budget,
        )?;
        terms.push(PolynomialTerm {
            exponents: crate::solving::coefficients::storage::copied(&term.powers, budget)?,
            coefficient: Reference::Existing(coefficient),
        });
    }
    admit(
        cas,
        SolveDraft::Polynomial(PolynomialData {
            domain: Reference::Existing(base.clone()),
            variables: fields(variables, budget)?,
            terms,
        }),
        budget,
    )
}

/// Derive the exact base and coordinate sorts before releasing the store borrow.
fn fraction_frame(
    cas: &Cas,
    domain: &Handle,
    budget: &mut Budget,
) -> Result<(Handle, Vec<Field>), Fault> {
    let crate::solving::CoefficientDomainData::Fraction { base, coordinates } =
        crate::solving::coefficients::storage::domain(&cas.state, domain)?
    else {
        return Err(crate::solving::coefficients::storage::invalid());
    };
    let SolveDraft::CoordinateSpace(product) =
        crate::solving::graph::object(&cas.state, coordinates.logical_id())?
    else {
        return Err(crate::solving::coefficients::storage::invalid());
    };
    let mut variables = vector(product.coordinates.len(), budget)?;
    for coordinate in &product.coordinates {
        let mut copied = fields(std::slice::from_ref(&coordinate.field), budget)?;
        variables.push(
            copied
                .pop()
                .ok_or_else(crate::solving::coefficients::storage::invalid)?,
        );
    }
    Ok((base.clone(), variables))
}

/// Materialize canonical values recursively, retaining graph sharing at every admission.
pub(crate) fn value(
    cas: &mut Cas,
    element: &Element,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let payload = match &element.value {
        Value::Scalar(value) => scalar(value, budget)?,
        Value::Fraction(value) => {
            let (base, variables) = fraction_frame(cas, &element.domain, budget)?;
            CoefficientValueData::Fraction {
                numerator: Reference::Existing(sparse(
                    cas,
                    &value.numerator,
                    &base,
                    &variables,
                    budget,
                )?),
                denominator: Reference::Existing(sparse(
                    cas,
                    &value.denominator,
                    &base,
                    &variables,
                    budget,
                )?),
            }
        }
        Value::Tower(entries) => {
            let mut coefficients = vector(entries.len(), budget)?;
            for entry in entries {
                coefficients.push(Reference::Existing(self::value(cas, entry, budget)?));
            }
            CoefficientValueData::Tower { coefficients }
        }
    };
    admit(
        cas,
        SolveDraft::CoefficientValue(CoefficientValueObject {
            domain: Reference::Existing(element.domain.clone()),
            value: payload,
        }),
        budget,
    )
}

/// A dense polynomial becomes canonical sparse syntax with zero coefficients omitted.
pub(crate) fn polynomial(
    cas: &mut Cas,
    value: &Dense,
    variables: &[Field],
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let mut terms = vector(value.coefficients.len(), budget)?;
    for (index, coefficient) in value.coefficients.iter().enumerate().rev() {
        let engine = Engine {
            state: &cas.state,
            domain: &value.domain,
        };
        if engine
            .formal_zero(coefficient, budget)
            .map_err(|error| fault(&error))?
        {
            continue;
        }
        let mut exponents = vector(1, budget)?;
        exponents.push(
            u32::try_from(index)
                .map_err(|_| Fault::capacity(crate::budget::BudgetKind::PolynomialDegree, AT))?,
        );
        terms.push(PolynomialTerm {
            exponents,
            coefficient: Reference::Existing(self::value(cas, coefficient, budget)?),
        });
    }
    admit(
        cas,
        SolveDraft::Polynomial(PolynomialData {
            domain: Reference::Existing(value.domain.clone()),
            variables: fields(variables, budget)?,
            terms,
        }),
        budget,
    )
}
