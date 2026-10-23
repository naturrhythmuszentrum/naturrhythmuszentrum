// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed reactance admits exact affine current statistics over the original shared voltage source.

use super::PhasorSolveDefinition;
use crate::{
    Budget,
    readings::{PhysicalReading, ReadingError, maps, support},
};
use nrz_cas::{Cas, Rational, limits::BudgetKind, readings::AffineMap};
use std::rc::Rc;

/// Exact affine covariance transport for two current readings driven by one shared voltage family.
///
/// Both impedances must be fixed nonzero imaginary constants over their original families.
/// Varying or uncertain impedance requires another statistical model and is not admitted.
#[derive(Clone, Debug)]
pub struct PhasorCovarianceLaw(Rc<Law>);

/// Original equations, source lineage and both physical target maps remain together.
#[derive(Debug)]
struct Law {
    /// The two original LC equations, including their physical product checks.
    equations: [PhasorSolveDefinition; 2],
    /// Actual current readings in the same order as the covariance factors.
    targets: [PhysicalReading; 2],
    /// Exact complete source-to-current maps; offsets affect means but not covariance.
    gains: [AffineMap; 2],
    /// Exact signed two-sided covariance map in logical column order.
    map: AffineMap,
    /// Retained exact evidence allowance.
    bytes: u64,
}

impl PhasorCovarianceLaw {
    /// Check fixed impedances and actual ordered current selections before deriving both gains.
    /// # Errors
    /// Refuses varying/zero impedance, independent voltage sources, substituted targets and limits.
    pub fn new(
        cas: &Cas,
        equations: [PhasorSolveDefinition; 2],
        targets: [PhysicalReading; 2],
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        let first = equations[0].voltage().reading().family();
        let second = equations[1].voltage().reading().family();
        let source = first.root() == second.root();
        let meanings = first.original_entries() == second.original_entries();
        if !source || !meanings || first.context() != second.context() {
            return Err(ReadingError::Unrelated);
        }
        let gains = [
            gain(cas, &equations[0], &targets[0], budget)?,
            gain(cas, &equations[1], &targets[1], budget)?,
        ];
        let map = maps::covariance(&gains[0], &gains[1], budget)?;
        let mut bytes = 8192_u64
            .checked_add(equations[0].retained_payload_bytes())
            .ok_or(ReadingError::Shape)?;
        if !equations[0].shares_allocation(&equations[1]) {
            bytes = bytes
                .checked_add(equations[1].retained_payload_bytes())
                .ok_or(ReadingError::Shape)?;
        }
        for affine in [&gains[0], &gains[1], &map] {
            for value in affine.coefficients().iter().chain(affine.offsets()) {
                budget.work(1, support::AT)?;
                let (top, bottom) = value.component_bits();
                let limbs = top
                    .div_ceil(64)
                    .checked_add(bottom.div_ceil(64))
                    .ok_or(ReadingError::Shape)?;
                bytes = bytes
                    .checked_add(limbs.checked_mul(16).ok_or(ReadingError::Shape)?)
                    .and_then(|size| size.checked_add(256))
                    .ok_or(ReadingError::Shape)?;
            }
        }
        budget.check(BudgetKind::StagedPayloadBytes, bytes, support::AT)?;
        budget.check(BudgetKind::OutputBytes, bytes, support::AT)?;
        Ok(Self(Rc::new(Law {
            equations,
            targets,
            gains,
            map,
            bytes,
        })))
    }
    /// Both original physical equations, retaining the same voltage source family.
    #[must_use]
    pub fn equations(&self) -> &[PhasorSolveDefinition; 2] {
        &self.0.equations
    }
    /// Actual selected current targets in covariance order.
    #[must_use]
    pub fn targets(&self) -> &[PhysicalReading; 2] {
        &self.0.targets
    }
    /// Complete exact source-to-current maps; repeated copies retain common source columns.
    #[must_use]
    pub fn gains(&self) -> &[AffineMap; 2] {
        &self.0.gains
    }
    /// Signed two-sided covariance map on the original shared voltage covariance.
    #[must_use]
    pub fn map(&self) -> &AffineMap {
        &self.0.map
    }
    /// Conservative retained exact proof allowance.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
}

/// Canonical current coordinates and fixed `i*X` yield `I=(V_imag/X,-V_real/X)`.
fn gain(
    cas: &Cas,
    equation: &PhasorSolveDefinition,
    target: &PhysicalReading,
    budget: &mut Budget,
) -> Result<AffineMap, ReadingError> {
    let current = equation.output().reading();
    if target.family() != current.family()
        || current.family_map(cas, budget)? != AffineMap::identity(2, budget)?
    {
        return Err(ReadingError::Relation);
    }
    let impedance = equation.impedance().reading().family_map(cas, budget)?;
    for coefficient in impedance.coefficients() {
        budget.work(1, support::AT)?;
        if !coefficient.is_zero() {
            return Err(ReadingError::Unsupported);
        }
    }
    let reactance = &impedance.offsets()[1];
    if reactance.is_zero() {
        return Err(ReadingError::Unsupported);
    }
    let reciprocal = Rational::one().div(reactance, budget)?;
    let inverse = AffineMap::new(
        2,
        2,
        vec![
            Rational::zero(),
            reciprocal.clone(),
            reciprocal.negated(budget)?,
            Rational::zero(),
        ],
        vec![Rational::zero(); 2],
        budget,
    )?;
    let voltage = equation.voltage().reading().family_map(cas, budget)?;
    let current_gain = inverse.compose(&voltage, budget)?;
    Ok(target
        .family_map(cas, budget)?
        .compose(&current_gain, budget)?)
}
