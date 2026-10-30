// SPDX-License-Identifier: AGPL-3.0-only
//! Public mathematical prerequisites for physical lattice question tests.

use super::decision_fixture::ledger;
use crate::{
    binding_testing::TestResult,
    reading_testing::{Fixture, matrix, real},
};
use nrz_bindings::readings::algebra::{
    Handle, Rational, Reference,
    decision::{IntegerCellRange, IntegerFamily},
    domain::{AmbientDomain, CaseArmDraft, CasesDraft, RegionDraft, RegionExpr},
    lattice::{
        CheckedLatticeBasis, CheckedLatticeIndexDomain, LatticeBasisOutcome, LatticeBasisQuestion,
        LatticeIndexQuestion,
    },
    proof::{ApplicationDraft, Certificate, Primitive, Statement, VerifiedWitness},
    syntax::{AtomicSort, Constant, Draft, ExpressionDraft, Operator, PredicateKind, Sort},
};
use std::rc::Rc;

/// One original mathematical coordinate frame with admitted affine and index prerequisites.
pub(super) struct LatticeFixture {
    /// Original public CAS context, shared by the target and physical normalization.
    pub(super) frame: Fixture,
    /// Checked nonsingularity and both inverse identities.
    pub(super) basis: Rc<CheckedLatticeBasis>,
    /// Checked unbounded Cartesian index product.
    pub(super) index: Rc<CheckedLatticeIndexDomain>,
    /// Original target column in the normalized frame.
    pub(super) target: Handle,
}

/// Phase, two-angle and rational-grid questions exercise one and two physical coordinates.
pub(super) fn lattice(kind: u8) -> TestResult<LatticeFixture> {
    let dimension = if kind == 0 { 1 } else { 2 };
    let names = ["q0", "q1"];
    let indices = ["k0", "k1"];
    let declarations = (0..dimension)
        .flat_map(|i| {
            [
                (names[i], real()),
                (indices[i], Sort::Value(AtomicSort::Integer)),
            ]
        })
        .collect::<Vec<_>>();
    let mut frame = Fixture::new(&declarations)?;
    let mut families = Vec::new();
    let mut proofs = Vec::new();
    for i in 0..dimension {
        let (family, proof) = floor(&mut frame, names[i], indices[i])?;
        families.push(family);
        proofs.push(proof);
    }
    let zero = number(&mut frame, "0")?;
    let period = if kind == 2 {
        number(&mut frame, "1/8")?
    } else {
        let two = number(&mut frame, "2")?;
        let pi = term(&mut frame, Operator::Constant(Constant::Pi), real(), &[])?;
        term(&mut frame, Operator::Mul, real(), &[two, pi])?
    };
    let mut coefficients = vec![period.clone()];
    if dimension == 2 {
        let second = if kind == 2 {
            number(&mut frame, "1/16")?
        } else {
            period
        };
        coefficients.extend([zero.clone(), zero.clone(), second]);
    }
    let shape = u32::try_from(dimension)?;
    let basis_node = column_or_matrix(&mut frame, shape, shape, &coefficients)?;
    let offsets = vec![zero; dimension];
    let offset = column_or_matrix(&mut frame, shape, 1, &offsets)?;
    let question = LatticeBasisQuestion {
        context: frame.context.clone(),
        basis: basis_node,
        offset,
    };
    let LatticeBasisOutcome::Admitted(basis) =
        frame.cas.admit_lattice_basis(&question, &mut ledger()?)?
    else {
        return Err("unresolved fixture basis".into());
    };
    let domain = region(
        &mut frame,
        RegionExpr::Meet(
            families
                .iter()
                .map(|family| Reference::Existing(family.domain.clone()))
                .collect(),
        ),
    )?;
    let index = frame.cas.admit_lattice_index_domain(
        &basis,
        &LatticeIndexQuestion {
            basis: question,
            families,
            coordinate_domain: domain,
        },
        &proofs,
        &mut ledger()?,
    )?;
    let entries = if kind == 0 {
        let five = number(&mut frame, "5/2")?;
        let pi = term(&mut frame, Operator::Constant(Constant::Pi), real(), &[])?;
        vec![term(&mut frame, Operator::Mul, real(), &[five, pi])?]
    } else if kind == 1 {
        vec![number(&mut frame, "20")?, number(&mut frame, "10")?]
    } else {
        vec![number(&mut frame, "3/10")?, number(&mut frame, "1/5")?]
    };
    let target = column_or_matrix(&mut frame, shape, 1, &entries)?;
    Ok(LatticeFixture {
        frame,
        basis: Rc::from(basis),
        index: Rc::new(index),
        target,
    })
}

/// Check one existing unbounded scalar floor family in the common ambient context.
fn floor(
    frame: &mut Fixture,
    coordinate: &str,
    index: &str,
) -> TestResult<(IntegerFamily, VerifiedWitness)> {
    let q = frame.symbol(coordinate)?;
    let k = frame.symbol(index)?;
    let embedded = term(
        frame,
        Operator::IntegerEmbed,
        real(),
        std::slice::from_ref(&k),
    )?;
    let one = number(frame, "1")?;
    let end = term(frame, Operator::Add, real(), &[embedded.clone(), one])?;
    let lower = term(frame, Operator::Sub, real(), &[q.clone(), embedded])?;
    let upper = term(frame, Operator::Sub, real(), &[end, q.clone()])?;
    let lower = guard(frame, PredicateKind::NonNegative, lower)?;
    let upper = guard(frame, PredicateKind::Positive, upper)?;
    let relation = region(
        frame,
        RegionExpr::Meet(vec![Reference::Existing(lower), Reference::Existing(upper)]),
    )?;
    let header = frame.cas.context_header(&frame.context)?;
    let nodes = frame.cas.admit(
        &[
            Draft::CaseArm(CaseArmDraft {
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                placements: Vec::new(),
                region: Reference::Existing(relation.clone()),
                value: Reference::Existing(k.clone()),
            }),
            Draft::Cases(CasesDraft {
                sort: Sort::Value(AtomicSort::Integer),
                header: Reference::Existing(header),
                scope: Vec::new(),
                domain: Reference::Existing(relation),
                arms: vec![Reference::Local(0)],
            }),
        ],
        &mut ledger()?,
    )?;
    let family = IntegerFamily {
        function: nodes.last().ok_or("missing cases")?.clone(),
        coordinate: q.clone(),
        index: k,
        domain: frame.cas.admissible_region(&frame.context)?,
        lower: None,
        upper: None,
        count: Some(Box::new(IntegerCellRange {
            coordinate: q,
            lower: None,
            upper: None,
        })),
    };
    let application = ApplicationDraft {
        rule: Primitive::RegionDecide,
        context: frame.context.clone(),
        conclusion: Statement::IntegerFamily(Box::new(family.clone())),
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    };
    let proof = frame
        .cas
        .check_proof(&frame.context, &[application], &mut ledger()?)?;
    Ok((family, proof))
}

/// A checked closed scalar literal, retaining the original declaration header.
fn number(frame: &mut Fixture, value: &str) -> TestResult<Handle> {
    term(
        frame,
        Operator::Rational(Rational::parse(value, &mut ledger()?)?),
        real(),
        &[],
    )
}

/// Explicit original row-major matrix syntax.
fn column_or_matrix(
    frame: &mut Fixture,
    rows: u32,
    columns: u32,
    entries: &[Handle],
) -> TestResult<Handle> {
    term(
        frame,
        Operator::Matrix { rows, columns },
        matrix(rows, columns),
        entries,
    )
}

/// Original typed scalar or matrix expression through public CAS admission.
pub(super) fn term(
    frame: &mut Fixture,
    operator: Operator,
    sort: Sort,
    operands: &[Handle],
) -> TestResult<Handle> {
    let header = frame.cas.context_header(&frame.context)?;
    let draft = Draft::Expression(ExpressionDraft {
        space: None,
        operator,
        sort,
        header: Reference::Existing(header),
        scope: Vec::new(),
        binding: None,
        operands: operands.iter().cloned().map(Reference::Existing).collect(),
    });
    frame
        .cas
        .admit(&[draft], &mut ledger()?)?
        .pop()
        .ok_or_else(|| "missing expression".into())
}

/// Native region under the same complete assignment frame.
fn region(frame: &mut Fixture, expression: RegionExpr) -> TestResult<Handle> {
    let header = frame.cas.context_header(&frame.context)?;
    let draft = Draft::Region(RegionDraft {
        header: Reference::Existing(header),
        scope: Vec::new(),
        ambient: AmbientDomain::Assignments,
        expression,
    });
    frame
        .cas
        .admit(&[draft], &mut ledger()?)?
        .pop()
        .ok_or_else(|| "missing region".into())
}

/// Exact half-open ownership predicates for the existing scalar cover theorem.
fn guard(frame: &mut Fixture, kind: PredicateKind, value: Handle) -> TestResult<Handle> {
    let header = frame.cas.context_header(&frame.context)?;
    let draft = Draft::Condition {
        header: Reference::Existing(header),
        scope: Vec::new(),
        kind,
        arguments: vec![Reference::Existing(value)],
    };
    let predicate = frame
        .cas
        .admit(&[draft], &mut ledger()?)?
        .pop()
        .ok_or("missing guard")?;
    let base = frame.cas.admissible_region(&frame.context)?;
    region(
        frame,
        RegionExpr::GuardTrue {
            base: Reference::Existing(base),
            predicate: Reference::Existing(predicate),
        },
    )
}
