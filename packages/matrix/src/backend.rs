// SPDX-License-Identifier: AGPL-3.0-only
//! Safe faer adapter for candidate computation only.

use faer::{Mat, prelude::*};

use crate::DenseSystem;

/// Solve the right-hand side and all identity columns with one partially pivoted LU factorization.
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
