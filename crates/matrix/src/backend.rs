// SPDX-License-Identifier: AGPL-3.0-only
//! Safe faer adapter for candidate computation only.

use faer::{Mat, prelude::*};

use crate::DenseSystem;

/// Solve the right-hand side and all identity columns with one partially pivoted LU factorization.
///
/// TODO ´todo:code:keep-physical-right-hand-side-and´: keep physical right-hand-side and identity-column descriptors distinct when batching candidates and inverses through normalized views under rule:vision:heterogeneous-assembly and req:vision:powers-and-factor-dimensions.
///
/// TODO ´todo:code:separate-physical-solution-columns-from-inverse´: separate physical solution columns from inverse identity columns while retaining normalized factor permutations and distinct output axes under rule:nrz:shape-solves-and-inverses and rule:nrz:shape-factorizations.
///
/// TODO ´todo:code:borrow-validated-arena-views-or-checked´: borrow validated arena views or checked packing buffers before backend construction under rule:nrz:descriptor-layout-validation.
///
/// TODO ´todo:code:separate-reusable-operator-factors-from-point´: separate reusable operator factors from point specific solve candidates under rule:nrz:computation-cache-validity.
pub(super) fn candidate_and_inverse(system: &DenseSystem) -> (Vec<f64>, Vec<f64>) {
    let n = system.n;
    let matrix = Mat::<f64>::from_fn(n, n, |row, col| system.a[row * n + col]);
    let rhs = Mat::<f64>::from_fn(n, n + 1, |row, col| {
        if col == 0 {
            system.b[row]
        } else if row + 1 == col {
            1.0
        } else {
            0.0
        }
    });
    let lu = matrix.partial_piv_lu();
    let solved = lu.solve(&rhs);
    let candidate = (0..n).map(|row| solved[(row, 0)]).collect();
    let mut inverse = Vec::with_capacity(n * n);
    for row in 0..n {
        for col in 0..n {
            inverse.push(solved[(row, col + 1)]);
        }
    }
    (candidate, inverse)
}
