// SPDX-License-Identifier: AGPL-3.0-only
//! Borrowed closed judgments reached through checked evidence edges in the source graph.

use super::{ExactConstant, PositiveConstant, Value};
use crate::budget::Budget;
use crate::error::{Fault, Location};
use crate::graph::{Cas, Handle};

/// Local preparation and borrowed identity citations share one accounting location.
const AT: Location = Location::new("exact constant cache");

/// Sealed local evidence for one original expression in its exact context.
///
/// The finite-real admission, normalization equality and sign are checked once.
/// Identity citations borrow this same preparation; they never rebuild its graph.
/// This receipt has no portable proof or context-discharge authority.
#[derive(Debug)]
pub struct PreparedConstant {
    /// Original expression, before any cancellation or normalization.
    original: Handle,
    /// Exact mathematical frame, including all retained assumptions and its family.
    context: Handle,
    /// Owned normalized value with privately checked finite-real and sign evidence.
    value: ExactConstant,
}

/// Finite-real identity whose sole nonliteral premise is a checked preparation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstantIdentityKind {
    /// The retained finite value multiplied on the right by literal one.
    MultiplyOne,
    /// The retained finite value plus literal zero on the right.
    AddZero,
}

/// Borrowed evidence citing precisely one retained finite-real judgment.
#[derive(Debug)]
pub struct ConstantIdentity<'a> {
    /// The single checked nonliteral premise and normalized conclusion.
    preparation: &'a PreparedConstant,
    /// The elementary finite-real identity applied to that premise.
    kind: ConstantIdentityKind,
}

impl Cas {
    /// Admit or cite an anonymous closed-constant lemma in this source graph.
    ///
    /// The original expression reaches its checked preparation through an evidence
    /// edge. Ordinary `ExactConstant::from_expression` remains independent.
    /// # Errors
    /// Refuses foreign handles, mismatched contexts, undefined constants or
    /// exhausted caller and source-store ceilings without publishing a prefix.
    pub fn prepare_constant(
        &mut self,
        context: &Handle,
        original: &Handle,
        budget: &mut Budget,
    ) -> Result<&PreparedConstant, Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = self.prepare_constant_node(context, original, budget);
        budget.restore_limits(previous);
        self.constant_preparation(result?)
    }
}

impl PreparedConstant {
    /// Construct evidence only after the complete original constant checker succeeds.
    pub(crate) const fn checked(original: Handle, context: Handle, value: ExactConstant) -> Self {
        Self {
            original,
            context,
            value,
        }
    }

    /// Formal schema instantiated by this complete original finite-real judgment.
    #[must_use]
    pub const fn formal_schema(&self) -> &'static crate::emptiness::LemmaSchema {
        &crate::proof::RETAINED_SCHEMAS[0]
    }

    /// Checked premise shapes; none is a new source assumption.
    #[must_use]
    pub const fn premise_kinds(&self) -> &'static [crate::proof::HypothesisKind] {
        &[
            crate::proof::HypothesisKind::DomainGuard,
            crate::proof::HypothesisKind::Equality,
        ]
    }

    /// Original unnormalized target retained by this checked judgment.
    #[must_use]
    pub const fn original(&self) -> &Handle {
        &self.original
    }

    /// Actual context whose complete identity qualified admission.
    #[must_use]
    pub const fn context(&self) -> &Handle {
        &self.context
    }

    /// Normalized value, borrowing the retained immutable evidence.
    #[must_use]
    pub const fn value(&self) -> &ExactConstant {
        &self.value
    }

    /// Strict positivity only when the retained closed sign proof established it.
    #[must_use]
    pub fn positive(&self) -> Option<PositiveConstant> {
        self.value.positive()
    }

    /// Cite an elementary identity of this already checked finite-real value.
    /// # Errors
    /// Refuses an exhausted caller Work ledger; no graph is allocated or changed.
    pub fn identity(
        &self,
        kind: ConstantIdentityKind,
        budget: &mut Budget,
    ) -> Result<ConstantIdentity<'_>, Fault> {
        budget.work(1, AT)?;
        Ok(ConstantIdentity {
            preparation: self,
            kind,
        })
    }
}

impl<'a> ConstantIdentity<'a> {
    /// The sole retained nonliteral premise, also owning the exact result.
    #[must_use]
    pub const fn preparation(&self) -> &'a PreparedConstant {
        self.preparation
    }

    /// Elementary identity checked under the finite-real premise.
    #[must_use]
    pub const fn kind(&self) -> ConstantIdentityKind {
        self.kind
    }

    /// Exact normalized result, sharing the preparation's value and evidence.
    #[must_use]
    pub const fn value(&self) -> &'a ExactConstant {
        self.preparation.value()
    }
}

/// Include the owned exact graph, complete canonical bytes and conservative entry metadata.
pub(crate) fn footprint(value: &ExactConstant) -> Result<u64, Fault> {
    let payload = match &value.0 {
        Value::Rational(number) => {
            let (numerator, denominator) = number.component_bits();
            numerator
                .div_ceil(8)
                .saturating_add(denominator.div_ceil(8))
                .saturating_mul(4)
        }
        Value::Expression(graph) => graph
            .cas
            .counts()
            .payload_bytes
            .saturating_add(crate::graph::size(graph.key.len(), AT)?),
    };
    Ok(payload.saturating_add(512))
}

#[cfg(test)]
mod tests;
