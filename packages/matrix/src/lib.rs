// SPDX-License-Identifier: AGPL-3.0-only
//! Provisional dense binary64 solve and independent certificate.
//! The interface remains provisional until the units and matrix bites settle it.
//! Candidate computation follows rule:vision:backend-boundaries; all evidence follows dec:vision:independent-verification-arithmetic.
#![expect(
    clippy::multiple_crate_versions,
    reason = "faer's dependency tree carries duplicate crate versions outside the workspace's control"
)]

use nrz_scalar::{ArithmeticFailure, Interval, dot, frobenius_upper, sum_squares};

/// The sole backend adapter, kept private to this crate.
#[expect(
    clippy::redundant_pub_crate,
    reason = "backend functions are scoped to the private module"
)]
mod backend;

/// A square center system with deterministic Frobenius input radii.
#[derive(Debug, Clone)]
pub struct DenseSystem {
    /// Matrix order.
    n: usize,
    /// Matrix center in row-major order.
    a: Vec<f64>,
    /// Matrix perturbation radius.
    a_radius: f64,
    /// Right-hand side center.
    b: Vec<f64>,
    /// Right-hand side perturbation radius.
    b_radius: f64,
}

impl DenseSystem {
    /// Validate a represented center and two declared perturbation bounds.
    /// This is an import boundary under rule:vision:imports-and-exactness.
    ///
    /// # Errors
    ///
    /// Invalid shape or non-finite entries and radii return a `NumericalFailure`.
    pub fn new(
        n: usize,
        a: Vec<f64>,
        a_radius: f64,
        b: Vec<f64>,
        b_radius: f64,
    ) -> Result<Self, NumericalFailure> {
        if n == 0 || n.checked_mul(n) != Some(a.len()) || b.len() != n {
            return Err(NumericalFailure::ShapeMismatch);
        }
        if a.iter().chain(&b).any(|value| !value.is_finite())
            || !a_radius.is_finite()
            || a_radius < 0.0
            || !b_radius.is_finite()
            || b_radius < 0.0
        {
            return Err(NumericalFailure::NonFinite);
        }
        Ok(Self {
            n,
            a,
            a_radius,
            b,
            b_radius,
        })
    }

    /// Matrix order.
    #[must_use]
    pub const fn order(&self) -> usize {
        self.n
    }
    /// Represented row-major matrix center.
    #[must_use]
    pub fn matrix(&self) -> &[f64] {
        &self.a
    }
    /// Represented right-hand side center.
    #[must_use]
    pub fn right_hand_side(&self) -> &[f64] {
        &self.b
    }
}

/// Numerical evidence accompanying a successful solve.
#[derive(Debug, Clone, Copy)]
pub struct Report {
    /// Upper bound on the Frobenius norm of identity minus approximate inverse times matrix.
    pub q: f64,
    /// Upper bound on the inverse norm under the Neumann argument.
    pub g: f64,
    /// Upper bound on the residual norm.
    pub rho: f64,
    /// Upper bound on the candidate norm.
    pub xnorm: f64,
    /// Declared matrix input radius.
    pub a_radius: f64,
    /// Declared right-hand-side input radius.
    pub b_radius: f64,
    /// Requested relative radius.
    pub relative_budget: f64,
    /// Matrix order.
    pub n: usize,
}

/// A candidate with an independently certified Euclidean error radius.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// Candidate solution.
    pub candidate: Vec<f64>,
    /// Absolute Euclidean error radius.
    pub radius: f64,
    /// Certificate diagnostics.
    pub report: Report,
}

/// A refusal to claim a requested certified solve.
#[derive(Debug, thiserror::Error)]
pub enum NumericalFailure {
    /// The approximate-inverse conditions do not establish a certificate.
    #[error("the approximate inverse cannot certify this system")]
    CannotCertify,
    /// A valid radius exceeds the requested relative budget.
    #[error("certified radius {radius} exceeds relative budget {budget}")]
    BudgetExceeded {
        /// Certified absolute radius.
        radius: f64,
        /// Requested relative budget.
        budget: f64,
    },
    /// Scalar arithmetic could not produce a finite enclosure.
    #[error("scalar enclosure failed: {0}")]
    Arithmetic(#[from] ArithmeticFailure),
    /// The input dimensions do not describe a nonempty square system.
    #[error("matrix and right-hand-side shapes do not match a nonempty square system")]
    ShapeMismatch,
    /// An entry, radius, or budget is invalid or non-finite.
    #[error("a system entry, radius, or budget is non-finite or invalid")]
    NonFinite,
}

/// Largest magnitude in an enclosing interval.
const fn magnitude(bound: Interval<f64>) -> f64 {
    bound.lo().abs().max(bound.hi().abs())
}

/// Upper bound on the Frobenius norm of identity minus approximate inverse times matrix.
fn inverse_defect(system: &DenseSystem, z: &[f64]) -> Result<f64, ArithmeticFailure> {
    let mut entries = Vec::with_capacity(system.a.len());
    for row in 0..system.n {
        for col in 0..system.n {
            let column: Vec<f64> = (0..system.n)
                .map(|k| system.a[k * system.n + col])
                .collect();
            let product = dot(&z[row * system.n..(row + 1) * system.n], &column)?;
            let identity = if row == col { 1.0 } else { 0.0 };
            entries.push(magnitude(Interval::point(identity)?.checked_sub(product)?));
        }
    }
    frobenius_upper(&entries)
}

/// Upper bound on the residual norm of the represented center system.
fn residual(system: &DenseSystem, x: &[f64]) -> Result<f64, ArithmeticFailure> {
    let mut entries = Vec::with_capacity(system.n);
    for row in 0..system.n {
        let product = dot(&system.a[row * system.n..(row + 1) * system.n], x)?;
        entries.push(magnitude(
            Interval::point(system.b[row])?.checked_sub(product)?,
        ));
    }
    frobenius_upper(&entries)
}

/// Compute the radii after a backend candidate and inverse are available.
fn certify_candidate(
    system: &DenseSystem,
    relative_budget: f64,
    x: Vec<f64>,
    z: &[f64],
) -> Result<Outcome, NumericalFailure> {
    if x.iter().chain(z.iter()).any(|value| !value.is_finite()) {
        return Err(NumericalFailure::CannotCertify);
    }
    let q = inverse_defect(system, z)?;
    if q >= 1.0 {
        return Err(NumericalFailure::CannotCertify);
    }
    let one = Interval::point(1.0)?;
    let g = Interval::point(frobenius_upper(z)?)?
        .checked_div(one.checked_sub(Interval::point(q)?)?)?
        .hi();
    let rho = residual(system, &x)?;
    let xnorm = frobenius_upper(&x)?;
    let ga = Interval::point(g)?.checked_mul(Interval::point(system.a_radius)?)?;
    if ga.hi() >= 1.0 {
        return Err(NumericalFailure::CannotCertify);
    }
    let numerator = Interval::point(rho)?
        .checked_add(Interval::point(system.b_radius)?)?
        .checked_add(Interval::point(system.a_radius)?.checked_mul(Interval::point(xnorm)?)?)?;
    let radius = Interval::point(g)?
        .checked_mul(numerator)?
        .checked_div(one.checked_sub(ga)?)?
        .hi();
    let lower_xnorm = sum_squares(&x)?.sqrt()?.lo();
    let allowed = Interval::point(relative_budget)?
        .checked_mul(Interval::point(lower_xnorm)?)?
        .lo();
    if radius > allowed {
        return Err(NumericalFailure::BudgetExceeded {
            radius,
            budget: relative_budget,
        });
    }
    let report = Report {
        q,
        g,
        rho,
        xnorm,
        a_radius: system.a_radius,
        b_radius: system.b_radius,
        relative_budget,
        n: system.n,
    };
    Ok(Outcome {
        candidate: x,
        radius,
        report,
    })
}

/// Solve a dense system and certify its candidate through plain-slice scalar arithmetic.
/// Refusal does not establish singularity (rule:vision:conditioning-and-refusal).
///
/// # Errors
///
/// An invalid budget, failed scalar enclosure, uncertifiable inverse or exceeded budget returns a `NumericalFailure`.
pub fn certified_solve(
    system: &DenseSystem,
    relative_budget: f64,
) -> Result<Outcome, NumericalFailure> {
    if !relative_budget.is_finite() || relative_budget < 0.0 {
        return Err(NumericalFailure::NonFinite);
    }
    let (x, z) = backend::candidate_and_inverse(system);
    certify_candidate(system, relative_budget, x, &z)
}

#[cfg(test)]
/// Unit evidence for certificate refusal and containment.
mod tests {
    use super::{DenseSystem, NumericalFailure, certified_solve, certify_candidate};

    /// A small exact diagonal system admits a certified all-ones solution.
    /// ´test:unit:small-exact-system-certifies´
    #[test]
    fn small_exact_system_certifies() {
        let system =
            DenseSystem::new(2, vec![2.0, 0.0, 0.0, 3.0], 0.0, vec![2.0, 3.0], 0.0).expect("shape");
        let result = certified_solve(&system, 1.0e-10).expect("certificate");
        for value in result.candidate {
            assert!((value - 1.0).abs() <= result.radius);
        }
    }

    /// A singular center cannot establish an approximate inverse.
    /// ´test:unit:singular-system-refuses´
    #[test]
    fn singular_system_refuses() {
        let system =
            DenseSystem::new(2, vec![1.0, 1.0, 1.0, 1.0], 0.0, vec![2.0, 2.0], 0.0).expect("shape");
        assert!(matches!(
            certified_solve(&system, 1.0),
            Err(NumericalFailure::CannotCertify)
        ));
    }

    /// A deliberately inadequate approximate inverse makes q reach one.
    /// ´test:unit:inverse-defect-refuses´
    #[test]
    fn inverse_defect_refuses() {
        let system =
            DenseSystem::new(2, vec![1.0, 0.0, 0.0, 1.0], 0.0, vec![1.0, 1.0], 0.0).expect("shape");
        assert!(matches!(
            certify_candidate(&system, 1.0, vec![1.0, 1.0], &[0.0; 4]),
            Err(NumericalFailure::CannotCertify)
        ));
    }

    /// A valid certificate refuses an unreasonably small budget.
    /// ´test:unit:small-budget-refuses´
    #[test]
    fn small_budget_refuses() {
        let system = DenseSystem::new(2, vec![2.0, 0.0, 0.0, 3.0], 1.0e-12, vec![2.0, 3.0], 0.0)
            .expect("shape");
        assert!(matches!(
            certified_solve(&system, 0.0),
            Err(NumericalFailure::BudgetExceeded { .. })
        ));
    }

    /// Input validation rejects incompatible shapes and non-finite values.
    /// ´test:unit:invalid-system-refuses´
    #[test]
    fn invalid_system_refuses() {
        assert!(matches!(
            DenseSystem::new(2, vec![1.0], 0.0, vec![1.0, 1.0], 0.0),
            Err(NumericalFailure::ShapeMismatch)
        ));
        assert!(matches!(
            DenseSystem::new(1, vec![f64::NAN], 0.0, vec![1.0], 0.0),
            Err(NumericalFailure::NonFinite)
        ));
        assert!(matches!(
            DenseSystem::new(1, vec![1.0], -1.0, vec![1.0], 0.0),
            Err(NumericalFailure::NonFinite)
        ));
    }
}
