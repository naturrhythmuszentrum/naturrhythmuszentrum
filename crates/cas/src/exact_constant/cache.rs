// SPDX-License-Identifier: AGPL-3.0-only
//! Caller-owned closed judgments keyed by complete canonical graph references.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::{ExactConstant, PositiveConstant, Value};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, LogicalId, size};

/// Local preparation and borrowed identity citations share one accounting location.
const AT: Location = Location::new("exact constant cache");

/// Caller-owned retention of checked constants across operations in one source store.
///
/// Within one owner and rollback generation, canonical expression and context IDs
/// identify their complete immutable dependency graphs, including declaration and
/// assumption identities. Normalized values and printed names are never keys.
/// Changing stores or rolling back requires fresh admission. Ordinary
/// `ExactConstant::from_expression` does not consult or allocate this cache.
#[derive(Debug, Default)]
pub struct ExactConstantCache {
    /// Strong ownership prevents an allocator from recycling a source namespace.
    owner: Option<Arc<()>>,
    /// Logical IDs may be reused only after this generation changes.
    epoch: u64,
    /// Context followed by the original, unnormalized expression.
    entries: BTreeMap<(LogicalId, LogicalId), PreparedConstant>,
    /// Conservative retained constants, keys, handles and tree allocation.
    bytes: u64,
}

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

impl ExactConstantCache {
    /// Empty retention without global state or a hidden budget.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            owner: None,
            epoch: 0,
            entries: BTreeMap::new(),
            bytes: 0,
        }
    }

    /// Number of distinct original expression/context judgments retained.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no checked judgment is retained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Conservative caller-owned payload, excluding shared source syntax.
    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        self.bytes
    }

    /// Admit once or borrow the existing checked judgment under its full original key.
    ///
    /// A hit pays the bit width of the retained entry count in Work, at least one.
    /// A miss uses the unchanged admission ledger; its existing graph-construction
    /// allowance covers the fixed reference key and tree insertion. Retained
    /// payload remains reserved beside admission scratch. Refusal publishes nothing.
    /// # Errors
    /// Refuses foreign handles, mismatched headers, unsupported or undefined
    /// constants, and exhausted caller or source-store storage ceilings.
    pub fn prepare<'a>(
        &'a mut self,
        cas: &Cas,
        context: &Handle,
        original: &Handle,
        budget: &mut Budget,
    ) -> Result<&'a PreparedConstant, Fault> {
        let previous = budget.restrict_storage(&cas.limits);
        let result = self.retained(cas, context, original, budget);
        budget.restore_limits(previous);
        result
    }

    /// Authenticate all original identities before either reuse or fresh admission.
    fn retained<'a>(
        &'a mut self,
        cas: &Cas,
        context: &Handle,
        original: &Handle,
        budget: &mut Budget,
    ) -> Result<&'a PreparedConstant, Fault> {
        let expression = cas.checked(original)?;
        let frame = cas.checked(context)?;
        if cas.state.expression(expression)?.header != cas.state.context(frame)?.header_id {
            return Err(Fault::invalid(InvalidCode::Scope, AT));
        }
        let same_owner = self
            .owner
            .as_ref()
            .is_some_and(|owner| Arc::ptr_eq(owner, &cas.memo_owner));
        let same_epoch = self.epoch == cas.memo_epoch && cas.memo_epoch != u64::MAX;
        let same_frame = same_owner && same_epoch;
        let key = (frame, expression);
        if same_frame && self.entries.contains_key(&key) {
            budget.work(
                u64::from(size(self.entries.len(), AT)?.bit_width().max(1)),
                AT,
            )?;
            retained(self.bytes, self.entries.len(), budget)?;
            return self.entries.get(&key).ok_or_else(invalid);
        }
        let value = budget.with_retained_payload(self.bytes, AT, |budget| {
            ExactConstant::from_expression(cas, original, budget)
        })?;
        let old_bytes = if same_frame { self.bytes } else { 0 };
        let count = if same_frame { self.entries.len() } else { 0 };
        let bytes = old_bytes.saturating_add(footprint(&value)?);
        retained(bytes, count.saturating_add(1), budget)?;
        if !same_frame {
            self.entries.clear();
            self.owner = Some(Arc::clone(&cas.memo_owner));
            self.epoch = cas.memo_epoch;
        }
        self.entries.insert(
            key,
            PreparedConstant {
                original: original.clone(),
                context: context.clone(),
                value,
            },
        );
        self.bytes = bytes;
        self.entries.get(&key).ok_or_else(invalid)
    }
}

impl PreparedConstant {
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

/// Bound all simultaneously retained local evidence under the caller's storage ceilings.
fn retained(bytes: u64, count: usize, budget: &Budget) -> Result<(), Fault> {
    budget.check(BudgetKind::OutputBytes, bytes, AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.check(BudgetKind::Vertices, size(count, AT)?, AT)
}

/// Include the owned exact graph, complete canonical bytes and conservative entry metadata.
fn footprint(value: &ExactConstant) -> Result<u64, Fault> {
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
            .saturating_add(size(graph.key.len(), AT)?),
    };
    Ok(payload.saturating_add(512))
}

/// Missing retained evidence cannot manufacture a checked result.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Reference, AT)
}

#[cfg(test)]
mod tests;
