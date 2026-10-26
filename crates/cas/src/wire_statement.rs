// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded typed statement and certificate payload reads.

use crate::budget::BudgetKind;
use crate::domain_model::{HypothesisRef, Side};
use crate::error::{Fault, Location};
use crate::proof_model::{Certificate, ConstructorLaw, DomainContract, Ownership, Statement};
use crate::wire_decode::{Decoded, object, objects};
use crate::wire_read::{Reader, invalid};

/// Bounded payload allocation location.
const AT: Location = Location::new("typed statement decoding");

/// Exact oriented-side tags never depend on enum layout.
fn side(reader: &mut Reader<'_, '_>) -> Result<Side, Fault> {
    match reader.byte()? {
        0 => Ok(Side::Below),
        1 => Ok(Side::Above),
        _ => Err(invalid()),
    }
}

/// Decode the representation; executable availability is checked separately by the kernel.
pub(crate) fn statement(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Statement, Fault> {
    Ok(match reader.byte()? {
        27 => Statement::IntegerSelection(Box::new(crate::decision_integer_wire::decode(
            reader, records,
        )?)),
        26 => Statement::IntegerFamily(Box::new(crate::decision_family_wire::decode(
            reader, records,
        )?)),
        25 => Statement::Solve(Box::new(crate::solving::SolveStatement::decode(
            reader, records,
        )?)),
        0 => Statement::Equality {
            left: object(reader, records)?,
            right: object(reader, records)?,
            domain: object(reader, records)?,
            domain_contract: match reader.byte()? {
                0 => DomainContract::DefinedAndEqual,
                1 => DomainContract::SameDefinedDomain,
                _ => return Err(invalid()),
            },
        },
        1 => Statement::PredicateFact {
            region: object(reader, records)?,
            predicate: object(reader, records)?,
        },
        2 => Statement::Subset {
            subset: object(reader, records)?,
            superset: object(reader, records)?,
        },
        3 => Statement::Disjoint {
            left: object(reader, records)?,
            right: object(reader, records)?,
        },
        4 => Statement::Cover {
            domain: object(reader, records)?,
            members: objects(reader, records, BudgetKind::RegionNodes)?,
        },
        5 => Statement::Empty {
            region: object(reader, records)?,
        },
        6 => Statement::Constructor {
            region: object(reader, records)?,
            law: match reader.byte()? {
                0 => ConstructorLaw::Membership,
                _ => return Err(invalid()),
            },
        },
        7 => Statement::DefinedDomain {
            function: object(reader, records)?,
            domain: object(reader, records)?,
        },
        8 => Statement::Compatibility {
            left: object(reader, records)?,
            right: object(reader, records)?,
            region: object(reader, records)?,
        },
        9 => Statement::Elaboration {
            source: object(reader, records)?,
            result: object(reader, records)?,
            domain: object(reader, records)?,
        },
        24 => Statement::Germ(Box::new(crate::germ_wire::decode_statement(
            reader, records,
        )?)),
        23 => Statement::Split(Box::new(crate::split_wire::decode_statement(
            reader, records,
        )?)),
        22 => Statement::Distribution(Box::new(crate::distribution_decode::statement(
            reader, records,
        )?)),
        tag => return advanced_statement(tag, reader, records),
    })
}

/// Decode function-indexed analytic statement representation without enabling its rules.
fn advanced_statement(
    tag: u8,
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Statement, Fault> {
    Ok(match tag {
        10 => Statement::Incidence {
            region: object(reader, records)?,
            boundary: object(reader, records)?,
            side: side(reader)?,
            neighborhood: object(reader, records)?,
        },
        11 => Statement::Ownership {
            function: object(reader, records)?,
            boundary: object(reader, records)?,
            value: match reader.byte()? {
                0 => Ownership::Present(object(reader, records)?),
                1 => Ownership::Absent,
                _ => return Err(invalid()),
            },
        },
        12 => Statement::DerivativeOn {
            function: object(reader, records)?,
            derivative: object(reader, records)?,
            coordinate: reader.name()?,
            domain: object(reader, records)?,
        },
        13 => {
            let input = object(reader, records)?;
            let output = object(reader, records)?;
            let count = reader.count(BudgetKind::PolynomialVariables, 4, 32)?;
            let mut variables = Vec::new();
            variables
                .try_reserve_exact(count)
                .map_err(|_| Fault::allocation(AT))?;
            for _ in 0..count {
                variables.push(reader.name()?);
            }
            Statement::Jacobian {
                input,
                output,
                variables,
                domain: object(reader, records)?,
            }
        }
        14 => Statement::SquareFree {
            input: object(reader, records)?,
            output: object(reader, records)?,
            variable: reader.name()?,
            field: reader.field()?,
            domain: object(reader, records)?,
        },
        15 => Statement::Jet {
            function: object(reader, records)?,
            region: object(reader, records)?,
            boundary: object(reader, records)?,
            side: side(reader)?,
            values: objects(reader, records, BudgetKind::JetEntries)?,
        },
        16 => Statement::Jump {
            function: object(reader, records)?,
            boundary: object(reader, records)?,
            order: reader.small()?,
            value: object(reader, records)?,
        },
        _ => return crate::boundary_decode::statement(tag, reader, records),
    })
}

/// Read simultaneous substitutions in their canonical declaration-key order.
pub(crate) fn substitutions(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Vec<crate::substitution::Substitution>, Fault> {
    let count = reader.count(BudgetKind::Vertices, 8, 96)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    let mut previous = String::new();
    for _ in 0..count {
        let key = reader.name()?;
        if key <= previous {
            return Err(invalid());
        }
        previous.clone_from(&key);
        result.push(crate::substitution::Substitution {
            key,
            replacement: object(reader, records)?,
        });
    }
    Ok(result)
}

/// Bound exact interval cells before any certificate vector is reserved.
pub(crate) fn region(
    reader: &mut Reader<'_, '_>,
) -> Result<crate::region::RegionCertificate, Fault> {
    let count = reader.count(BudgetKind::RegionCells, 9, 128)?;
    let cells = count
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(invalid)?;
    reader.budget.specialized(
        BudgetKind::RegionCells,
        crate::graph::size(cells, AT)?,
        false,
        AT,
    )?;
    let mut cuts = Vec::new();
    cuts.try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        cuts.push(reader.rational()?);
    }
    let count = reader.count(BudgetKind::RegionNodes, 4, 32)?;
    let mut membership = Vec::new();
    membership
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        let count = reader.count(BudgetKind::RegionCells, 1, 1)?;
        if count != cells {
            return Err(invalid());
        }
        let mut row = Vec::new();
        row.try_reserve_exact(count)
            .map_err(|_| Fault::allocation(AT))?;
        for _ in 0..count {
            row.push(reader.boolean()?);
        }
        membership.push(row);
    }
    Ok(crate::region::RegionCertificate { cuts, membership })
}

/// A certificate contains data, never an acceptance bit or unregistered theorem callback.
pub(crate) fn certificate(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<Certificate, Fault> {
    Ok(match reader.byte()? {
        17 => Certificate::IntegerSelection(Box::new(
            crate::decision_integer_wire::decode_certificate(reader, records)?,
        )),
        tag @ (15 | 16) => crate::decision_wire::decode_sign(tag, reader, records)?,
        14 => Certificate::Decision(Box::new(crate::decision_wire::decode(reader, records)?)),
        13 => Certificate::Emptiness(Box::new(crate::emptiness::decode(reader, records)?)),
        12 => Certificate::Solve(Box::new(crate::solving::SolveCertificate::decode(
            reader, records,
        )?)),
        0 => Certificate::None,
        1 => Certificate::Region(region(reader)?),
        2 => Certificate::Hypothesis(HypothesisRef {
            context: object(reader, records)?,
            slot: reader.small()?,
        }),
        3 => Certificate::Close {
            frame: object(reader, records)?,
        },
        4 => {
            let count = reader.count(BudgetKind::Depth, 8, 8)?;
            let mut path = Vec::new();
            path.try_reserve_exact(count)
                .map_err(|_| Fault::allocation(AT))?;
            for _ in 0..count {
                path.push(reader.small()?);
            }
            Certificate::Congruence(path)
        }
        5 => Certificate::SquareFree(Box::new(
            crate::multiplicity::SquareFreeCertificate::decode(reader)?,
        )),
        6 => Certificate::Cases(Box::new(case_certificate(reader, records)?)),
        11 => Certificate::Pullback(Box::new(crate::pullback_wire::decode_certificate(reader)?)),
        10 => Certificate::Germ(Box::new(crate::germ_wire::decode_certificate(reader)?)),
        9 => Certificate::Split(Box::new(crate::split_wire::decode_certificate(reader)?)),
        8 => Certificate::Distribution(Box::new(crate::distribution_decode::certificate(
            reader, records,
        )?)),
        7 => Certificate::Boundary(Box::new(crate::boundary_decode::certificate(
            reader, records,
        )?)),
        _ => return Err(invalid()),
    })
}

/// Decode every case vector under the corresponding semantic ceiling before allocation.
fn case_certificate(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<crate::case_model::CaseCertificate, Fault> {
    let regions = objects(reader, records, BudgetKind::RegionNodes)?;
    let arms = objects(reader, records, BudgetKind::CaseArms)?;
    let frames = objects(reader, records, BudgetKind::LocalContexts)?;
    let count = reader.count(BudgetKind::ProofNodes, 8, 64)?;
    let mut cells = Vec::new();
    cells
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        cells.push(region(reader)?);
    }
    Ok(crate::case_model::CaseCertificate {
        regions,
        arms,
        frames,
        cells,
    })
}
