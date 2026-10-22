// SPDX-License-Identifier: AGPL-3.0-only
//! Numerical error in a companion parameter targets that parameter, not its physical values.

use super::super::{ReadingError, maps, support};
use super::{CompanionReading, CompanionReadingError, parameters};
use crate::{Descriptor, PairingWitness, UncertaintyMode};
use nrz_cas::{Cas, Handle, limits::Budget, readings::AffineMap, syntax::Operator};

/// Checked transport of numerical parameter error through a companion's linear map.
#[derive(Clone, Debug)]
pub struct ParameterErrorReading {
    /// Actual output companion whose numerical parameter error is bounded.
    target: CompanionReading,
    /// Distinct source numerical-error declaration targeting the original parameter symbol.
    source: Handle,
    /// Actual defined output error symbol.
    node: Handle,
    /// Absolute linear companion map, with no affine offset.
    map: AffineMap,
    /// Numerical-error difference quantities of the companion parameter entries.
    units: Vec<PairingWitness>,
}

impl ParameterErrorReading {
    /// Derive parameter-error transport while retaining its exact input and output targets.
    ///
    /// This transports prior parameter evaluation error. The numerical owner must
    /// additionally bound the evaluation/import error of this map itself.
    /// # Errors
    /// Refuses substituted source/output symbols, wrong shapes and exact resource limits.
    pub fn define(
        cas: &mut Cas,
        symbol: &Handle,
        source: &Handle,
        target: &CompanionReading,
        budget: &mut Budget,
    ) -> Result<Self, CompanionReadingError> {
        let (input_sort, _, _, _) = support::inspect(cas, target.parameters().node(), budget)?;
        let (source_sort, operator, _, header) = support::inspect(cas, source, budget)?;
        let context = &target.parameters.context;
        let declared = matches!(operator, Operator::Free(_));
        if !declared
            || source_sort != input_sort
            || header != cas.context_header(context)?
            || source == target.parameters().node()
        {
            return Err(CompanionReadingError::Target);
        }
        let (output_sort, _, _, _) = support::inspect(cas, target.node(), budget)?;
        let (sort, _, _, header) = support::inspect(cas, symbol, budget)?;
        if sort != output_sort
            || header != cas.context_header(context)?
            || symbol == target.symbol()
            || symbol == source
            || symbol == target.parameters().node()
        {
            return Err(CompanionReadingError::Target);
        }
        let mut units = support::reserve(target.units().len(), budget)?;
        for pairing in target.units() {
            let parameter = pairing
                .uncertainty_descriptor()
                .ok_or(ReadingError::Meaning)?;
            units.push(PairingWitness::uncertainty(
                UncertaintyMode::Radius,
                parameter,
                None,
                None,
            )?);
        }
        let map = maps::radius(target.map(), budget)?;
        let node = cas.define_reading(symbol, source, &map, budget)?;
        Ok(Self {
            target: target.clone(),
            source: source.clone(),
            node,
            map,
            units,
        })
    }
    /// Exact output numerical-error symbol.
    #[must_use]
    pub const fn node(&self) -> &Handle {
        &self.node
    }
    /// Actual companion parameter target; never a physical value target by substitution.
    #[must_use]
    pub const fn target(&self) -> &CompanionReading {
        &self.target
    }
    /// Original parameter-error symbol.
    #[must_use]
    pub const fn source(&self) -> &Handle {
        &self.source
    }
    /// Original parameter value whose evaluation error the source symbol bounds.
    #[must_use]
    pub const fn source_target(&self) -> &Handle {
        self.target.parameters().node()
    }
    /// Exact absolute parameter transformation.
    #[must_use]
    pub const fn map(&self) -> &AffineMap {
        &self.map
    }
    /// Exact per-entry parameter error units.
    #[must_use]
    pub fn units(&self) -> &[PairingWitness] {
        &self.units
    }
    /// Match numerical parameter-error descriptors to this checked target.
    /// # Errors
    /// Refuses physical uncertainty, another parameter or a changed measure/offset.
    pub fn check_units(&self, actual: &[Descriptor]) -> Result<(), CompanionReadingError> {
        parameters::check_units(&self.units, actual)
    }
}
