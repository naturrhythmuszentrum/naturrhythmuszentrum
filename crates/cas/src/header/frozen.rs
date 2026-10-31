// SPDX-License-Identifier: AGPL-3.0-only
//! Shared immutable declaration validation, independent of fresh graph identities.

use std::sync::Arc;

use crate::budget::{Budget, BudgetKind};
use crate::encoding::{self, Encoder};
use crate::error::{Fault, InvalidCode, Location};
use crate::model::Environment;

/// Independently validated, sorted mathematical declarations and their canonical signature.
///
/// Sharing an environment does not share a graph's header or Free-symbol identity.
#[derive(Clone, Debug)]
pub struct FrozenEnvironment(Arc<Checked>);

/// One checked payload, shared by all headers retaining this declaration authority.
#[derive(Debug)]
struct Checked {
    /// Canonically sorted complete scalar declaration table.
    environment: Environment,
    /// Exact existing canonical signature, without a new wire tag.
    bytes: Vec<u8>,
    /// Name-comparison width is the environment checker's policy-dependent premise.
    name_bytes: u64,
    /// Largest actual name required when copying the cached canonical bytes.
    largest_name: usize,
}

/// The ordinary header location preserves the existing encoder's charge attribution.
const AT: Location = Location::new("semantic header");

impl FrozenEnvironment {
    /// Validate, sort and encode a space-free environment once on the caller's ledger.
    ///
    /// # Errors
    /// Refuses invalid declarations, space-dependent sorts and exhausted resources.
    pub fn freeze(environment: &Environment, budget: &mut Budget) -> Result<Self, Fault> {
        if environment
            .declarations
            .iter()
            .any(|item| super::space_sort(&item.sort))
        {
            return Err(Fault::invalid(InvalidCode::Context, AT));
        }
        let environment = crate::validation::environment(environment, budget)?;
        Self::validated(environment, budget)
    }

    /// Seal an already validated extension without repeating its admission checks.
    pub(super) fn validated(environment: Environment, budget: &mut Budget) -> Result<Self, Fault> {
        let mut out = Encoder::new(budget, AT);
        encoding::environment(&environment, &mut out)?;
        let (bytes, largest_name) = out.finish_names();
        let name_bytes = budget.limits().name_bytes;
        Ok(Self(Arc::new(Checked {
            environment,
            bytes,
            name_bytes,
            largest_name,
        })))
    }

    /// Borrow the exact existing mathematical declaration language.
    #[must_use]
    pub fn environment(&self) -> &Environment {
        &self.0.environment
    }

    /// Borrow the signature encoded during freezing; callers cannot alter its authority.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.0.bytes
    }

    /// Copy the unchanged signature with the original name limits still enforced.
    pub(crate) fn encode(&self, out: &mut Encoder<'_>) -> Result<(), Fault> {
        out.frozen(self.canonical_bytes(), self.0.largest_name)
    }

    /// Authenticate the effective environment policy and contain its retained declaration data.
    pub(crate) fn authenticate(&self, budget: &mut Budget) -> Result<(), Fault> {
        budget.work(1, AT)?;
        if self.0.name_bytes != budget.limits().name_bytes {
            return Err(Fault::invalid(InvalidCode::Policy, AT));
        }
        let count = crate::graph::size(self.declarations.len(), AT)?;
        let bytes = crate::graph::size(self.canonical_bytes().len(), AT)?;
        budget.check(BudgetKind::Vertices, count, AT)?;
        budget.check(BudgetKind::PayloadBytes, bytes, AT)?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        Ok(())
    }
}

impl std::ops::Deref for FrozenEnvironment {
    type Target = Environment;

    fn deref(&self) -> &Self::Target {
        self.environment()
    }
}

impl PartialEq for FrozenEnvironment {
    fn eq(&self, other: &Self) -> bool {
        self.environment() == other.environment()
    }
}

impl Eq for FrozenEnvironment {}

#[cfg(test)]
mod tests;
