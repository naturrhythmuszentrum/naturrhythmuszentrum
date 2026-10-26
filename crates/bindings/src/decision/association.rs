// SPDX-License-Identifier: AGPL-3.0-only
//! Sealed original-source associations precede numerical allocation and live admission.

use super::{DecisionError, DecisionExpression, IntegerDecisionCase, storage};
use crate::{BindingSymbols, CheckedExpression, Declaration, Role};
use nrz_cas::limits::Budget;
use std::rc::Rc;

/// Complete original constant inputs of one declared decision, in declaration-key order.
///
/// Each distinct original occurs once; the retained original expression preserves all
/// operand occurrences, including repetition and operands removed by normalization.
/// These declarations authorize an external association, not an enclosure theorem.
#[derive(Clone, Debug)]
pub struct DecisionInputs(Rc<Data>);

/// Immutable case identity and the only numerical symbols assigned to its originals.
#[derive(Debug)]
struct Data {
    /// Exact source, maps, case and parameter instance.
    case: IntegerDecisionCase,
    /// Complete original constant dependency inventory.
    operands: Vec<DecisionOperand>,
    /// New association metadata and symbol payloads, excluding shared original graphs.
    bytes: u64,
}

/// One original physical constant and its designated coherent numerical symbol bundle.
#[derive(Clone, Debug)]
pub struct DecisionOperand {
    /// Original name, never used to authenticate a foreign owner.
    name: String,
    /// Retains every original declaration and exact map without copying its graph.
    source: DecisionExpression,
    /// Fresh value, total-bound and numerical-error identities assigned before storage.
    symbols: BindingSymbols,
}

impl IntegerDecisionCase {
    /// Declare the complete nominal input association before importing numerical values.
    ///
    /// The original check supplies the transitive declaration inventory. Only closed
    /// constant leaves and derived expressions are supported. Each constant retains its
    /// original physical map and receives coherent scalar symbols with both radii.
    /// # Errors
    /// Refuses open/uncertainty inputs, points, counts and caller resource exhaustion.
    /// No association or mutation of the original declaration graph survives refusal.
    pub fn associate_inputs(&self, budget: &mut Budget) -> Result<DecisionInputs, DecisionError> {
        let source = self.source();
        let original = source.owner().checked(source.name())?;
        let mut bytes = 128_u64;
        let mut count = 0_usize;
        for (name, declaration) in original.declarations() {
            budget.work(1, storage::AT)?;
            match declaration {
                Declaration::Constant { descriptor, .. }
                    if descriptor.role() == Role::Difference
                        && descriptor.kind().count_domain().is_none() =>
                {
                    count = count.checked_add(1).ok_or(DecisionError::Capacity)?;
                    bytes = bytes
                        .checked_add(
                            u64::try_from(name.len()).map_err(|_| DecisionError::Capacity)?,
                        )
                        .and_then(|value| value.checked_add(128))
                        .ok_or(DecisionError::Capacity)?;
                }
                Declaration::Derived(_) => {}
                _ => return Err(DecisionError::Source),
            }
        }
        storage::retained(bytes, budget)?;
        budget.work(bytes, storage::AT)?;
        let mut operands = Vec::new();
        operands
            .try_reserve_exact(count)
            .map_err(|_| DecisionError::Capacity)?;
        for (name, declaration) in original.declarations() {
            if let Declaration::Constant { descriptor, .. } = declaration {
                let coherent = descriptor.coherent();
                let symbols = budget.with_retained_payload(bytes, storage::AT, |budget| {
                    BindingSymbols::declare(&coherent, [1, 1], [true, true], budget)
                })?;
                let symbol_bytes = symbols.value().cas()?.counts().payload_bytes;
                bytes = bytes
                    .checked_add(symbol_bytes)
                    .ok_or(DecisionError::Capacity)?;
                storage::retained(bytes, budget)?;
                operands.push(DecisionOperand {
                    name: name.clone(),
                    source: source.clone(),
                    symbols,
                });
            }
        }
        Ok(DecisionInputs(Rc::new(Data {
            case: self.clone(),
            operands,
            bytes,
        })))
    }
}

impl DecisionInputs {
    /// Original complete case identity, including its exact parameter instance.
    #[must_use]
    pub fn case(&self) -> &IntegerDecisionCase {
        &self.0.case
    }
    /// Canonically ordered distinct originals; no caller supplies or truncates this list.
    #[must_use]
    pub fn operands(&self) -> &[DecisionOperand] {
        &self.0.operands
    }
    /// New metadata and symbol payload allowance; original declaration graphs remain shared.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
    /// Authenticate the full case and every designated value and companion identity.
    ///
    /// The receiver must obtain these symbols from its actual frozen inputs, then
    /// separately authenticate their heap versions and numerical enclosure scope.
    /// # Errors
    /// Refuses a different source, map, case, parameter, order or companion inventory.
    pub fn authenticate(
        &self,
        case: &IntegerDecisionCase,
        symbols: &[&BindingSymbols],
        budget: &mut Budget,
    ) -> Result<(), DecisionError> {
        case.validate_attachment(&self.0.case.attachment(), budget)?;
        budget.work(1, storage::AT)?;
        if symbols.len() != self.0.operands.len() {
            return Err(DecisionError::Association);
        }
        for (actual, expected) in symbols.iter().zip(&self.0.operands) {
            budget.work(3, storage::AT)?;
            if actual.value() != expected.symbols.value()
                || actual.bound() != expected.symbols.bound()
                || actual.error() != expected.symbols.error()
            {
                return Err(DecisionError::Association);
            }
        }
        Ok(())
    }
}

impl DecisionOperand {
    /// Original declaration key, scoped by the retained physical source owner.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Original physical descriptor, exact unit map and checked original syntax.
    /// # Errors
    /// Refuses an unavailable original declaration rather than supplying a substitute.
    pub fn original(&self) -> Result<&CheckedExpression, DecisionError> {
        Ok(self.source.owner().checked(&self.name)?)
    }
    /// Designated coherent value and both companions, declared before numerical storage.
    #[must_use]
    pub const fn symbols(&self) -> &BindingSymbols {
        &self.symbols
    }
}
