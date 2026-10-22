// SPDX-License-Identifier: AGPL-3.0-only
//! Complex selections are inhabited by explicit Real-in-Complex component witnesses.

use super::invalid;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Draft, Handle, Reference};
use crate::model::{AtomicSort, Field, Operator, PredicateKind};
use crate::solving::radicals::construction::{Construction, admit};
use crate::solving::towers::materialize;
use crate::solving::{
    CertificateRootSelectionRefinement, CoefficientDomainData, CoefficientValueData,
    CoefficientValueObject, RefinementMode, RefinementTargetKind, RootSelectionData, SelectionMode,
    SolveCertificate, SolveDraft, SolveSchema, SolveStatement, StatementRootExistence,
};

/// The imaginary unit is an exact Gaussian literal with an explicit family interpretation.
fn unit(cas: &mut Cas, construction: &Construction, budget: &mut Budget) -> Result<Handle, Fault> {
    let domain = materialize::admit(
        cas,
        SolveDraft::CoefficientDomain(CoefficientDomainData::Gaussian),
        budget,
    )?;
    let value = materialize::admit(
        cas,
        SolveDraft::CoefficientValue(CoefficientValueObject {
            domain: Reference::Existing(domain.clone()),
            value: CoefficientValueData::Gaussian {
                real: crate::number::Rational::zero(),
                imaginary: crate::number::Rational::one(),
            },
        }),
        budget,
    )?;
    let region = cas.state.region(construction.region.logical_id())?.clone();
    let ambient = admit(
        cas,
        &Draft::Region(crate::domain_model::RegionDraft {
            header: Reference::Existing(construction.header.clone()),
            scope: region.scope,
            ambient: region.ambient,
            expression: crate::domain_model::RegionExpr::Ambient,
        }),
        budget,
    )?;
    let global = Construction {
        header: construction.header.clone(),
        context: construction.context.clone(),
        region: ambient,
        steps: Vec::new(),
    };
    let map = global.mapping(cas, &domain, &[], budget)?;
    admit(
        cas,
        &Draft::ExactCoefficient {
            header: Reference::Existing(construction.header.clone()),
            value: Reference::Existing(value),
            map: Reference::Existing(map),
        },
        budget,
    )
}

/// Positive-base values have zero imaginary component; negative-base values have zero real component.
fn predicate(
    cas: &mut Cas,
    construction: &Construction,
    imaginary: bool,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let scope = vec![vec![AtomicSort::Scalar(Field::Complex)]];
    let bound = super::build::expression(
        cas,
        &construction.header,
        &scope,
        Operator::Bound { depth: 0, slot: 0 },
        Field::Complex,
        &[],
        budget,
    )?;
    let component = super::build::expression(
        cas,
        &construction.header,
        &scope,
        if imaginary {
            Operator::RealPart
        } else {
            Operator::ImagPart
        },
        Field::Real,
        &[bound],
        budget,
    )?;
    let zero = super::build::expression(
        cas,
        &construction.header,
        &scope,
        Operator::Rational(crate::number::Rational::zero()),
        Field::Real,
        &[],
        budget,
    )?;
    admit(
        cas,
        &Draft::Condition {
            header: Reference::Existing(construction.header.clone()),
            scope,
            kind: PredicateKind::Equal,
            arguments: vec![Reference::Existing(component), Reference::Existing(zero)],
        },
        budget,
    )
}

/// Existing witness refinement proves the selected relation and the component constraint together.
pub(super) fn include(
    cas: &mut Cas,
    construction: &mut Construction,
    target: &Handle,
    real: &Handle,
    imaginary: bool,
    budget: &mut Budget,
) -> Result<(Handle, Handle), Fault> {
    let factor = if imaginary {
        unit(cas, construction, budget)?
    } else {
        construction.expression(
            cas,
            Operator::Rational(crate::number::Rational::zero()),
            &[],
            false,
            budget,
        )?
    };
    let witness = construction.expression(
        cas,
        if imaginary {
            Operator::Mul
        } else {
            Operator::Add
        },
        &[real.clone(), factor],
        false,
        budget,
    )?;
    let predicate = predicate(cas, construction, imaginary, budget)?;
    let selection = materialize::admit(
        cas,
        SolveDraft::RootSelection(RootSelectionData {
            cell: None,
            header: Reference::Existing(construction.header.clone()),
            scope: Vec::new(),
            relations: vec![Reference::Existing(target.clone())],
            predicate: Reference::Existing(predicate),
            mode: SelectionMode::Constrained,
            convention: None,
            interval: None,
        }),
        budget,
    )?;
    let root = admit(
        cas,
        &Draft::RootValue {
            header: Reference::Existing(construction.header.clone()),
            selection: Reference::Existing(selection.clone()),
            slot: 0,
        },
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
            witnesses: vec![witness],
            compatibility: Vec::new(),
            form: None,
        }),
        budget,
    )?;
    if root == *real {
        return Err(invalid());
    }
    Ok((selection, root))
}
