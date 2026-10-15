// SPDX-License-Identifier: AGPL-3.0-only
//! Direct coefficient-by-coefficient action checks, independent of seed generation.

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Handle, State, size};
use crate::model::Field;
use crate::solving::PolynomialData;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{copied, vector};

/// Orbit failures preserve their own resource location.
pub(crate) const AT: Location = Location::new("finite permutation orbit");

/// Invalid finite evidence never becomes a theorem.
pub(crate) const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedFragment, AT)
}

/// Every slot appears exactly once, with its original scalar sort.
pub(crate) fn permutation(
    image: &[u32],
    variables: &[Field],
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(size(image.len(), AT)?.max(1), AT)?;
    if image.len() != variables.len() {
        return Err(invalid());
    }
    let mut seen = vector(image.len(), budget)?;
    seen.resize(image.len(), false);
    for (source, target) in image.iter().enumerate() {
        let target = usize::try_from(*target).map_err(|_| invalid())?;
        let occupied = seen.get_mut(target).ok_or_else(invalid)?;
        if *occupied || variables.get(source) != variables.get(target) {
            return Err(invalid());
        }
        *occupied = true;
    }
    Ok(())
}

/// Test the image of one monomial without sorting or multiplying polynomials.
fn term_image(
    source: &[u32],
    image: Option<&[u32]>,
    budget: &mut Budget,
) -> Result<Vec<u32>, Fault> {
    budget.solving(BudgetKind::OrbitTerms, 1, AT)?;
    let mut result = copied(source, budget)?;
    if let Some(image) = image {
        for (power, target) in source.iter().zip(image) {
            *result
                .get_mut(usize::try_from(*target).map_err(|_| invalid())?)
                .ok_or_else(invalid)? = *power;
        }
    }
    Ok(result)
}

/// Admitted sparse terms are canonical; compare exact coefficients at each transformed key.
pub(crate) fn equals(
    state: &State,
    source: &PolynomialData<Handle>,
    target: &PolynomialData<Handle>,
    image: Option<&[u32]>,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    budget.work(size(source.variables.len(), AT)?.max(1), AT)?;
    if (&source.domain, &source.variables, source.terms.len())
        != (&target.domain, &target.variables, target.terms.len())
    {
        return Ok(false);
    }
    let engine = Engine {
        state,
        domain: &source.domain,
    };
    for term in &source.terms {
        let key = term_image(&term.exponents, image, budget)?;
        let mut found = None;
        for candidate in &target.terms {
            budget.solving(BudgetKind::OrbitTerms, 1, AT)?;
            budget.work(size(key.len(), AT)?.max(1), AT)?;
            if candidate.exponents == key {
                found = Some(candidate);
                break;
            }
        }
        let Some(candidate) = found else {
            return Ok(false);
        };
        let left = engine
            .read(&term.coefficient, budget)
            .map_err(|error| fault(&error))?;
        let right = engine
            .read(&candidate.coefficient, budget)
            .map_err(|error| fault(&error))?;
        if !engine
            .formal_equal(&left, &right, budget)
            .map_err(|error| fault(&error))?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::permutation;
    use crate::budget::Budget;
    use crate::limits::Limits;
    use crate::model::Field;

    /// Bijection checking includes bounds and preservation of the ordered field frame.
    /// ´test:unit:permutation-bijection-and-sorts´
    #[test]
    fn permutation_bijection_and_sorts() -> Result<(), crate::error::Fault> {
        let mut budget = Budget::new(Limits::default())?;
        let variables = [Field::Real, Field::Real, Field::Complex];
        permutation(&[1, 0, 2], &variables, &mut budget)?;
        for image in [&[0, 0, 2][..], &[0, 1, 3], &[0, 2, 1], &[0, 1]] {
            assert!(permutation(image, &variables, &mut budget).is_err());
        }
        permutation(&[], &[], &mut budget)?;
        Ok(())
    }
}
