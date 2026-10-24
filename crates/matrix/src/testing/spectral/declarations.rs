// SPDX-License-Identifier: AGPL-3.0-only
//! Scalar physical input declarations and their separate original total/error parameters.

use nrz_bindings::readings::algebra::syntax::{AtomicSort, Declaration, Extent, Field, Sort};

/// Every value/parameter symbol is declared before original proof construction or heap execution.
pub(super) fn inventory() -> Vec<Declaration> {
    let matrix = |rows| {
        Sort::Value(AtomicSort::Matrix {
            field: Field::Real,
            rows: Extent::Literal(rows),
            columns: Extent::Literal(1),
        })
    };
    let mut result = Vec::new();
    for prefix in ["inductance", "capacitance"] {
        result.push(Declaration {
            key: format!("{prefix}_stored"),
            display_name: prefix.into(),
            sort: matrix(1),
        });
        for channel in ["total", "error"] {
            for suffix in ["source", "output"] {
                let key = format!("{prefix}_{channel}_{suffix}");
                result.push(Declaration {
                    display_name: key.clone(),
                    key,
                    sort: matrix(1),
                });
            }
        }
    }
    result
}
