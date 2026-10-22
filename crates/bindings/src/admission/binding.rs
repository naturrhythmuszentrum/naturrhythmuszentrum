// SPDX-License-Identifier: AGPL-3.0-only
//! Storage-neutral checked associations between exact symbols and opaque region tickets.

use super::{AdmissionError, BindingSymbols};

/// Immutable value and companion binding; tickets grant no live storage access.
#[derive(Clone, Debug)]
pub struct SymbolBinding<T> {
    /// Exact declared symbol inventory.
    symbols: BindingSymbols,
    /// Value, deterministic-bound and numerical-error tickets, in that fixed order.
    regions: [Option<T>; 3],
}

impl<T: Eq> SymbolBinding<T> {
    /// Join declared symbols to distinct numerical-region tickets.
    ///
    /// The numerical owner must resolve every ticket and its current version under
    /// an exclusive lease. This check validates only the exact binding inventory.
    /// # Errors
    /// Refuses missing/extra regions or a repeated region with two symbol owners.
    pub fn check(symbols: BindingSymbols, regions: [Option<T>; 3]) -> Result<Self, AdmissionError> {
        symbols.validate()?;
        let presence = [true, symbols.bound().is_some(), symbols.error().is_some()];
        for (index, (region, required)) in regions.iter().zip(presence).enumerate() {
            if region.is_some() != required {
                return Err(AdmissionError::RegionInventory);
            }
            if let Some(ticket) = region
                && regions[..index]
                    .iter()
                    .flatten()
                    .any(|earlier| earlier == ticket)
            {
                return Err(AdmissionError::RegionInventory);
            }
        }
        Ok(Self { symbols, regions })
    }
}

impl<T> SymbolBinding<T> {
    /// Exact declaration inventory retained by this selected binding.
    #[must_use]
    pub const fn symbols(&self) -> &BindingSymbols {
        &self.symbols
    }
    /// Ordered opaque value, deterministic-bound and numerical-error region tickets.
    #[must_use]
    pub const fn regions(&self) -> &[Option<T>; 3] {
        &self.regions
    }
}
