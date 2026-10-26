// SPDX-License-Identifier: AGPL-3.0-only
//! Exact borrowed classification of retained predicate and statement syntax.

use super::HypothesisKind;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, LogicalId, Vertex};
use crate::model::{AtomicSort, Operator, PredicateKind, Sort};
use crate::proof_model::Statement;

/// A mathematical kind never reinterprets a non-predicate as an assumption.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Sort, Location::new("hypothesis kind"))
}

/// Check only the two equality operands; no expression traversal or simplification occurs.
fn equality_zero(cas: &Cas, arguments: &[LogicalId]) -> Result<bool, Fault> {
    let [left, right] = arguments else {
        return Ok(false);
    };
    for id in [left, right] {
        if matches!(&cas.state.expression(*id)?.operator, Operator::Rational(value) if value.is_zero())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Preserve the outer predicate kind, refining only an exact literal-zero equality.
fn predicate(
    cas: &Cas,
    kind: &PredicateKind,
    arguments: &[LogicalId],
) -> Result<HypothesisKind, Fault> {
    if *kind == PredicateKind::Equal && equality_zero(cas, arguments)? {
        Ok(HypothesisKind::DeclaredZero)
    } else {
        Ok(kind.hypothesis_kind())
    }
}

/// Distinguish strict, weak and zero Real sign conclusions without evaluating the subject.
fn sign(cas: &Cas, value: &crate::solving::StatementRealSign) -> Result<HypothesisKind, Fault> {
    use crate::solving::RealSignConclusion;
    Ok(match value.conclusion {
        RealSignConclusion::Positive | RealSignConclusion::Negative => HypothesisKind::StrictSign,
        RealSignConclusion::NonNegative => HypothesisKind::NonNegative,
        RealSignConclusion::NonPositive => HypothesisKind::NonPositive,
        RealSignConclusion::NonZero => HypothesisKind::NonZero,
        RealSignConclusion::Zero => {
            let subject = cas.state.expression(cas.checked(&value.subject)?)?;
            if subject.sort == Sort::Value(AtomicSort::Boolean) {
                cas.predicate_hypothesis_kind(&value.subject)?
            } else {
                HypothesisKind::DeclaredZero
            }
        }
    })
}

impl Cas {
    /// Read the formal kind of an admitted predicate without evaluating or proving it.
    ///
    /// Inspection examines at most its two equality operands and allocates no payload.
    /// # Errors
    /// Refuses foreign handles, non-predicate records and invalid equality operands.
    pub fn predicate_hypothesis_kind(&self, value: &Handle) -> Result<HypothesisKind, Fault> {
        match self.state.node(self.checked(value)?)? {
            Vertex::Condition(value) => predicate(self, &value.kind, &value.arguments),
            Vertex::Expression(value) if value.sort == Sort::Value(AtomicSort::Boolean) => {
                if let Operator::Predicate(kind) = &value.operator {
                    predicate(self, kind, &value.operands)
                } else {
                    Ok(HypothesisKind::Boolean)
                }
            }
            _ => Err(invalid()),
        }
    }

    /// Name a typed statement's premise kind; classification alone supplies no proof.
    /// # Errors
    /// Refuses a predicate or zero-sign subject whose referenced syntax is foreign or malformed.
    pub fn statement_hypothesis_kind(&self, value: &Statement) -> Result<HypothesisKind, Fault> {
        Ok(match value {
            Statement::PredicateFact {
                predicate: condition,
                ..
            } => self.predicate_hypothesis_kind(condition)?,
            Statement::Solve(value) => match value.as_ref() {
                crate::solving::SolveStatement::RealSign(value) => sign(self, value)?,
                _ => HypothesisKind::SolveEvidence,
            },
            Statement::Germ(_) => HypothesisKind::GermEvidence,
            Statement::Split(_) => HypothesisKind::SplitEvidence,
            Statement::Distribution(_) => HypothesisKind::DistributionEvidence,
            Statement::IntegerFamily(_) => HypothesisKind::IntegerFamily,
            Statement::BoundaryPartition(_) => HypothesisKind::BoundaryPartition,
            Statement::CaseDerivative(_) => HypothesisKind::CaseDerivative,
            Statement::DerivativeAt(_) => HypothesisKind::DerivativeAt,
            Statement::DerivativeDoesNotExist(_) => HypothesisKind::DerivativeNonexistence,
            Statement::TraceValue(_) => HypothesisKind::Trace,
            Statement::Equality { .. } => HypothesisKind::Equality,
            Statement::Subset { .. } => HypothesisKind::RegionMembership,
            Statement::Disjoint { .. } => HypothesisKind::Disjoint,
            Statement::Cover { .. } => HypothesisKind::Cover,
            Statement::Empty { .. } => HypothesisKind::EmptyRegion,
            Statement::Constructor { .. } => HypothesisKind::Constructor,
            Statement::DefinedDomain { .. } => HypothesisKind::DefinedDomain,
            Statement::Compatibility { .. } => HypothesisKind::Compatibility,
            Statement::Elaboration { .. } => HypothesisKind::Elaboration,
            Statement::Incidence { .. } => HypothesisKind::Incidence,
            Statement::Ownership { .. } => HypothesisKind::Ownership,
            Statement::DerivativeOn { .. } => HypothesisKind::Derivative,
            Statement::Jacobian { .. } => HypothesisKind::Jacobian,
            Statement::SquareFree { .. } => HypothesisKind::SquareFree,
            Statement::Jet { .. } => HypothesisKind::Jet,
            Statement::Jump { .. } => HypothesisKind::Jump,
        })
    }
}
