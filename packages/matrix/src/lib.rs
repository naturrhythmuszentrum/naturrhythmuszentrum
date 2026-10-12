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
///
/// TODO ´todo:code:own-checked-block-dimensions-measures-and´: own checked block dimensions, measures and kinds with intrinsic input and computational uncertainties under rule:vision:heterogeneous-assembly and def:vision:intrinsic-uncertainty-companions.
#[derive(Debug, Clone)]
pub struct DenseSystem {
    /// Matrix order.
    ///
    /// TODO ´todo:code:retain-matrix-order-as-structural-metadata´: retain matrix order as structural metadata rather than a physical count under rule:vision:counts-and-metadata.
    n: usize,
    /// Matrix center in row-major order.
    ///
    /// TODO ´todo:code:attach-equation-over-variable-quotient-descriptors´: attach equation-over-variable quotient descriptors to matrix centers before backend storage under rule:vision:heterogeneous-assembly.
    a: Vec<f64>,
    /// Matrix perturbation radius.
    ///
    /// TODO ´todo:code:carry-matrix-input-bounds-as-intrinsic´: carry matrix input bounds as intrinsic uncertainty with the operator block quotient dimension and its own kind under rule:vision:uncertainty-unit-algebra.
    a_radius: f64,
    /// Right-hand side center.
    ///
    /// TODO ´todo:code:declare-right-hand-side-dimensions-canonical´: declare right-hand-side dimensions, canonical measures and affine difference roles under rule:vision:heterogeneous-assembly and rule:vision:measure-conversions.
    b: Vec<f64>,
    /// Right-hand side perturbation radius.
    ///
    /// TODO ´todo:code:carry-right-hand-side-bounds-with´: carry right-hand-side bounds with their own difference measure and deterministic uncertainty kind under rule:vision:uncertainty-unit-algebra.
    b_radius: f64,
}

impl DenseSystem {
    /// Validate a represented center and two declared perturbation bounds.
    /// This is an import boundary under rule:vision:imports-and-exactness.
    ///
    /// TODO ´todo:code:validate-block-quotients-measure-conversions-kinds´: validate block quotients, measure conversions, kinds and uncertainty descriptors before numerical import under req:vision:algebraic-unit-checks and rule:vision:uncertainty-unit-algebra.
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
    ///
    /// TODO ´todo:code:expose-matrix-centers-through-an-explicit´: expose matrix centers through an explicit raw or quantity-aware boundary that retains owner evidence under dec:vision:storage-and-scalar-wrapping.
    #[must_use]
    pub fn matrix(&self) -> &[f64] {
        &self.a
    }
    /// Represented right-hand side center.
    ///
    /// TODO ´todo:code:expose-right-hand-side-centers-with´: expose right-hand-side centers with an explicit relation to their quantity and uncertainty descriptors under dec:vision:storage-and-scalar-wrapping.
    #[must_use]
    pub fn right_hand_side(&self) -> &[f64] {
        &self.b
    }
}

/// Numerical evidence accompanying a successful solve.
///
/// TODO ´todo:code:make-report-bounds-views-of-intrinsic´: make report bounds views of intrinsic matrix and solution companions with independently derived descriptors under def:vision:intrinsic-uncertainty-companions and rule:vision:uncertainty-unit-algebra.
#[derive(Debug, Clone, Copy)]
pub struct Report {
    /// Upper bound on the Frobenius norm of identity minus approximate inverse times matrix.
    ///
    /// TODO ´todo:code:type-inverse-defect-bounds-as-dimensionless´: type inverse-defect bounds as dimensionless uncertainty quantities rather than counts under def:vision:certified-solve-contract and rule:vision:uncertainty-unit-algebra.
    pub q: f64,
    /// Upper bound on the inverse norm under the Neumann argument.
    ///
    /// TODO ´todo:code:type-inverse-norm-bounds-with-reciprocal´: type inverse-norm bounds with reciprocal operator dimensions or the declared normalized map under def:vision:certified-solve-contract and rule:vision:uncertainty-unit-algebra.
    pub g: f64,
    /// Upper bound on the residual norm.
    ///
    /// TODO ´todo:code:type-residual-bounds-in-the-equation´: type residual bounds in the equation difference measure with a computational uncertainty kind under def:vision:certified-solve-contract and rule:vision:uncertainty-unit-algebra.
    pub rho: f64,
    /// Upper bound on the candidate norm.
    ///
    /// TODO ´todo:code:type-candidate-norm-bounds-in-the´: type candidate-norm bounds in the solution difference measure or declared normalized coordinates under def:vision:certified-solve-contract and rule:vision:uncertainty-unit-algebra.
    pub xnorm: f64,
    /// Declared matrix input radius.
    ///
    /// TODO ´todo:code:reference-the-matrix-companion-bound-without´: reference the matrix companion bound without losing its quotient dimension, measure or uncertainty kind under def:vision:intrinsic-uncertainty-companions and rule:vision:uncertainty-unit-algebra.
    pub a_radius: f64,
    /// Declared right-hand-side input radius.
    ///
    /// TODO ´todo:code:reference-the-right-hand-side-companion´: reference the right-hand-side companion bound with its independent difference descriptor under def:vision:intrinsic-uncertainty-companions and rule:vision:uncertainty-unit-algebra.
    pub b_radius: f64,
    /// Requested relative radius.
    ///
    /// TODO ´todo:code:retain-relative-budgets-as-dimensionless-ratios´: retain relative budgets as dimensionless ratios tied to declared compatible scales under dec:vision:accuracy-policy and rule:vision:uncertainty-unit-algebra.
    pub relative_budget: f64,
    /// Matrix order.
    pub n: usize,
}

/// A candidate with an independently certified Euclidean error radius.
///
/// TODO ´todo:code:return-solution-values-and-all-supported´: return solution values and all supported measurement and computational uncertainty contents together under def:vision:intrinsic-uncertainty-companions.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// Candidate solution.
    ///
    /// TODO ´todo:code:restore-solution-quotient-dimensions-kinds-measures´: restore solution quotient dimensions, kinds, measures and affine origins from checked coordinates under rule:vision:heterogeneous-assembly and rule:vision:measure-conversions.
    pub candidate: Vec<f64>,
    /// Absolute Euclidean error radius.
    ///
    /// TODO ´todo:code:store-the-certified-solution-radius-with´: store the certified solution radius with its own difference measure, derived dimension and uncertainty kind under rule:vision:uncertainty-unit-algebra.
    pub radius: f64,
    /// Certificate diagnostics.
    pub report: Report,
}

/// A refusal to claim a requested certified solve.
///
/// TODO ´todo:code:report-dynamic-dimension-measure-and-kind´: report dynamic dimension, measure and kind failures separately from shape and arithmetic failures under req:vision:algebraic-unit-checks.
#[derive(Debug, thiserror::Error)]
pub enum NumericalFailure {
    /// The approximate-inverse conditions do not establish a certificate.
    #[error("the approximate inverse cannot certify this system")]
    CannotCertify,
    /// A valid radius exceeds the requested relative budget.
    ///
    /// TODO ´todo:code:retain-distinct-certified-radius-and-relative´: retain distinct certified-radius and relative-ratio descriptors in budget failures under dec:vision:accuracy-policy and rule:vision:uncertainty-unit-algebra.
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
///
/// TODO ´todo:code:preserve-the-interval-bound-dimension-and´: preserve the interval-bound dimension and uncertainty kind when deriving a magnitude under rule:vision:uncertainty-unit-algebra.
const fn magnitude(bound: Interval<f64>) -> f64 {
    bound.lo().abs().max(bound.hi().abs())
}

/// Upper bound on the Frobenius norm of identity minus approximate inverse times matrix.
///
/// TODO ´todo:code:check-reciprocal-operator-products-and-typed´: check reciprocal operator products and typed identity blocks before computing dimensionless defect bounds under req:vision:algebraic-unit-checks and rule:vision:heterogeneous-assembly.
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
///
/// TODO ´todo:code:check-equation-compatible-residual-terms-and´: check equation-compatible residual terms and derive their computational uncertainty descriptors under req:vision:algebraic-unit-checks and rule:vision:uncertainty-unit-algebra.
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
///
/// TODO ´todo:code:propagate-carried-input-and-computational-companions´: propagate carried input and computational companions through the solve bound with algebraically checked dimensions and kinds under def:vision:certified-solve-contract and rule:vision:uncertainty-unit-algebra.
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
/// TODO ´todo:code:gate-solves-on-checked-quantity-descriptors´: gate solves on checked quantity descriptors and ratio budgets and return intrinsic uncertainty contents under req:vision:algebraic-unit-checks and def:vision:intrinsic-uncertainty-companions.
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
///
/// TODO ´todo:code:test-static-and-dynamic-block-quotients´: test static and dynamic block quotients, normalized bounds and intrinsic uncertainty modes under rule:vision:heterogeneous-assembly and rule:vision:uncertainty-unit-algebra.
mod tests {
    use super::{DenseSystem, NumericalFailure, certified_solve, certify_candidate};

    /// A small exact diagonal system admits a certified all-ones solution.
    ///
    /// TODO ´todo:code:certify-a-dimensioned-system-with-intrinsic´: certify a dimensioned system with intrinsic input and computational companions and correctly typed solution bounds under def:vision:certified-solve-contract and rule:vision:uncertainty-unit-algebra.
    ///
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
    ///
    /// TODO ´todo:code:preserve-unit-descriptors-and-carried-uncertainty´: preserve unit descriptors and carried uncertainty when a dimensioned singular system refuses certification under def:vision:intrinsic-uncertainty-companions.
    ///
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
    ///
    /// TODO ´todo:code:test-dimensionless-defect-bound-kinds-and´: test dimensionless defect-bound kinds and reject wrongly dimensioned inverse or identity blocks under req:vision:algebraic-unit-checks and rule:vision:uncertainty-unit-algebra.
    ///
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
    ///
    /// TODO ´todo:code:test-budget-refusal-across-equivalent-measures´: test budget refusal across equivalent measures with absolute difference bounds and relative ratios kept distinct under dec:vision:accuracy-policy and rule:vision:uncertainty-unit-algebra.
    ///
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
    ///
    /// TODO ´todo:code:reject-incompatible-block-dimensions-affine-roles´: reject incompatible block dimensions, affine roles and uncertainty descriptors before backend calls under rule:vision:heterogeneous-assembly and rule:vision:uncertainty-unit-algebra.
    ///
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
