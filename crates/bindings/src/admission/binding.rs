// SPDX-License-Identifier: AGPL-3.0-only
//! Storage-neutral associations for the closed homogeneous recipe.

use super::{AdmissionError, BindingSymbols};

/// Immutable selected value and companion association; tickets grant no live authority.
///
/// Ticket equality is not an entry-set or map judgment. The numerical owner must
/// resolve actual coverage and checked overlaps under its exclusive lease.
#[derive(Clone, Debug)]
pub struct SymbolBinding<T> {
    /// Exact homogeneous recipe declaration, not a restriction on numerical storage.
    symbols: BindingSymbols,
    /// Present value and companion tickets in declaration order, without empty slots.
    regions: Vec<T>,
}

impl<T> SymbolBinding<T> {
    /// Join the complete declared recipe inventory to opaque selected-region tickets.
    ///
    /// Shared tickets neither establish compatibility nor constitute a refusal here:
    /// actual entry coverage and its checked relations belong to live admission.
    /// # Errors
    /// Refuses missing or extra associations and invalid companion declarations.
    pub fn check(symbols: BindingSymbols, regions: Vec<T>) -> Result<Self, AdmissionError> {
        symbols.validate()?;
        let required =
            1 + usize::from(symbols.bound().is_some()) + usize::from(symbols.error().is_some());
        if regions.len() != required {
            return Err(AdmissionError::RegionInventory);
        }
        Ok(Self { symbols, regions })
    }

    /// Exact declaration inventory retained by this selected binding.
    #[must_use]
    pub const fn symbols(&self) -> &BindingSymbols {
        &self.symbols
    }
    /// Present opaque region tickets in the same order as their declared symbols.
    #[must_use]
    pub fn regions(&self) -> &[T] {
        &self.regions
    }
}
