// SPDX-License-Identifier: AGPL-3.0-only
//! Exact ordered lattice roles in the existing portable statement language.

use super::LatticeCover;
use crate::budget::BudgetKind;
use crate::encoding::Encoder;
use crate::error::Fault;
use crate::graph::Handle;
use crate::lattice::{LatticeBasisQuestion, LatticeIndexQuestion};
use crate::wire_decode::{Decoded, object};
use crate::wire_read::Reader;

/// Every original identity is an ordinary graph dependency.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the statement kernel and portable replay share these crate-private lattice hooks"
)]
pub(crate) fn objects(value: &LatticeCover) -> Vec<&Handle> {
    let mut result = vec![
        &value.index.basis.context,
        &value.index.basis.basis,
        &value.index.basis.offset,
        &value.index.coordinate_domain,
        &value.forward,
        &value.inverse,
        &value.determinant,
    ];
    for family in &value.index.families {
        result.extend(crate::decision_family_wire::objects(family));
    }
    result
}

/// Encode the original context, ordered factors and affine evidence without cell enumeration.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the statement kernel and portable replay share these crate-private lattice hooks"
)]
pub(crate) fn encode(
    value: &LatticeCover,
    out: &mut Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    for object in [
        &value.index.basis.context,
        &value.index.basis.basis,
        &value.index.basis.offset,
        &value.index.coordinate_domain,
        &value.forward,
        &value.inverse,
        &value.determinant,
    ] {
        write(object, out)?;
    }
    out.count(value.index.families.len())?;
    for family in &value.index.families {
        crate::decision_family_wire::encode(family, out, write)?;
    }
    Ok(())
}

/// Bound the finite binder inventory before allocating decoded factors.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the statement kernel and portable replay share these crate-private lattice hooks"
)]
pub(crate) fn decode(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<LatticeCover, Fault> {
    let context = object(reader, records)?;
    let basis = object(reader, records)?;
    let offset = object(reader, records)?;
    let coordinate_domain = object(reader, records)?;
    let forward = object(reader, records)?;
    let inverse = object(reader, records)?;
    let determinant = object(reader, records)?;
    let count = reader.count(BudgetKind::ProofNodes, 6, 512)?;
    let mut families = Vec::new();
    families
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(super::AT))?;
    for _ in 0..count {
        families.push(crate::decision_family_wire::decode(reader, records)?);
    }
    Ok(LatticeCover {
        index: LatticeIndexQuestion {
            basis: LatticeBasisQuestion {
                context,
                basis,
                offset,
            },
            families,
            coordinate_domain,
        },
        forward,
        inverse,
        determinant,
    })
}
