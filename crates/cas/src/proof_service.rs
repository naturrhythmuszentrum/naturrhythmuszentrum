// SPDX-License-Identifier: AGPL-3.0-only
//! Typed public boundaries around scalar cost and multiplicity services.

use crate::budget::Budget;
use crate::cost::CostPolicy;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle};
use crate::proof_model::{Judgment, VerifiedWitness};

/// A service cannot erase a scoped theorem it cannot represent.
const AT: Location = Location::new("typed proof service");

/// Retain exactly the checked scalar view required by an existing cost service.
fn scalar_witness(
    cas: &Cas,
    value: &VerifiedWitness,
    budget: &mut Budget,
) -> Result<crate::scalar_model::VerifiedWitness, Fault> {
    cas.checked(value.proof())?;
    let claim = crate::proof_view::scalar_claim(cas, value.judgment())?;
    let mut hypotheses = Vec::new();
    hypotheses
        .try_reserve_exact(value.hypotheses.len())
        .map_err(|_| Fault::allocation(AT))?;
    for reference in &value.hypotheses {
        let frame = cas.state.context(cas.checked(&reference.context)?)?;
        if frame.parent.is_some() {
            return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT));
        }
        let slot = usize::try_from(reference.slot)
            .map_err(|_| Fault::invalid(InvalidCode::Reference, AT))?;
        hypotheses.push(
            frame
                .assumptions
                .get(slot)
                .ok_or_else(|| Fault::invalid(InvalidCode::Reference, AT))?
                .0
                .clone(),
        );
    }
    let bytes = cas.export_witness(value, budget)?;
    Ok(crate::scalar_model::VerifiedWitness {
        proof: value.proof.clone(),
        claim,
        hypotheses,
        bytes,
    })
}

impl Cas {
    /// Register a strict cost improvement for the exact typed equality under the current policy.
    ///
    /// # Errors
    /// Refuses changed statements or residuals, unsupported theorem classes and non-improvement.
    pub fn register_derived(
        &mut self,
        witness: &VerifiedWitness,
        expected: &Judgment,
        cost: u64,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let witness = scalar_witness(self, witness, budget)?;
        let expected = crate::proof_view::scalar_claim(self, expected)?;
        self.register_scalar_derived(&witness, &expected, cost, budget)
    }

    /// Register a replayed improvement with the exact complete policy identity and table.
    ///
    /// # Errors
    /// Refuses conflicting policies, incompatible theorem scopes and nonpositive costs.
    pub fn register_derived_with_policy(
        &mut self,
        witness: &VerifiedWitness,
        expected: &Judgment,
        policy: &CostPolicy,
        cost: u64,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let witness = scalar_witness(self, witness, budget)?;
        let expected = crate::proof_view::scalar_claim(self, expected)?;
        self.register_scalar_derived_with_policy(&witness, &expected, policy, cost, budget)
    }

    /// Certify a finite frozen equality route within one exact typed theorem class.
    ///
    /// # Errors
    /// Refuses unsupported theorem classes, changed policies and exhausted resources.
    pub fn certify_frozen_cost(
        &mut self,
        judgment: &Judgment,
        policy: &CostPolicy,
        budget: &mut Budget,
    ) -> Result<crate::frozen::FrozenCertificate, Fault> {
        crate::proof_view::validate_judgment(self, judgment, budget)?;
        let claim = crate::proof_view::scalar_claim(self, judgment)?;
        self.certify_scalar_frozen_cost(&claim, policy, budget)
    }
}
