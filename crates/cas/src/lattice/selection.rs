// SPDX-License-Identifier: AGPL-3.0-only
//! Named tuple selection composes a checked affine cover and original-target box replay.

#[expect(
    clippy::redundant_pub_crate,
    reason = "the statement kernel shares the closed lattice selection wire"
)]
pub(crate) mod wire;

use super::{LatticeBoxEnclosure, LatticeCoordinateEnclosure, LatticeCover};
use crate::budget::Budget;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, Vertex, size};
use crate::model::{AtomicSort, Operator, Sort};
use crate::number::Rational;
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, ScopedConditions, Statement, VerifiedWitness,
};
use crate::rule::Primitive;

/// Every selection charge belongs to this optional owner.
const AT: Location = Location::new("lattice tuple selection");

/// Exact original lattice question, target and ordered proposed Integer values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LatticeSelection {
    /// Complete original affine cover and ordered index domain.
    pub cover: LatticeCover,
    /// Original explicit Real target column, including unnormalized entry syntax.
    pub target: Handle,
    /// One exact Integer literal per coordinate, in declared order.
    pub values: Vec<Handle>,
}

/// Closed original-target evidence for one of the two admitted box forms.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LatticeEnclosure {
    /// Transport the entire original E through the exact admitted inverse.
    Original(Box<LatticeBoxEnclosure>),
    /// Replay the original affine expression directly; this makes no claim about an E.
    Coordinates(Box<LatticeCoordinateEnclosure>),
}

/// Authenticate exact typed roles even when the statement is only a hypothesis.
#[expect(
    clippy::redundant_pub_crate,
    reason = "statement validation shares these closed lattice roles"
)]
pub(crate) fn roles(cas: &Cas, value: &LatticeSelection, budget: &mut Budget) -> Result<(), Fault> {
    let dimension = super::cover::check::roles(cas, &value.cover, budget)?;
    let home = cas
        .state
        .context(cas.checked(&value.cover.index.basis.context)?)?;
    if cas.lattice_rectangle(cas.checked(&value.target)?, home.header_id)? != (dimension, 1)
        || size(value.values.len(), AT)? != u64::from(dimension)
    {
        return Err(invalid());
    }
    for selected in &value.values {
        literal(cas, selected, &value.cover.index.basis.context)?;
    }
    Ok(())
}

/// A tuple entry retains mathematical Integer sort independently of executable capacity.
fn literal<'a>(cas: &'a Cas, value: &Handle, context: &Handle) -> Result<&'a Rational, Fault> {
    let expression = cas.state.expression(cas.checked(value)?)?;
    let home = cas.state.context(cas.checked(context)?)?;
    if expression.sort != Sort::Value(AtomicSort::Integer)
        || expression.header != home.header_id
        || !expression.scope.is_empty()
        || expression.binding.is_some()
        || expression.space.is_some()
        || expression.domain.is_some()
        || expression.attachment.is_some()
    {
        return Err(invalid());
    }
    let Operator::Rational(number) = &expression.operator else {
        return Err(invalid());
    };
    if !number.is_integer() {
        return Err(invalid());
    }
    Ok(number)
}

/// One retained cover, complete original replay, exact range membership and strict cuts suffice.
#[expect(
    clippy::redundant_pub_crate,
    reason = "typed proof dispatch shares this closed lattice checker"
)]
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let Statement::LatticeSelection(selection) = &draft.conclusion else {
        return Err(invalid());
    };
    let Certificate::LatticeSelection(enclosure) = &draft.certificate else {
        return Err(invalid());
    };
    let [cover] = premises else {
        return Err(invalid());
    };
    if draft.context != selection.cover.index.basis.context || !draft.substitutions.is_empty() {
        return Err(invalid());
    }
    cover_matches(selection, cover)?;
    roles(cas, selection, budget)?;
    validate(cas, selection, enclosure, budget)?;
    let scratch = budget
        .limits()
        .component_bits
        .div_ceil(8)
        .saturating_mul(32)
        .saturating_add(4_096);
    budget.with_retained_payload(scratch, AT, |budget| {
        ranges(cas, selection, cover, budget)?;
        cuts(cas, selection, enclosure, budget)
    })?;
    let mut conditions = ScopedConditions::default();
    crate::statement::merge(cas, &mut conditions, &cover.conditions, budget)?;
    Ok(conditions)
}

/// A checked cover about another original question cannot supply selection authority.
fn cover_matches(selection: &LatticeSelection, cover: &CheckedApplication) -> Result<(), Fault> {
    let Statement::LatticeCover(actual) = &cover.judgment.statement else {
        return Err(invalid());
    };
    if actual.as_ref() != &selection.cover
        || cover.rule != Primitive::RegionDecide
        || cover.certificate != Certificate::None
        || cover.judgment.context != selection.cover.index.basis.context
        || !cover.conditions.residuals.is_empty()
    {
        return Err(invalid());
    }
    Ok(())
}

/// Complete declared range membership is checked before any potentially inconclusive cut.
fn ranges(
    cas: &Cas,
    selection: &LatticeSelection,
    cover: &CheckedApplication,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for (index, (selected, family)) in selection
        .values
        .iter()
        .zip(&selection.cover.index.families)
        .enumerate()
    {
        let value = literal(cas, selected, &selection.cover.index.basis.context)?;
        value.check(budget, AT)?;
        let scalar = scalar_cover(cas, cover, index)?;
        let range = ApplicationDraft {
            rule: Primitive::RegionDecide,
            context: scalar.judgment.context.clone(),
            conclusion: Statement::IntegerFamily(Box::new(family.clone())),
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::None,
        };
        crate::decision_cut::family::contains(cas, &range, family, scalar, value, budget)?;
    }
    Ok(())
}

/// Strict cuts are independent of cover composition and carry no cell search.
fn cuts(
    cas: &Cas,
    selection: &LatticeSelection,
    enclosure: &LatticeEnclosure,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for (index, selected) in selection.values.iter().enumerate() {
        let value = literal(cas, selected, &selection.cover.index.basis.context)?;
        let (lower, upper) = bounds(enclosure, index)?;
        cell(value, lower, upper, budget)?;
    }
    Ok(())
}

/// Original target identity is part of the expected proposition, never only an enclosure hint.
fn validate(
    cas: &Cas,
    selection: &LatticeSelection,
    enclosure: &LatticeEnclosure,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let basis = &selection.cover.index.basis;
    let inverse = &selection.cover.inverse;
    match enclosure {
        LatticeEnclosure::Original(contract) => {
            if &contract.basis != basis
                || &contract.inverse != inverse
                || contract.original.target != selection.target
            {
                return Err(invalid());
            }
            super::affine_enclosure::validate(
                cas,
                contract,
                u32::try_from(selection.values.len()).map_err(|_| invalid())?,
                budget,
            )?;
        }
        LatticeEnclosure::Coordinates(contract) => {
            if &contract.basis != basis
                || &contract.inverse != inverse
                || contract.target != selection.target
            {
                return Err(invalid());
            }
            super::coordinate_enclosure::validate(
                cas,
                contract,
                u32::try_from(selection.values.len()).map_err(|_| invalid())?,
                budget,
            )?;
        }
    }
    Ok(())
}

/// Checked cover roots retain their original scalar range proofs as ordered direct premises.
fn scalar_cover<'a>(
    cas: &'a Cas,
    cover: &CheckedApplication,
    coordinate: usize,
) -> Result<&'a CheckedApplication, Fault> {
    let proof = *cover
        .premises
        .get(coordinate.saturating_add(3))
        .ok_or_else(invalid)?;
    let Vertex::Proof(application) = cas.state.node(proof)? else {
        return Err(invalid());
    };
    let Vertex::ScopedApplication(application) = cas.state.node(*application)? else {
        return Err(invalid());
    };
    Ok(application)
}

/// Both box forms expose the same closed rational endpoint cut.
fn bounds(
    enclosure: &LatticeEnclosure,
    coordinate: usize,
) -> Result<(&Rational, &Rational), Fault> {
    match enclosure {
        LatticeEnclosure::Original(value) => value
            .bounds
            .get(coordinate)
            .map(|(lower, upper)| (lower, upper))
            .ok_or_else(invalid),
        LatticeEnclosure::Coordinates(value) => value
            .coordinates
            .coordinates
            .get(coordinate)
            .map(|entry| (&entry.lower, &entry.upper))
            .ok_or_else(invalid),
    }
}

/// Upper faces are excluded exactly; no center or epsilon can supply the strict inequality.
fn cell(
    value: &Rational,
    lower: &Rational,
    upper: &Rational,
    budget: &mut Budget,
) -> Result<(), Fault> {
    use std::cmp::Ordering;
    if value.compare(lower, budget)? == Ordering::Greater {
        return Err(Fault::invalid(
            InvalidCode::DecisionBoundaryIntersection,
            AT,
        ));
    }
    let next = value.add(&Rational::one(), budget)?;
    if upper.compare(&next, budget)? != Ordering::Less {
        return Err(Fault::invalid(
            InvalidCode::DecisionBoundaryIntersection,
            AT,
        ));
    }
    Ok(())
}

impl Cas {
    /// Check one named exact tuple against a retained affine cover and original-target enclosure.
    ///
    /// The portable witness retains the cover, original target, every coordinate certificate,
    /// Machin dependencies and qualified parameter assumptions. It enumerates no cells.
    /// # Errors
    /// Refuses wrong original questions, non-Integer or excluded tuples, inconclusive whole-box
    /// cuts, unsupported enclosure evidence and exhausted resources. Publication is atomic.
    pub fn prove_lattice_selection(
        &mut self,
        cover: &VerifiedWitness,
        expected: &LatticeSelection,
        enclosure: &LatticeEnclosure,
        budget: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        self.prove_lattice_selection_root(cover.proof(), expected, enclosure, budget)
    }

    /// The producer and prepared-attempt consumer share one transactional proof path.
    fn prove_lattice_selection_root(
        &mut self,
        cover: &Handle,
        expected: &LatticeSelection,
        enclosure: &LatticeEnclosure,
        budget: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        self.proof_limits(budget, |cas, budget| {
            cas.transaction(budget, |cas, budget| {
                let bytes = wire::payload(cas, expected, enclosure, budget)?;
                let proof = cas.checked(cover)?;
                let retained =
                    crate::proof_view::application_bytes(cas, proof, budget)?.saturating_add(bytes);
                budget.with_retained_payload(retained, AT, |budget| {
                    let premise = crate::proof_view::application(cas, proof, budget)?;
                    let draft = ApplicationDraft {
                        rule: Primitive::CaseSelect,
                        context: expected.cover.index.basis.context.clone(),
                        conclusion: Statement::LatticeSelection(Box::new(expected.clone())),
                        premises: vec![0],
                        substitutions: Vec::new(),
                        certificate: Certificate::LatticeSelection(Box::new(enclosure.clone())),
                    };
                    let mut checked = crate::scoped_check::check_application(
                        cas,
                        &draft,
                        std::slice::from_ref(&premise),
                        budget,
                    )?;
                    checked.premises = vec![proof];
                    let root = cas.publish_scoped(checked, budget)?;
                    cas.seal_scoped(&root, budget)
                })
            })
        })
    }
}

/// Altered original questions and incomplete typed tuples cannot borrow another selection.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::EndpointMismatch, AT)
}
