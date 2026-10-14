// SPDX-License-Identifier: AGPL-3.0-only
//! The sole current typed witness language and internal scalar theorem keys.

use crate::budget::Budget;
use crate::encoding::{self, Encoder};
use crate::error::Fault;
use crate::graph::Cas;
use crate::scalar_model::{Claim, Relation};

pub(crate) use crate::wire_build::encode;

/// Encode one fixed-width u32 local reference.
pub(crate) fn index(value: u32, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.raw(&value.to_le_bytes())
}

/// Encode the exact admitted relation and ordered variable payload.
pub(crate) fn encode_relation(value: &Relation, out: &mut Encoder<'_>) -> Result<(), Fault> {
    match value {
        Relation::EqualOnDomain => out.byte(0),
        Relation::Derivative { variable } => {
            out.byte(1)?;
            out.name(variable)
        }
        Relation::SquareFree { variable, field } => {
            out.byte(3)?;
            out.name(variable)?;
            encoding::field(field, out)
        }
        Relation::Jacobian { variables } => {
            out.byte(2)?;
            out.count(variables.len())?;
            for variable in variables {
                out.name(variable)?;
            }
            Ok(())
        }
    }
}

/// Include the full scoped typed theorem in scalar search/cache comparisons.
pub(crate) fn claim_key(cas: &Cas, claim: &Claim, budget: &mut Budget) -> Result<Vec<u8>, Fault> {
    let judgment = crate::proof_view::typed_claim(cas, claim, budget)?;
    crate::wire_build::judgment_key(cas, &judgment, budget)
}
