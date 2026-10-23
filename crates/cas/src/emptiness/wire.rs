// SPDX-License-Identifier: AGPL-3.0-only
//! Closed forward-versioned tags retain every node, evidence edge and referenced object.

use super::{
    AT, EmptinessDag, EmptyEvidence, EmptyFact, EmptyFactor, EmptyFactorIdentity, EmptyLeaf,
    EmptyNode,
};
use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::error::Fault;
use crate::graph::{Cas, Handle, size};
use crate::wire_decode::Decoded;
use crate::wire_read::{Reader, invalid};

/// Encode four-byte collection counts and eight-byte bounded slots at their matching reader widths.
pub(crate) fn encode(
    dag: &EmptinessDag,
    out: &mut Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    out.count(dag.nodes.len())?;
    for node in &dag.nodes {
        write(&node.region, out)?;
        match &node.evidence {
            EmptyEvidence::Split { cover, children } => {
                out.byte(0)?;
                out.unsigned(u64::from(*cover))?;
                out.count(children.len())?;
                for child in children {
                    out.unsigned(u64::from(*child))?;
                }
            }
            EmptyEvidence::Leaf(leaf) => {
                out.byte(1)?;
                encode_leaf(leaf, out, write)?;
            }
            EmptyEvidence::Lemma { premise } => {
                out.byte(2)?;
                out.unsigned(u64::from(*premise))?;
            }
        }
    }
    Ok(())
}

/// A fact reference preserves either a checked local premise slot or an exact ancestral guard.
fn encode_fact(
    fact: &EmptyFact,
    out: &mut Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    match fact {
        EmptyFact::Premise(slot) => {
            out.byte(0)?;
            out.unsigned(u64::from(*slot))
        }
        EmptyFact::Guard(region) => {
            out.byte(1)?;
            write(region, out)
        }
    }
}

/// Factor order, positive multiplicities and the independent identity premise are all retained.
fn encode_leaf(
    leaf: &EmptyLeaf,
    out: &mut Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    match leaf {
        EmptyLeaf::Sign { subject, nonzero } => {
            out.byte(0)?;
            write(subject, out)?;
            encode_fact(nonzero, out, write)
        }
        EmptyLeaf::FactorIdentity(value) => {
            out.byte(1)?;
            for object in [&value.subject, &value.product, &value.scalar] {
                write(object, out)?;
            }
            encode_fact(&value.scalar_nonzero, out, write)?;
            out.count(value.factors.len())?;
            for factor in &value.factors {
                write(&factor.subject, out)?;
                out.unsigned(u64::from(factor.multiplicity))?;
                encode_fact(&factor.nonzero, out, write)?;
            }
            out.unsigned(u64::from(value.identity))
        }
    }
}

/// Prepay all retained node and vector copies, and authenticate every embedded handle.
pub(crate) fn measure(cas: &Cas, dag: &EmptinessDag, budget: &mut Budget) -> Result<(), Fault> {
    budget.check(BudgetKind::ProofNodes, size(dag.nodes.len(), AT)?, AT)?;
    let mut out = Encoder::new(budget, AT);
    encode(dag, &mut out, &mut |handle, out| {
        cas.checked(handle)?;
        crate::statement::handle(handle, out)
    })?;
    let bytes = size(out.finish().len(), AT)?.saturating_mul(32);
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(bytes, AT)
}

/// Decode a bounded topological node vector; the kernel checks edges and mathematical content.
pub(crate) fn decode(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<EmptinessDag, Fault> {
    let count = reader.count(BudgetKind::ProofNodes, 9, 128)?;
    let mut nodes = Vec::new();
    nodes
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        let region = crate::wire_decode::object(reader, records)?;
        let evidence = match reader.byte()? {
            0 => {
                let cover = reader.small()?;
                let count = reader.count(BudgetKind::ProofNodes, 8, 4)?;
                let mut children = Vec::new();
                children
                    .try_reserve_exact(count)
                    .map_err(|_| Fault::allocation(AT))?;
                for _ in 0..count {
                    children.push(reader.small()?);
                }
                EmptyEvidence::Split { cover, children }
            }
            1 => EmptyEvidence::Leaf(Box::new(decode_leaf(reader, records)?)),
            2 => EmptyEvidence::Lemma {
                premise: reader.small()?,
            },
            _ => return Err(invalid()),
        };
        nodes.push(EmptyNode { region, evidence });
    }
    Ok(EmptinessDag { nodes })
}

/// Refuse unknown fact tags before using any referenced premise.
fn decode_fact(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<EmptyFact, Fault> {
    match reader.byte()? {
        0 => Ok(EmptyFact::Premise(reader.small()?)),
        1 => Ok(EmptyFact::Guard(crate::wire_decode::object(
            reader, records,
        )?)),
        _ => Err(invalid()),
    }
}

/// Each leaf tag has one exact finite schema; no opaque producer evidence is admitted.
fn decode_leaf(reader: &mut Reader<'_, '_>, records: &[Decoded]) -> Result<EmptyLeaf, Fault> {
    match reader.byte()? {
        0 => Ok(EmptyLeaf::Sign {
            subject: crate::wire_decode::object(reader, records)?,
            nonzero: decode_fact(reader, records)?,
        }),
        1 => {
            let subject = crate::wire_decode::object(reader, records)?;
            let product = crate::wire_decode::object(reader, records)?;
            let scalar = crate::wire_decode::object(reader, records)?;
            let scalar_nonzero = decode_fact(reader, records)?;
            let count = reader.count(BudgetKind::PolynomialTerms, 17, 96)?;
            let mut factors = Vec::new();
            factors
                .try_reserve_exact(count)
                .map_err(|_| Fault::allocation(AT))?;
            for _ in 0..count {
                factors.push(EmptyFactor {
                    subject: crate::wire_decode::object(reader, records)?,
                    multiplicity: reader.small()?,
                    nonzero: decode_fact(reader, records)?,
                });
            }
            Ok(EmptyLeaf::FactorIdentity(Box::new(EmptyFactorIdentity {
                subject,
                product,
                scalar,
                scalar_nonzero,
                factors,
                identity: reader.small()?,
            })))
        }
        _ => Err(invalid()),
    }
}
