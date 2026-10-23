// SPDX-License-Identifier: AGPL-3.0-only
//! A fixed-reactance law changes covariance units through checked original equations.

use super::{
    CompanionParameters, CompanionReading, CompanionReadingError, CompanionReadingKind, parameters,
};
use crate::{
    Budget,
    readings::{PhasorCovarianceLaw, ReadingError, support},
};
use nrz_cas::{Cas, Handle};

impl CompanionReading {
    /// Define current covariance from the original shared voltage covariance through both exact gains.
    ///
    /// This affine finite-moment law requires both original impedances fixed and nonzero.
    /// It retains the checked equations instead of treating output current parameters as sources.
    /// Numerical evaluation, source covariance validity and parameter errors remain separate.
    /// # Errors
    /// Refuses unrelated sources, changed target units, varying impedance, wrong output shape and limits.
    pub fn phasor_covariance(
        cas: &mut Cas,
        symbol: &Handle,
        parameters: &CompanionParameters,
        law: PhasorCovarianceLaw,
        budget: &mut Budget,
    ) -> Result<Self, CompanionReadingError> {
        if parameters.kind() != CompanionReadingKind::Covariance {
            return Err(CompanionReadingError::Kind);
        }
        if &parameters.context != law.equations()[0].voltage().reading().family().context() {
            return Err(CompanionReadingError::Target);
        }
        for (original, equation) in parameters.targets.iter().zip(law.equations()) {
            let expected = parameters::parameter_target(
                CompanionReadingKind::Covariance,
                equation.voltage().reading(),
                budget,
            )?;
            if original != &expected {
                return Err(CompanionReadingError::Target);
            }
        }
        if symbol == parameters.node()
            || law.targets().iter().any(|target| target.symbol() == symbol)
        {
            return Err(CompanionReadingError::Target);
        }
        let (sort, _, _, header) = support::inspect(cas, symbol, budget)?;
        let rows =
            u32::try_from(law.targets()[0].entries().len()).map_err(|_| ReadingError::Shape)?;
        let columns =
            u32::try_from(law.targets()[1].entries().len()).map_err(|_| ReadingError::Shape)?;
        if header != cas.context_header(&parameters.context)?
            || support::shape(&sort)? != [rows, columns]
        {
            return Err(ReadingError::Shape.into());
        }
        let targets = law.targets().to_vec();
        let output = targets
            .iter()
            .map(|target| parameters::ParameterTarget {
                node: target.node().clone(),
                entries: target.entries().to_vec(),
            })
            .collect::<Vec<_>>();
        let units = parameters::units(CompanionReadingKind::Covariance, &output, budget)?;
        let node = cas.define_reading(symbol, parameters.node(), law.map(), budget)?;
        Ok(Self {
            parameters: parameters.clone(),
            symbol: symbol.clone(),
            node,
            targets,
            map: law.map().clone(),
            units,
            phasor: Some(law),
        })
    }
}
