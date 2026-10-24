// SPDX-License-Identifier: AGPL-3.0-only
//! The existing Real inverse/residual/perturbation theorem certifies every original block copy.

use super::super::{AT, PhasorError, numeric::Values};
use super::{
    PhasorAllocationStage as Stage,
    inventory::{Owner, Run},
};
use crate::{
    Extent, SolveError,
    solve_verification::{self as verify, SolveMatrix, SolveWork},
};
use nrz_bindings::Budget;
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// Separate center, numerical-family and total-family evidence in coherent current coordinates.
#[derive(Clone, Copy, Debug)]
pub struct PhasorSolveCertificate<P: Scalar> {
    /// Inverse defect, positive lower margin and proved induced inverse-norm upper bound.
    pub inverse: [P; 3],
    /// Represented block residual and its forward bound, before input numerical errors.
    pub residual: [P; 2],
    /// Positive lower numerical and total perturbation margins, respectively.
    pub margins: [P; 2],
    /// Joint block Frobenius numerical and total radii, respectively; they overlap.
    pub radii: [P; 2],
}

/// Final numerical owners share the same complete original allocation plan.
type Solution<P> = (Owner<P, Values<P>>, Owner<P, PhasorSolveCertificate<P>>);

/// One finite block and a selected overlapping radius channel, borrowed for verification.
struct Block<'a, P: Scalar> {
    /// Actual original centers, never reconstructed from one preferred copy.
    centers: &'a [P; 4],
    /// Actual per-copy radii for the current theorem.
    radii: &'a [P; 4],
    /// Checked two-by-two shape.
    extent: Extent,
}
impl<P: Scalar> SolveMatrix<P> for Block<'_, P> {
    fn extent(&self) -> Extent {
        self.extent
    }
    fn get(&self, row: usize, column: usize) -> Result<P, SolveError> {
        Ok(self.centers[self.extent.index(row, column)?])
    }
    fn bound(&self, row: usize, column: usize) -> Result<Option<P>, SolveError> {
        Ok(Some(self.radii[self.extent.index(row, column)?]))
    }
}

/// Refuse a pole or any unresolved sign before constructing an inverse candidate.
pub(super) fn domain<P: Scalar>(
    impedance: &Values<P>,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Enclosure<P>, PhasorError> {
    budget.work(32, AT)?;
    let first = enclosure(impedance.centers[1], impedance.totals[1], context)?;
    let second = enclosure(-impedance.centers[2], impedance.totals[2], context)?;
    let zero =
        |value: Enclosure<P>| value.lower().get() == P::ZERO && value.upper().get() == P::ZERO;
    if zero(first) && zero(second) {
        return Err(PhasorError::ZeroImpedance);
    }
    let positive = first.lower().get() > P::ZERO && second.lower().get() > P::ZERO;
    let negative = first.upper().get() < P::ZERO && second.upper().get() < P::ZERO;
    if !positive && !negative {
        return Err(PhasorError::ReactanceNotSeparated);
    }
    Ok(first.abs())
}

/// Certify both RHS columns together; no component is restored by silently copying another.
pub(super) fn solve<P: Scalar + 'static>(
    impedance: &Values<P>,
    voltage: &Values<P>,
    run: &Run<P>,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Solution<P>, PhasorError> {
    // Fixed upper schedule covers candidate arithmetic, both norms and every verifier contraction.
    budget.work(2048, AT)?;
    let extent = Extent::new(2, 2)?;
    let operator = Block {
        centers: &impedance.centers,
        radii: &impedance.totals,
        extent,
    };
    let rhs = Block {
        centers: &voltage.centers,
        radii: &voltage.totals,
        extent,
    };
    let approximate = run.inline(Stage::InverseCandidate, || inverse(&impedance.centers))?;
    let centers = run.inline(Stage::CurrentCandidate, || {
        candidate(&approximate, &voltage.centers)
    })?;
    let mut work = SolveWork::new(16);
    let verifier = super::verification::Verifier { run, context };
    let witness = verifier.inverse(&operator, &approximate[..], &mut work)?;
    let center = verifier.center(&operator, &rhs, &centers[..], &witness, &mut work)?;
    let family = verifier.family(&operator, &rhs, &witness, &center, false)?;
    let numerical_operator = Block {
        radii: &impedance.errors,
        ..operator
    };
    let numerical_rhs = Block {
        radii: &voltage.errors,
        ..rhs
    };
    let numerical =
        verifier.family(&numerical_operator, &numerical_rhs, &witness, &center, true)?;
    solution(&centers, &witness, &center, &family, &numerical, run)
}

/// Final arrays and certificate facts are constructed under their own publication meanings.
fn solution<P: Scalar + 'static>(
    centers: &[P; 4],
    witness: &verify::InverseWitness<P>,
    center: &verify::CenterCertificate<P>,
    family: &verify::FamilyCertificate<P>,
    numerical: &verify::FamilyCertificate<P>,
    run: &Run<P>,
) -> Result<Solution<P>, PhasorError> {
    let total = if family.radius < numerical.radius {
        numerical.radius
    } else {
        family.radius
    };
    let certificate = run.inline(Stage::Certificate, || {
        Ok(PhasorSolveCertificate {
            inverse: [witness.defect, witness.margin, witness.inverse],
            residual: [center.residual, center.radius],
            margins: [numerical.margin, family.margin],
            radii: [numerical.radius, total],
        })
    })?;
    Ok((
        run.inline(Stage::Result, || {
            Ok(Values {
                centers: *centers,
                totals: [total; 4],
                errors: [numerical.radius; 4],
            })
        })?,
        certificate,
    ))
}

/// A finite candidate is not a theorem; the independent Real verifier checks it afterwards.
fn inverse<P: Scalar>(entries: &[P; 4]) -> Result<[P; 4], PhasorError> {
    let determinant = Finite::new(entries[0] * entries[3] - entries[1] * entries[2])?.get();
    if determinant == P::ZERO {
        return Err(SolveError::CandidateFailure.into());
    }
    let mut result = [entries[3], -entries[1], -entries[2], entries[0]];
    for value in &mut result {
        *value = Finite::new(*value / determinant)?.get();
    }
    Ok(result)
}

/// Compute every current block entry from the actual stored voltage and inverse candidates.
fn candidate<P: Scalar>(inverse: &[P; 4], voltage: &[P; 4]) -> Result<[P; 4], PhasorError> {
    let mut result = [P::ZERO; 4];
    for column in 0..2 {
        for row in 0..2 {
            result[row + 2 * column] = Finite::new(
                inverse[row] * voltage[2 * column] + inverse[row + 2] * voltage[1 + 2 * column],
            )?
            .get();
        }
    }
    Ok(result)
}

/// Outward admission of an original per-copy deterministic or numerical box.
fn enclosure<P: Scalar>(
    center: P,
    radius: P,
    context: &ArithmeticContext,
) -> Result<Enclosure<P>, PhasorError> {
    Ok(
        Enclosure::singleton(Finite::new(center)?)
            .add(Enclosure::new(-radius, radius)?, context)?,
    )
}
