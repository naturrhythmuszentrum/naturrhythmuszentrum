// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit angle and rate vocabulary, separate from ordinary same-kind conversion.
//!
//! # Angle and rate vocabulary
//!
//! `Registry::with_angle_frequency` explicitly installs `rad`, `degree`, `turn`, `Hz`, `rad/s` and `s^-1`. Degrees have exact scale `pi/180` and turns `2*pi`; all six are differences with zero offset. `Kind::frequency`, `Kind::angular_frequency` and `Kind::rate` declare three distinct meanings at the same inverse-time dimension. The empty registry remains empty, and `register_measure` only registers a checked descriptor: a reporting key such as `turn` cannot create a physical relation.

use nrz_cas::{ExactConstant, Rational};

use super::{BinaryOp, Registry};
use crate::exact::{budget, ratio};
use crate::{Descriptor, ExponentPolicy, Kind, Measure, Role, Scale, UnitError};

impl Registry {
    /// Register radians, degrees, turns, hertz, angular frequency and derivative rate.
    ///
    /// Keys are `rad`, `degree`, `turn`, `Hz`, `rad/s` and `s^-1`. The three rate
    /// kinds remain distinct. Their full-cycle relation is selected explicitly by
    /// `Self::frequency_relation`, never by ordinary descriptor conversion.
    /// # Errors
    /// Refuses exact graph, descriptor or rule admission failures.
    pub fn with_angle_frequency(policy: ExponentPolicy) -> Result<Self, UnitError> {
        let mut registry = Self::new(policy);
        let pi = Scale::pi()?.value()?;
        let degree = pi.div(&ratio(180, 1)?.into(), &mut budget()?)?;
        let turn = pi.mul(&ratio(2, 1)?.into(), &mut budget()?)?;
        for (key, kind, scale) in [
            ("rad", Kind::Angle, ExactConstant::one()),
            ("degree", Kind::Angle, degree),
            ("turn", Kind::Angle, turn),
            ("Hz", Kind::frequency()?, ExactConstant::one()),
            ("rad/s", Kind::angular_frequency()?, ExactConstant::one()),
            ("s^-1", Kind::rate()?, ExactConstant::one()),
        ] {
            let descriptor = Descriptor::new(
                kind,
                Measure::new(key, Scale::Expression(scale), Rational::zero())?,
                Role::Difference,
            )?;
            registry.register_measure(key, &descriptor)?;
        }
        registry.register_cycle_rules()?;
        Ok(registry)
    }

    /// Register a reporting key only after checking the complete descriptor.
    /// # Errors
    /// Refuses empty or duplicate keys, conflicting kinds and inadmissible dimensions.
    pub fn register_measure(
        &mut self,
        key: impl Into<String>,
        descriptor: &Descriptor,
    ) -> Result<(), UnitError> {
        let key = key.into();
        if key.trim().is_empty() {
            return Err(UnitError::InvalidMeasureName);
        }
        if self.measures.contains_key(&key) {
            return Err(UnitError::DuplicateRule);
        }
        self.admit(descriptor)?;
        self.measures.insert(key, descriptor.clone());
        Ok(())
    }

    /// Look up a previously checked measure descriptor without interpreting its spelling.
    #[must_use]
    pub fn measure(&self, key: &str) -> Option<&Descriptor> {
        self.measures.get(key)
    }

    /// Forward products keep their original angle operand; the inverse cycle reading is explicit.
    fn register_cycle_rules(&mut self) -> Result<(), UnitError> {
        let frequency = Kind::frequency()?;
        let angular = Kind::angular_frequency()?;
        self.register_binary(
            BinaryOp::Multiply,
            frequency.clone(),
            Kind::Angle,
            angular.clone(),
        )?;
        self.register_binary(BinaryOp::Multiply, Kind::Angle, frequency, angular.clone())?;
        self.register_binary(
            BinaryOp::Multiply,
            Kind::rate()?,
            Kind::Angle,
            angular.clone(),
        )?;
        self.register_binary(BinaryOp::Divide, angular, Kind::Angle, Kind::rate()?)?;
        self.full_cycle = self.measures.get("turn").cloned();
        Ok(())
    }
}
