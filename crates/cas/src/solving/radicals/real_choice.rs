// SPDX-License-Identifier: AGPL-3.0-only
//! Named real witnesses for computed ground binomials, followed by checked complex inclusion.

use super::construction::{Construction, admit};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Draft, ExpressionDraft, Handle, Reference};
use crate::model::{AtomicSort, Field, Operator, PredicateKind, Sort};
use crate::number::Rational;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::model::CoefficientFailure;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{invalid, vector};
use crate::solving::towers::{dense::Dense, materialize};
use crate::solving::{
    AdjunctionMode, CertificateRootAdjunction, CertificateRootSelectionRefinement,
    CoefficientDomainData, CoefficientMapData, MapMode, MapTarget, RefinementMode,
    RefinementTargetKind, RootRelationData, RootSelectionData, SelectionMode, SolveCertificate,
    SolveDraft, SolveSchema, SolveStatement, StatementRootExistence,
};

/// The actual dense coefficients must be a monic ground binomial of an admitted real-root order.
fn candidate(
    cas: &Cas,
    source: &Dense,
    budget: &mut Budget,
) -> Result<Option<(Rational, u32)>, Fault> {
    let degree = u32::try_from(
        source
            .coefficients
            .len()
            .checked_sub(1)
            .ok_or_else(invalid)?,
    )
    .map_err(|_| invalid())?;
    if degree < 2 || (degree.is_multiple_of(2) && degree != 2) {
        return Ok(None);
    }
    let engine = Engine {
        state: &cas.state,
        domain: &source.domain,
    };
    let one = engine
        .constant(true, budget)
        .map_err(|error| fault(&error))?;
    if !engine
        .formal_equal(
            source.coefficients.last().ok_or_else(invalid)?,
            &one,
            budget,
        )
        .map_err(|error| fault(&error))?
    {
        return Ok(None);
    }
    for coefficient in source
        .coefficients
        .iter()
        .skip(1)
        .take(source.coefficients.len().saturating_sub(2))
    {
        if !engine
            .formal_zero(coefficient, budget)
            .map_err(|error| fault(&error))?
        {
            return Ok(None);
        }
    }
    let constant = match super::formal_base::rational(
        source.coefficients.first().ok_or_else(invalid)?,
        budget,
    ) {
        Ok(value) => value,
        Err(CoefficientFailure::Unsupported(_)) => return Ok(None),
        Err(CoefficientFailure::Fault(error)) => return Err(error),
    };
    if degree == 2 && constant.compare(&Rational::zero(), budget)? != std::cmp::Ordering::Less {
        return Ok(None);
    }
    Ok(Some((constant, degree)))
}

/// Typed real syntax may live under the original complex header without changing its declarations.
fn real(
    construction: &Construction,
    cas: &mut Cas,
    operator: Operator,
    arguments: &[Handle],
    scope: crate::model::Scope,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let mut operands = vector(arguments.len(), budget)?;
    operands.extend(arguments.iter().cloned().map(Reference::Existing));
    admit(
        cas,
        &Draft::Expression(ExpressionDraft {
            header: Reference::Existing(construction.header.clone()),
            scope,
            operator,
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            operands,
            space: None,
            binding: None,
        }),
        budget,
    )
}

/// A degree-generic rational polynomial carries an explicit real scalar map on this exact region.
fn real_relation(
    construction: &Construction,
    cas: &mut Cas,
    constant: &Rational,
    degree: u32,
    budget: &mut Budget,
) -> Result<(Handle, Handle), Fault> {
    let domain = materialize::admit(
        cas,
        SolveDraft::CoefficientDomain(CoefficientDomainData::Rational),
        budget,
    )?;
    let engine = Engine {
        state: &cas.state,
        domain: &domain,
    };
    let mut coefficients = vector(
        usize::try_from(degree)
            .map_err(|_| invalid())?
            .checked_add(1)
            .ok_or_else(invalid)?,
        budget,
    )?;
    coefficients.push(
        engine
            .rational(constant, budget)
            .map_err(|error| fault(&error))?,
    );
    for _ in 1..degree {
        coefficients.push(
            engine
                .constant(false, budget)
                .map_err(|error| fault(&error))?,
        );
    }
    coefficients.push(
        engine
            .constant(true, budget)
            .map_err(|error| fault(&error))?,
    );
    let polynomial = Dense::new(&engine, coefficients, budget).map_err(|error| fault(&error))?;
    let stored = materialize::polynomial(cas, &polynomial, &[Field::Real], budget)?;
    let map = materialize::admit(
        cas,
        SolveDraft::CoefficientMap(CoefficientMapData {
            source: Reference::Existing(domain.clone()),
            target: MapTarget::ScalarAmbient {
                header: Reference::Existing(construction.header.clone()),
                field: Field::Real,
            },
            mode: MapMode::Specialization,
            region: Reference::Existing(construction.region.clone()),
            images: Vec::new(),
            denominators: Vec::new(),
        }),
        budget,
    )?;
    let relation = materialize::admit(
        cas,
        SolveDraft::RootRelation(RootRelationData {
            header: Reference::Existing(construction.header.clone()),
            scope: Vec::new(),
            ambient: Field::Real,
            polynomial: Reference::Existing(stored),
            binder: vec![AtomicSort::Scalar(Field::Real)],
            map: Reference::Existing(map),
            region: Reference::Existing(construction.region.clone()),
            multiplicity: None,
        }),
        budget,
    )?;
    Ok((domain, relation))
}

/// Each selected value has the exact single relation and predicate that its existence step checks.
fn selection_values(
    construction: &Construction,
    cas: &mut Cas,
    relation: &Handle,
    predicate: &Handle,
    mode: SelectionMode,
    budget: &mut Budget,
) -> Result<(Handle, Handle), Fault> {
    let selected = materialize::admit(
        cas,
        SolveDraft::RootSelection(RootSelectionData {
            cell: None,
            header: Reference::Existing(construction.header.clone()),
            scope: Vec::new(),
            relations: vec![Reference::Existing(relation.clone())],
            predicate: Reference::Existing(predicate.clone()),
            mode,
            convention: None,
            interval: None,
        }),
        budget,
    )?;
    let root = admit(
        cas,
        &Draft::RootValue {
            header: Reference::Existing(construction.header.clone()),
            selection: Reference::Existing(selected.clone()),
            slot: 0,
        },
        budget,
    )?;
    Ok((selected, root))
}

/// Real odd roots use the named theorem; positive square roots retain their actual sign constraint.
fn witness(
    construction: &mut Construction,
    cas: &mut Cas,
    constant: &Rational,
    degree: u32,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let (domain, relation) = real_relation(construction, cas, constant, degree, budget)?;
    let scope = vec![vec![AtomicSort::Scalar(Field::Real)]];
    let bound = real(
        construction,
        cas,
        Operator::Bound { depth: 0, slot: 0 },
        &[],
        scope.clone(),
        budget,
    )?;
    let arguments = if degree == 2 {
        vec![Reference::Existing(bound)]
    } else {
        vec![
            Reference::Existing(bound.clone()),
            Reference::Existing(bound),
        ]
    };
    let predicate = admit(
        cas,
        &Draft::Condition {
            header: Reference::Existing(construction.header.clone()),
            scope,
            kind: if degree == 2 {
                PredicateKind::Positive
            } else {
                PredicateKind::Equal
            },
            arguments,
        },
        budget,
    )?;
    let (selected, root) = selection_values(
        construction,
        cas,
        &relation,
        &predicate,
        if degree == 2 {
            SelectionMode::Constrained
        } else {
            SelectionMode::Any
        },
        budget,
    )?;
    let unit = construction.unit(cas, &domain, budget)?;
    construction.push(
        SolveSchema::RootAdjunction,
        SolveStatement::RootExistence(StatementRootExistence {
            relation: selected.clone(),
            region: construction.region.clone(),
        }),
        SolveCertificate::RootAdjunction(CertificateRootAdjunction {
            mode: if degree == 2 {
                AdjunctionMode::PositiveRealSquareRoot
            } else {
                AdjunctionMode::RealOddRoot
            },
            relation,
            selection: Some(selected),
            leading_unit: unit,
            order: Some(degree),
            splitting_chain: Vec::new(),
        }),
        budget,
    )?;
    Ok(root)
}

/// The target's realness constraint is proved using the actual named witness, not asserted as inhabited.
pub(crate) fn choose(
    cas: &mut Cas,
    construction: &mut Construction,
    source: &Dense,
    target: &Handle,
    budget: &mut Budget,
) -> Result<Option<(Handle, Handle)>, Fault> {
    let Some((constant, degree)) = candidate(cas, source, budget)? else {
        return Ok(None);
    };
    let root = witness(construction, cas, &constant, degree, budget)?;
    let bound = construction.expression(
        cas,
        Operator::Bound { depth: 0, slot: 0 },
        &[],
        true,
        budget,
    )?;
    let scope = vec![vec![AtomicSort::Scalar(Field::Complex)]];
    let imaginary = real(
        construction,
        cas,
        Operator::ImagPart,
        &[bound],
        scope.clone(),
        budget,
    )?;
    let zero = real(
        construction,
        cas,
        Operator::Rational(Rational::zero()),
        &[],
        scope.clone(),
        budget,
    )?;
    let predicate = admit(
        cas,
        &Draft::Condition {
            header: Reference::Existing(construction.header.clone()),
            scope,
            kind: PredicateKind::Equal,
            arguments: vec![Reference::Existing(imaginary), Reference::Existing(zero)],
        },
        budget,
    )?;
    let (selection, included) = selection_values(
        construction,
        cas,
        target,
        &predicate,
        SelectionMode::Constrained,
        budget,
    )?;
    construction.push(
        SolveSchema::RootSelectionRefinement,
        SolveStatement::RootExistence(StatementRootExistence {
            relation: selection.clone(),
            region: construction.region.clone(),
        }),
        SolveCertificate::RootSelectionRefinement(CertificateRootSelectionRefinement {
            mode: RefinementMode::Existence,
            target_kind: RefinementTargetKind::Selection,
            target: selection.clone(),
            witnesses: vec![root],
            compatibility: Vec::new(),
            form: None,
        }),
        budget,
    )?;
    Ok(Some((selection, included)))
}
