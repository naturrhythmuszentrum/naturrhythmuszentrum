// SPDX-License-Identifier: AGPL-3.0-only
//! Mathematical declarations contain shapes only; physical parameter meaning is sealed beside them.

use super::AdmissionError;
use nrz_cas::{
    Cas, Handle, Reference,
    domain::HeaderDraft,
    limits::Budget,
    syntax::{
        AtomicSort, Declaration, Draft, Environment, ExpressionDraft, Extent, Field, Operator, Sort,
    },
};

/// Admit one shape-only parameter declaration without inventing physical CAS node fields.
pub(super) fn declare(
    shape: [u32; 2],
    budget: &mut Budget,
) -> Result<(Cas, Handle), AdmissionError> {
    let mut graph = Cas::new(budget.limits().clone())?;
    let sort = Sort::Value(AtomicSort::Matrix {
        field: Field::Real,
        rows: Extent::Literal(shape[0]),
        columns: Extent::Literal(shape[1]),
    });
    let mut nodes = graph.admit(
        &[
            Draft::Header(HeaderDraft::Core(Environment {
                declarations: vec![Declaration {
                    key: "parameter".into(),
                    display_name: "parameter".into(),
                    sort: sort.clone(),
                }],
                ..Environment::default()
            })),
            Draft::Expression(ExpressionDraft {
                space: None,
                operator: Operator::Free("parameter".into()),
                sort,
                header: Reference::Local(0),
                scope: Vec::new(),
                binding: None,
                operands: Vec::new(),
            }),
        ],
        budget,
    )?;
    let node = nodes.pop().ok_or(AdmissionError::ResourceLimit)?;
    Ok((graph, node))
}
