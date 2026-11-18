// SPDX-License-Identifier: AGPL-3.0-only
//! Receiving-policy meet and bounded original-store identity inventory.

use super::{AT, Result, accounting};
use crate::lc_omega::owner::Originals;
use crate::{BindingSymbols, QuantitySymbol};
use nrz_cas::limits::{Budget, Limits};

/// Visit complete input policies, but include the output only in the residence inventory.
pub(super) fn receiving(
    originals: &Originals,
    mut policy: Limits,
    budget: &mut Budget,
) -> Result<Limits> {
    let capacity = originals
        .inputs
        .len()
        .checked_mul(6)
        .and_then(|n| n.checked_add(3))
        .ok_or(crate::AdmissionError::ResourceLimit)?;
    let mut stores = accounting::allocate::<&QuantitySymbol>(capacity, budget)?;
    let bytes = accounting::vector::<&QuantitySymbol>(stores.capacity())?;
    budget.with_retained_payload(bytes, AT, |budget| {
        accounting::original(2, budget)?;
        for pair in &originals.inputs {
            for bundle in pair {
                for symbol in symbols(bundle)? {
                    accounting::expression(1, budget)?;
                    policy = policy.meet(symbol.cas()?.limits());
                    inventory(symbol, &mut stores, budget)?;
                }
            }
        }
        for symbol in symbols(&originals.output)? {
            inventory(symbol, &mut stores, budget)?;
        }
        Ok(policy)
    })
}

/// A captured admitted bundle always has both independently targeted companions.
fn symbols(bundle: &BindingSymbols) -> Result<[&QuantitySymbol; 3]> {
    Ok([
        bundle.value(),
        bundle
            .bound()
            .ok_or(crate::AdmissionError::CompanionMismatch)?,
        bundle
            .error()
            .ok_or(crate::AdmissionError::CompanionMismatch)?,
    ])
}

/// Borrowed symbol slots identify the exact stable underlying Cas without retaining new edges.
fn inventory<'a>(
    symbol: &'a QuantitySymbol,
    stores: &mut Vec<&'a QuantitySymbol>,
    budget: &mut Budget,
) -> Result<()> {
    accounting::original(1, budget)?;
    let current = symbol.cas()?;
    for previous in stores.iter() {
        accounting::original(1, budget)?;
        let previous = previous.cas()?;
        if std::ptr::eq(&raw const *current, &raw const *previous) {
            return Ok(());
        }
    }
    stores.push(symbol);
    Ok(())
}
