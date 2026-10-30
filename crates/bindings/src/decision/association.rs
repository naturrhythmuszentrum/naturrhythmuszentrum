// SPDX-License-Identifier: AGPL-3.0-only
//! Sealed original-source associations precede numerical allocation and live admission.

use super::{DecisionError, DecisionExpression, IntegerDecisionCase, storage};
use crate::{BindingSymbols, CheckedExpression, Declaration, Descriptor, Role};
use nrz_cas::limits::Budget;
use std::rc::Rc;

/// Complete original physical inputs of one declared decision, in declaration-key order.
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
    /// Complete original constant and explicit-family dependency inventory.
    operands: Vec<DecisionOperand>,
    /// New association metadata and symbol payloads, excluding shared original graphs.
    bytes: u64,
}

/// One original physical operand and its designated coherent numerical symbol bundle.
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
    /// Declare the complete original input association before importing numerical values.
    ///
    /// The original check supplies the transitive declaration inventory. Closed constants
    /// and explicitly admitted input families retain their original physical maps and
    /// receive coherent scalar symbols with both radii.
    /// # Errors
    /// Refuses inputs without declared families, points, counts and caller resource exhaustion.
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
                | Declaration::Input { descriptor, .. }
                    if descriptor.role() == Role::Difference
                        && descriptor.kind().count_domain().is_none()
                        && (matches!(declaration, Declaration::Constant { .. })
                            || source.input_families().iter().any(|(key, _)| key == name)) =>
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
            if let Declaration::Constant { descriptor, .. }
            | Declaration::Input { descriptor, .. } = declaration
            {
                operands.push(operand(source, name, descriptor, &mut bytes, budget)?);
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
        authenticate(&self.0.operands, symbols, budget)
    }
}

impl DecisionOperand {
    /// Explicit original source box and qualified premises, when this operand varies physically.
    #[must_use]
    pub fn family(&self) -> Option<&nrz_cas::decision::ScalarKernelFamily> {
        self.source
            .input_families()
            .iter()
            .find_map(|(name, family)| (name == &self.name).then_some(family))
    }
    /// Original exact affine definition, when this numerical operand supplies a selected source.
    #[must_use]
    pub fn reading(&self) -> Option<&crate::readings::PhysicalReading> {
        self.source
            .input_readings()
            .iter()
            .find_map(|(name, reading)| (name == &self.name).then_some(reading))
    }
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

/// Construct one designated symbol bundle with the existing retained-input accounting.
pub(super) fn operand(
    source: &DecisionExpression,
    name: &str,
    descriptor: &Descriptor,
    bytes: &mut u64,
    budget: &mut Budget,
) -> Result<DecisionOperand, DecisionError> {
    let coherent = descriptor.coherent();
    let symbols = budget.with_retained_payload(*bytes, storage::AT, |budget| {
        BindingSymbols::declare(&coherent, [1, 1], [true, true], budget)
    })?;
    let symbol_bytes = symbols.value().cas()?.counts().payload_bytes;
    *bytes = bytes
        .checked_add(symbol_bytes)
        .ok_or(DecisionError::Capacity)?;
    storage::retained(*bytes, budget)?;
    Ok(DecisionOperand {
        name: name.into(),
        source: source.clone(),
        symbols,
    })
}

/// Authenticate complete ordered value and companion identities for either decision shape.
pub(super) fn authenticate(
    operands: &[DecisionOperand],
    symbols: &[&BindingSymbols],
    budget: &mut Budget,
) -> Result<(), DecisionError> {
    budget.work(1, storage::AT)?;
    if symbols.len() != operands.len() {
        return Err(DecisionError::Association);
    }
    for (actual, expected) in symbols.iter().zip(operands) {
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
