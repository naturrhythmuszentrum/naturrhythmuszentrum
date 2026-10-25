// SPDX-License-Identifier: AGPL-3.0-only
//! Implicit differentiation joins the complete root law to its original operator Jacobian.

use super::{AT, LcSpectralLaw, ReadingError, SpectralLawError};
use crate::readings::{ReciprocalJacobian, SourceJacobianUsage};
use nrz_cas::limits::{Budget, BudgetKind};
use std::rc::Rc;

/// Shared exact derivative adapter for both signed physical LC roots.
///
/// For `B=[[0,a],[-b,0]]`, the retained law proves `r=sqrt(a*b)>0` and
/// physical imaginary rates `+r/t_ref,-r/t_ref`. Differentiating the checked
/// characteristic identity gives `d(r/t_ref)=(b*da+a*db)/(2*r*t_ref)`.
/// The negative root has the opposite derivative; both real derivatives vanish.
/// This is a first-order relation and supplies no nonlinear moment identity.
#[derive(Clone, Debug)]
pub struct LcRootJacobian(Rc<Definition>);

/// The complete root theorem and original derivative have the same physical inputs.
#[derive(Debug)]
struct Definition {
    /// Original complete roots, positivity and fixed state/time transport.
    law: LcSpectralLaw,
    /// Cached original operator derivative, shared without derivation or replay.
    operator: ReciprocalJacobian,
    /// Caller snapshots around exact identity admission.
    usage: SourceJacobianUsage,
    /// New adapter allocation; the law and derivative retain their original owners.
    bytes: u64,
}

impl LcRootJacobian {
    /// Retain one original operator derivative for the complete signed root pair.
    /// # Errors
    /// Refuses substituted operators, sources, references, omitted directions and caller limits.
    pub fn new(
        law: LcSpectralLaw,
        operator: ReciprocalJacobian,
        budget: &mut Budget,
    ) -> Result<Self, SpectralLawError> {
        let before = budget.usage();
        check(&law, &operator, budget)?;
        let bytes = u64::try_from(size_of::<Definition>() + 2 * size_of::<usize>())
            .map_err(|_| ReadingError::Shape)?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        Ok(Self(Rc::new(Definition {
            law,
            operator,
            usage: SourceJacobianUsage {
                before,
                after: budget.usage(),
            },
            bytes,
        })))
    }
    /// Complete original root law and its fixed physical transport.
    #[must_use]
    pub fn law(&self) -> &LcSpectralLaw {
        &self.0.law
    }
    /// The same cached exact operator Jacobian consumed by reciprocal covariance.
    #[must_use]
    pub fn operator(&self) -> &ReciprocalJacobian {
        &self.0.operator
    }
    /// Complete caller snapshots around adapter preparation, with no derivative replay.
    #[must_use]
    pub fn usage(&self) -> SourceJacobianUsage {
        self.0.usage
    }
    /// Newly owned adapter bytes, excluding already shared exact evidence.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
}

/// Full original identities precede the fixed implicit differentiation schedule.
fn check(
    law: &LcSpectralLaw,
    derivative: &ReciprocalJacobian,
    budget: &mut Budget,
) -> Result<(), SpectralLawError> {
    let expected = law.operator();
    let actual = derivative.definition();
    let work = expected
        .reuse_key()
        .as_bytes()
        .len()
        .checked_add(expected.inputs().len())
        .ok_or(ReadingError::Shape)?;
    budget.work(u64::try_from(work).map_err(|_| ReadingError::Shape)?, AT)?;
    let identity = expected.reuse_key() == actual.reuse_key()
        && expected.inputs() == actual.inputs()
        && expected.frame().context() == actual.frame().context()
        && expected.frame().entries() == actual.frame().entries();
    let complete = derivative.key().positions().iter().copied().eq(0..2);
    if !identity || !complete {
        return Err(SpectralLawError::Transport);
    }
    Ok(())
}
