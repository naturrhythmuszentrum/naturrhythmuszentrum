// SPDX-License-Identifier: AGPL-3.0-only
//! Lattice originals share the scalar symbol constructor and complete identity authenticator.

use super::{DecisionError, LatticeDecisionCase, storage::length};
use crate::{
    BindingSymbols, DecisionExpression, DecisionOperand, Declaration, Descriptor, Kind,
    decision::{association, storage},
};
use nrz_cas::limits::Budget;
use std::{collections::BTreeSet, rc::Rc};

/// Complete original constant inventory, ordered by first physical occurrence and declaration key.
#[derive(Clone, Debug)]
pub struct LatticeInputs(Rc<Data>);

/// The original question and every designated value/companion symbol stay alive together.
#[derive(Debug)]
struct Data {
    /// Original axes, source owners, exact maps, ranges and cell convention.
    case: LatticeDecisionCase,
    /// Each distinct original constant appears once, even across several coordinates.
    operands: Vec<DecisionOperand>,
    /// Newly owned metadata and symbol payload, excluding shared original owners.
    bytes: u64,
}

/// Borrow an original constant without copying its physical descriptor or evidence.
struct Original<'a> {
    /// The exact source owner supplies the identity namespace.
    source: &'a DecisionExpression,
    /// Original declaration key in that owner.
    name: &'a str,
    /// Exact checked original unit map and point/difference role.
    descriptor: &'a Descriptor,
}

impl LatticeDecisionCase {
    /// Associate all original target, offset and basis inputs before numerical allocation.
    ///
    /// Shared sources deduplicate by frozen owner identity and declaration key, never
    /// by their values. Canceled and zero-multiplied originals remain in the inventory.
    /// # Errors
    /// Refuses open or unsupported inputs and exhausted cumulative caller resources.
    /// No input association is returned until the whole symbol inventory is complete.
    pub fn associate_inputs(&self, budget: &mut Budget) -> Result<LatticeInputs, DecisionError> {
        let mut originals = Vec::new();
        let mut seen = BTreeSet::new();
        let mut bytes = 128_u64;
        for axis in &self.specification().axes {
            for source in [&axis.target, &axis.offset].into_iter().chain(&axis.basis) {
                inventory(
                    source.source(),
                    &mut originals,
                    &mut seen,
                    &mut bytes,
                    budget,
                )?;
            }
        }
        storage::retained(bytes, budget)?;
        budget.work(bytes, storage::AT)?;
        let mut operands = Vec::new();
        operands
            .try_reserve_exact(originals.len())
            .map_err(|_| DecisionError::Capacity)?;
        let scratch = u64::try_from(originals.len())
            .map_err(|_| DecisionError::Capacity)?
            .saturating_mul(128);
        budget.with_retained_payload(scratch, storage::AT, |budget| {
            for original in originals {
                operands.push(association::operand(
                    original.source,
                    original.name,
                    original.descriptor,
                    &mut bytes,
                    budget,
                )?);
            }
            Ok::<_, DecisionError>(())
        })?;
        Ok(LatticeInputs(Rc::new(Data {
            case: self.clone(),
            operands,
            bytes,
        })))
    }
}

/// Keep the first physical occurrence of each original declaration, even at equal values.
fn inventory<'a>(
    source: &'a DecisionExpression,
    originals: &mut Vec<Original<'a>>,
    seen: &mut BTreeSet<(usize, &'a str)>,
    bytes: &mut u64,
    budget: &mut Budget,
) -> Result<(), DecisionError> {
    for (name, declaration) in source.checked()?.declarations() {
        budget.work(1, storage::AT)?;
        let descriptor = match declaration {
            Declaration::Constant { descriptor, .. }
                if matches!(
                    descriptor.kind(),
                    Kind::Ratio | Kind::Angle | Kind::Named(..)
                ) =>
            {
                descriptor
            }
            Declaration::Derived(_) => continue,
            _ => return Err(DecisionError::Source),
        };
        budget.work(length(name)?.saturating_add(1), storage::AT)?;
        let key = (std::ptr::from_ref(source.owner()).addr(), name.as_str());
        if !seen.insert(key) {
            continue;
        }
        *bytes = bytes
            .checked_add(length(name)?)
            .and_then(|value| value.checked_add(128))
            .ok_or(DecisionError::Capacity)?;
        storage::retained(*bytes, budget)?;
        originals
            .try_reserve(1)
            .map_err(|_| DecisionError::Capacity)?;
        originals.push(Original {
            source,
            name,
            descriptor,
        });
    }
    Ok(())
}

impl LatticeInputs {
    /// Complete original physical question, with no live numerical authority.
    #[must_use]
    pub fn case(&self) -> &LatticeDecisionCase {
        &self.0.case
    }

    /// Distinct originals in deterministic first-occurrence order.
    #[must_use]
    pub fn operands(&self) -> &[DecisionOperand] {
        &self.0.operands
    }

    /// Newly owned symbol and association payload; original source graphs remain shared.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }

    /// Authenticate the original question and every designated value and companion identity.
    /// # Errors
    /// Refuses another source/map/convention declaration or missing, reordered or equal new symbols.
    pub fn authenticate(
        &self,
        case: &LatticeDecisionCase,
        symbols: &[&BindingSymbols],
        budget: &mut Budget,
    ) -> Result<(), DecisionError> {
        case.validate_attachment(&self.0.case.attachment(), budget)?;
        association::authenticate(&self.0.operands, symbols, budget)
    }
}
