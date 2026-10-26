// SPDX-License-Identifier: AGPL-3.0-only
//! Certified projection of an explicit floor family to one exact Integer result.

use std::cmp::Ordering;

use crate::budget::Budget;
use crate::decision::{IntegerSelection, RationalEnclosure};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Vertex};
use crate::model::{AtomicSort, Operator, PredicateKind, Sort};
use crate::number::Rational;
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, DomainContract, ScopedConditions, Statement,
};
use crate::rule::Primitive;

/// Mathematical selection is separate from executable materialization.
const AT: Location = Location::new("integer family selection");

/// An altered family, value or correspondence cannot reuse another selection's evidence.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::EndpointMismatch, AT)
}

/// Validate the complete proposition even when used only as an explicit hypothesis.
#[expect(
    clippy::redundant_pub_crate,
    reason = "statement validation and the closed checker share these roles"
)]
pub(crate) fn roles(cas: &Cas, selection: &IntegerSelection) -> Result<(), Fault> {
    crate::decision_cut::family::roles(cas, &selection.family)?;
    literal(cas, selection)?;
    Ok(())
}

/// The selected result is an exact Integer literal under the original declaration header.
fn literal<'a>(cas: &'a Cas, selection: &IntegerSelection) -> Result<&'a Rational, Fault> {
    let value = cas.state.expression(cas.checked(&selection.value)?)?;
    let function = cas
        .state
        .expression(cas.checked(&selection.family.function)?)?;
    if value.sort != Sort::Value(AtomicSort::Integer)
        || value.header != function.header
        || !value.scope.is_empty()
        || value.binding.is_some()
        || value.space.is_some()
        || value.domain.is_some()
        || value.attachment.is_some()
    {
        return Err(invalid());
    }
    let Operator::Rational(number) = &value.operator else {
        return Err(invalid());
    };
    if !number.is_integer() {
        return Err(invalid());
    }
    Ok(number)
}

/// Checked cover, original-target replay and strict cell inclusion establish the unique projection.
#[expect(
    clippy::redundant_pub_crate,
    reason = "typed proof dispatch calls this closed selection checker"
)]
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let Statement::IntegerSelection(selection) = &draft.conclusion else {
        return Err(invalid());
    };
    let Certificate::IntegerSelection(enclosure) = &draft.certificate else {
        return Err(invalid());
    };
    let [cover, correspondence] = premises else {
        return Err(invalid());
    };
    if !draft.substitutions.is_empty() {
        return Err(invalid());
    }
    roles(cas, selection)?;
    family(cas, draft, selection, cover)?;
    coordinate(cas, draft, selection, enclosure, correspondence)?;
    crate::decision_enclosure::validate(cas, enclosure, budget)?;
    let scratch = budget
        .limits()
        .component_bits
        .div_ceil(8)
        .saturating_mul(32)
        .saturating_add(4_096);
    budget.with_retained_payload(scratch, AT, |budget| {
        let value = literal(cas, selection)?;
        value.check(budget, AT)?;
        crate::decision_cut::family::contains(cas, draft, &selection.family, cover, value, budget)?;
        cell(value, enclosure, budget)
    })?;
    let mut conditions = ScopedConditions::default();
    for premise in premises {
        crate::statement::merge(cas, &mut conditions, &premise.conditions, budget)?;
    }
    Ok(conditions)
}

/// A formal family hypothesis or a theorem about a different projection cannot supply cover.
fn family(
    cas: &Cas,
    draft: &ApplicationDraft,
    selection: &IntegerSelection,
    cover: &CheckedApplication,
) -> Result<(), Fault> {
    let Statement::IntegerFamily(family) = &cover.judgment.statement else {
        return Err(invalid());
    };
    if cover.rule != Primitive::RegionDecide
        || cover.certificate != Certificate::None
        || cover.judgment.context != draft.context
        || !cover.conditions.residuals.is_empty()
        || family.as_ref() != &selection.family
        || cas.state.context(cas.checked(&draft.context)?)?.admissible
            != cas.checked(&selection.family.domain)?
    {
        return Err(invalid());
    }
    Ok(())
}

/// The coordinate correspondence holds on the entire admitted family with every condition retained.
fn coordinate(
    cas: &Cas,
    draft: &ApplicationDraft,
    selection: &IntegerSelection,
    enclosure: &RationalEnclosure,
    premise: &CheckedApplication,
) -> Result<(), Fault> {
    let family = &selection.family;
    let different_context = enclosure.context != draft.context;
    let different_domain = enclosure.input_family != family.domain;
    if different_context
        || different_domain
        || premise.judgment.context != draft.context
        || !premise.conditions.residuals.is_empty()
    {
        return Err(invalid());
    }
    match &premise.judgment.statement {
        Statement::Equality {
            left,
            right,
            domain,
            domain_contract: DomainContract::DefinedAndEqual,
        } if *left == family.coordinate
            && *right == enclosure.target
            && *domain == family.domain =>
        {
            Ok(())
        }
        Statement::PredicateFact { region, predicate } if *region == family.domain => {
            let Vertex::Condition(predicate) = cas.state.node(cas.checked(predicate)?)? else {
                return Err(invalid());
            };
            if predicate.kind != PredicateKind::Equal
                || predicate.arguments
                    != [
                        cas.checked(&family.coordinate)?,
                        cas.checked(&enclosure.target)?,
                    ]
            {
                return Err(invalid());
            }
            Ok(())
        }
        _ => Err(invalid()),
    }
}

/// A closed enclosure must lie wholly below the next Integer, including its own upper endpoint.
fn cell(value: &Rational, enclosure: &RationalEnclosure, budget: &mut Budget) -> Result<(), Fault> {
    if value.compare(&enclosure.lower, budget)? == Ordering::Greater {
        return Err(Fault::invalid(
            InvalidCode::DecisionBoundaryIntersection,
            AT,
        ));
    }
    let next = value.add(&Rational::one(), budget)?;
    if enclosure.upper.compare(&next, budget)? != Ordering::Less {
        return Err(Fault::invalid(
            InvalidCode::DecisionBoundaryIntersection,
            AT,
        ));
    }
    Ok(())
}

impl Cas {
    /// Borrow the original evidence of a checked certified Integer selection.
    ///
    /// This authenticates the retained proof root without replaying its certificate
    /// or imposing an executable capacity. The witness retains every qualified
    /// source assumption; the borrowed view grants no physical count interpretation.
    /// # Errors
    /// Refuses foreign witnesses, other proof roots, residual obligations and exhausted Work.
    pub fn integer_selection_evidence<'a>(
        &'a self,
        witness: &crate::proof_model::VerifiedWitness,
        budget: &mut Budget,
    ) -> Result<(&'a IntegerSelection, &'a RationalEnclosure), Fault> {
        budget.work(4, AT)?;
        let Vertex::Proof(application) = self.state.node(self.checked(witness.proof())?)? else {
            return Err(invalid());
        };
        let Vertex::ScopedApplication(proof) = self.state.node(*application)? else {
            return Err(invalid());
        };
        let Statement::IntegerSelection(selection) = &proof.judgment.statement else {
            return Err(invalid());
        };
        let Certificate::IntegerSelection(enclosure) = &proof.certificate else {
            return Err(invalid());
        };
        if proof.rule != Primitive::CaseSelect || !proof.conditions.residuals.is_empty() {
            return Err(invalid());
        }
        Ok((selection, enclosure))
    }

    /// Materialize a checked selected Integer within an inclusive executable capacity.
    /// The caller retains the witness and its qualified assumptions alongside the count.
    /// # Errors
    /// Refuses foreign or unproved selections, negative or oversized counts and exhausted budgets.
    pub fn materialize_integer_selection(
        &self,
        witness: &crate::proof_model::VerifiedWitness,
        capacity: u64,
        budget: &mut Budget,
    ) -> Result<u64, Fault> {
        budget.work(4, AT)?;
        let Vertex::Proof(application) = self.state.node(self.checked(witness.proof())?)? else {
            return Err(invalid());
        };
        let Vertex::ScopedApplication(proof) = self.state.node(*application)? else {
            return Err(invalid());
        };
        let Statement::IntegerSelection(selection) = &proof.judgment.statement else {
            return Err(invalid());
        };
        if proof.rule != Primitive::CaseSelect
            || !matches!(proof.certificate, Certificate::IntegerSelection(_))
            || !proof.conditions.residuals.is_empty()
        {
            return Err(invalid());
        }
        literal(self, selection)?.materialize_count(capacity, budget)
    }
}
