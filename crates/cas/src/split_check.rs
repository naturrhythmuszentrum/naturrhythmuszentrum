// SPDX-License-Identifier: AGPL-3.0-only
//! Independent endpoint, integrability and telescoping checks for finite split calculus.

use crate::budget::{Budget, BudgetKind};
use crate::case_syntax::invalid;
use crate::error::Fault;
use crate::graph::{Cas, size};
use crate::number::Rational;
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, ScopedConditions, Statement,
};
use crate::rule::Primitive;
use crate::split_geometry::AT;
use crate::split_model::{SplitCertificate, SplitKind, SplitPiece};

impl Cas {
    /// Borrow the independently checked finite cover and endpoint certificate.
    ///
    /// # Errors
    /// Refuses foreign witnesses and proofs belonging to another schema.
    pub fn split_certificate(
        &self,
        witness: &crate::proof_model::VerifiedWitness,
    ) -> Result<&SplitCertificate, Fault> {
        let crate::graph::Vertex::Proof(application) =
            self.state.node(self.checked(witness.proof())?)?
        else {
            return Err(invalid());
        };
        let crate::graph::Vertex::ScopedApplication(application) = self.state.node(*application)?
        else {
            return Err(invalid());
        };
        let Certificate::Split(value) = &application.certificate else {
            return Err(invalid());
        };
        Ok(value)
    }
}

/// No unchecked regularity premise or unsupported improper endpoint is admitted.
pub(crate) fn check(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let (Statement::Split(value), Certificate::Split(certificate)) =
        (&draft.conclusion, &draft.certificate)
    else {
        return Err(invalid());
    };
    let expected_rule = match value.kind {
        SplitKind::Integral => Primitive::IntegralCaseSplit,
        SplitKind::Sum => Primitive::SumCaseSplit,
    };
    if draft.rule != expected_rule || !premises.is_empty() || !draft.substitutions.is_empty() {
        return Err(invalid());
    }
    measure(certificate, budget)?;
    let context = cas.state.context(cas.checked(&draft.context)?)?.admissible;
    crate::case_check::subset(cas, cas.checked(&value.domain)?, context, budget)?;
    let expected = crate::split_geometry::reconstruct(cas, value, budget)?;
    if certificate.partition != expected.partition
        || certificate.points != expected.points
        || certificate.pieces.len() != expected.pieces.len()
    {
        return Err(invalid());
    }
    let mut total = Rational::zero();
    for (piece, geometry) in certificate.pieces.iter().zip(&expected.pieces) {
        if piece.cell != geometry.cell
            || piece.lower != geometry.lower
            || piece.upper != geometry.upper
            || piece.polynomial != geometry.polynomial
        {
            return Err(invalid());
        }
        total = total.add(&contribution(piece, value.kind, budget)?, budget)?;
    }
    if value
        .range
        .lower
        .compare(&value.range.upper, budget)?
        .is_gt()
    {
        total = total.negated(budget)?;
    }
    if total != value.value {
        return Err(invalid());
    }
    Ok(ScopedConditions::default())
}

/// Check the algebraic witness and both endpoint evaluations independently.
fn contribution(
    piece: &SplitPiece,
    kind: SplitKind,
    budget: &mut Budget,
) -> Result<Rational, Fault> {
    crate::split_polynomial::check(&piece.polynomial, &piece.primitive, kind, budget)?;
    let upper = if kind == SplitKind::Sum {
        piece.upper.add(&Rational::one(), budget)?
    } else {
        piece.upper.clone()
    };
    if crate::split_polynomial::endpoint(&piece.primitive, &piece.lower, budget)? != piece.start
        || crate::split_polynomial::endpoint(&piece.primitive, &upper, budget)? != piece.end
    {
        return Err(invalid());
    }
    piece.end.sub(&piece.start, budget)
}

/// Bound every untrusted vector and exact number before copying or checking.
pub(crate) fn measure(value: &SplitCertificate, budget: &mut Budget) -> Result<(), Fault> {
    budget.specialized(
        BudgetKind::CaseArms,
        size(value.pieces.len(), AT)?,
        false,
        AT,
    )?;
    budget.specialized(
        BudgetKind::BoundaryNodes,
        size(value.points.len(), AT)?,
        false,
        AT,
    )?;
    crate::region_check::measure(&value.partition, budget)?;
    let mut bytes = 0_u64;
    for piece in &value.pieces {
        for coefficients in [&piece.polynomial, &piece.primitive] {
            crate::univariate::validate(coefficients, budget)?;
            bytes = bytes
                .checked_add(crate::univariate::payload(coefficients, budget)?)
                .ok_or_else(invalid)?;
        }
        for number in [&piece.lower, &piece.upper, &piece.start, &piece.end] {
            number.check(budget, AT)?;
        }
    }
    for point in &value.points {
        point.point.check(budget, AT)?;
        if let Some(number) = &point.value {
            number.check(budget, AT)?;
        }
    }
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(bytes, AT)
}
