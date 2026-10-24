// SPDX-License-Identifier: AGPL-3.0-only
//! Exact coefficient comparison proves the declared similarity on every positive L/C input.

use super::{
    AT, CoordinateNormalization, EntryMeaning, PositiveReciprocalMap, ReadingError,
    SpectralLawError as Error,
};
use crate::{Kind, Measure, Role};
use nrz_cas::{Rational, limits::Budget};

/// Physical checks precede structural-zero elimination and exact coefficient comparison.
pub(super) fn check(
    operator: &PositiveReciprocalMap,
    state: &CoordinateNormalization,
    time: &EntryMeaning,
    budget: &mut Budget,
) -> Result<(), Error> {
    if operator.shape() != [2, 2]
        || operator.inputs().len() != 2
        || state.entries().len() != 2
        || operator.frame().entries().len() != 2
    {
        return Err(Error::Structure);
    }
    if state.context() != operator.frame().context() {
        return Err(Error::Transport);
    }
    physical(state, operator, time, budget)?;
    structure(operator.map(), budget)?;
    factors(operator, state, time, budget)
}

/// The full affine coefficient family, including offsets, must have the restricted zero/sign pattern.
fn structure(map: &nrz_cas::readings::AffineMap, budget: &mut Budget) -> Result<(), Error> {
    if map.inputs() != 2 || map.outputs() != 4 {
        return Err(Error::Structure);
    }
    for index in [0, 1, 6, 7] {
        budget.work(1, AT)?;
        if !map.coefficients()[index].is_zero() {
            return Err(Error::NonzeroDiagonal);
        }
    }
    for index in [0, 3] {
        budget.work(1, AT)?;
        if !map.offsets()[index].is_zero() {
            return Err(Error::NonzeroDiagonal);
        }
    }
    for value in [
        &map.coefficients()[3],
        &map.coefficients()[4],
        &map.offsets()[1],
        &map.offsets()[2],
    ] {
        budget.work(1, AT)?;
        if !value.is_zero() {
            return Err(Error::Structure);
        }
    }
    let upper = &map.coefficients()[5];
    let lower = &map.coefficients()[2];
    if upper.is_zero() || upper.is_negative() || !lower.is_negative() {
        return Err(Error::Sign);
    }
    Ok(())
}

/// Closed LC kinds keep derivative rates separate from angular and cyclic frequencies.
fn physical(
    state: &CoordinateNormalization,
    operator: &PositiveReciprocalMap,
    time: &EntryMeaning,
    budget: &mut Budget,
) -> Result<(), Error> {
    for (entry, name, dimensions) in [
        (&state.entries()[0], "voltage", [2, 1, -3, -1, 0, 0, 0]),
        (&state.entries()[1], "current", [0, 0, 0, 1, 0, 0, 0]),
        (
            &operator.frame().entries()[0],
            "inductance",
            [2, 1, -2, -2, 0, 0, 0],
        ),
        (
            &operator.frame().entries()[1],
            "capacitance",
            [-2, -1, 4, 2, 0, 0, 0],
        ),
        (time, "duration", [0, 0, 1, 0, 0, 0, 0]),
    ] {
        meaning(entry, name, dimensions, budget)?;
    }
    Ok(())
}

/// Compare every exact dimension exponent on the caller's ledger without a hidden default reset.
pub(super) fn meaning(
    entry: &EntryMeaning,
    name: &str,
    dimensions: [i64; 7],
    budget: &mut Budget,
) -> Result<(), Error> {
    let descriptor = entry.descriptor();
    budget.work(
        u64::try_from(name.len()).map_err(|_| ReadingError::Shape)?,
        AT,
    )?;
    let Kind::Named(key, dimension) = descriptor.kind() else {
        return Err(ReadingError::Meaning.into());
    };
    if key != name
        || descriptor.role() != Role::Difference
        || descriptor.measure() != &Measure::coherent()
    {
        return Err(ReadingError::Meaning.into());
    }
    for (actual, expected) in dimension.exponents().iter().zip(dimensions) {
        if actual != &Rational::from_ratio(expected, 1, budget)? {
            return Err(ReadingError::Meaning.into());
        }
    }
    Ok(())
}

/// `a=t_ref*I_ref/(V_ref*C)` and `b=t_ref*V_ref/(I_ref*L)` establish one similarity.
fn factors(
    operator: &PositiveReciprocalMap,
    state: &CoordinateNormalization,
    time: &EntryMeaning,
    budget: &mut Budget,
) -> Result<(), Error> {
    let voltage = state.entries()[0].magnitude();
    let current = state.entries()[1].magnitude();
    let duration = time.magnitude();
    let upper = duration
        .mul(current, budget)?
        .div(voltage, budget)?
        .div(operator.frame().entries()[1].magnitude(), budget)?;
    let lower = duration
        .mul(voltage, budget)?
        .div(current, budget)?
        .div(operator.frame().entries()[0].magnitude(), budget)?;
    let expected = operator.map().coefficients()[2].negated(budget)?;
    if upper.as_rational() != Some(&operator.map().coefficients()[5])
        || lower.as_rational() != Some(&expected)
    {
        return Err(Error::Transport);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Error, structure};
    use nrz_cas::{
        Rational,
        limits::{Budget, Limits},
        readings::AffineMap,
    };
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// A center-zero diagonal with a varying reciprocal coefficient is not a structured family.
    /// ´test:unit:lc-spectral-structure-refuses-center-only-zero´
    #[test]
    fn lc_spectral_structure_refuses_center_only_zero() -> TestResult {
        let mut budget = Budget::new(Limits::default())?;
        let zero = Rational::zero();
        let one = Rational::one();
        let negative = one.negated(&mut budget)?;
        let entries = vec![
            negative.clone(),
            zero.clone(),
            negative,
            zero.clone(),
            zero.clone(),
            one.clone(),
            zero.clone(),
            zero.clone(),
        ];
        let map = AffineMap::new(
            2,
            4,
            entries,
            vec![one, zero.clone(), zero.clone(), zero],
            &mut budget,
        )?;
        assert!(matches!(
            structure(&map, &mut budget),
            Err(Error::NonzeroDiagonal)
        ));
        Ok(())
    }

    /// Wrong off-diagonal signs and even constant nonzero diagonals refuse with named outcomes.
    /// ´test:unit:lc-spectral-structure-checks-signs-and-diagonals´
    #[test]
    fn lc_spectral_structure_checks_signs_and_diagonals() -> TestResult {
        for (coefficient, offset, expected) in
            [(1, 0, "sign"), (-1, 1, "diagonal"), (-1, 0, "accepted")]
        {
            let mut budget = Budget::new(Limits::default())?;
            let mut entries = vec![Rational::zero(); 8];
            entries[2] = Rational::from_ratio(coefficient, 1, &mut budget)?;
            entries[5] = Rational::one();
            let mut shifts = vec![Rational::zero(); 4];
            shifts[3] = Rational::from_ratio(offset, 1, &mut budget)?;
            let map = AffineMap::new(2, 4, entries, shifts, &mut budget)?;
            let result = structure(&map, &mut budget);
            match expected {
                "sign" => assert!(matches!(result, Err(Error::Sign))),
                "diagonal" => assert!(matches!(result, Err(Error::NonzeroDiagonal))),
                _ => assert!(result.is_ok()),
            }
        }
        Ok(())
    }
}
