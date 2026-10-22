// SPDX-License-Identifier: AGPL-3.0-only
//! The full-cycle definition supplies an explicit relation between distinct frequency kinds.
//!
//! # Full-cycle frequency relation
//!
//! `Registry::frequency_relation` checks the distinct source and target kinds, difference roles and the registered full-cycle definition. Its private `FrequencyRelation` retains both actual coordinate descriptors, the typed turn angle, direction and exact CAS conversion. Forward coordinates multiply by the full-cycle angle and reverse coordinates divide by it; both endpoint measure scales participate. Thus `1000/pi Hz` becomes exactly `2000 rad/s`, and the inverse restores the exact original. `apply_exact` additionally checks the captured source schema, while ordinary same-kind conversion continues to refuse hertz versus angular frequency and derivative rate. The catalog's original frequency-times-angle multiplication rules support checked equations with retained source declarations; inverse cyclic interpretation uses the explicit full-cycle relation. Generic angular-frequency divided by angle retains the derivative-rate kind.

use nrz_cas::ExactConstant;

use super::Registry;
use crate::exact::budget;
use crate::{Conversion, Descriptor, Kind, Role, UnitError};

/// Direction of the exact full-cycle descriptor relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrequencyDirection {
    /// Angular frequency equals one full-cycle angle times cyclic frequency.
    ToAngular,
    /// Cyclic frequency equals angular frequency divided by the full-cycle angle.
    ToCyclic,
}

/// Checked descriptor relation with both kinds, the typed cycle angle and the exact CAS map.
///
/// It cannot be obtained from dimension equality or ordinary measure conversion.
/// Source and target descriptors retain their actual measures; the cycle is one
/// unit of the registered turn measure, including its exact `2*pi` scale.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrequencyRelation {
    /// Original source meaning and coordinate measure.
    source: Descriptor,
    /// Original target meaning and coordinate measure.
    target: Descriptor,
    /// Exact physical angle corresponding to one complete cycle.
    cycle: Descriptor,
    /// The selected mathematical direction, distinct from measure conversion.
    direction: FrequencyDirection,
    /// Exact coordinate map derived from all three descriptors.
    conversion: Conversion,
}

impl Registry {
    /// Check the registered full-cycle relation in either direction without identifying kinds.
    /// # Errors
    /// Refuses absent registration, wrong kinds/dimensions, affine points or exact budgets.
    pub fn frequency_relation(
        &self,
        source: &Descriptor,
        target: &Descriptor,
    ) -> Result<FrequencyRelation, UnitError> {
        self.validate(source)?;
        self.validate(target)?;
        let cycle = self
            .full_cycle
            .as_ref()
            .ok_or(UnitError::UnsupportedKindRule)?;
        let frequency = Kind::frequency()?;
        let angular = Kind::angular_frequency()?;
        if source.dimension() != &frequency.dimension()?
            || target.dimension() != &angular.dimension()?
        {
            return Err(UnitError::DimensionMismatch);
        }
        let forward = source.kind() == &frequency && target.kind() == &angular;
        let reverse = source.kind() == &angular && target.kind() == &frequency;
        let direction = if forward {
            FrequencyDirection::ToAngular
        } else if reverse {
            FrequencyDirection::ToCyclic
        } else {
            return Err(UnitError::KindMismatch);
        };
        FrequencyRelation::checked(source, target, cycle, direction)
    }
}

impl FrequencyRelation {
    /// Derive exact coefficients only after source/target kind selection and cycle admission.
    fn checked(
        source: &Descriptor,
        target: &Descriptor,
        cycle: &Descriptor,
        direction: FrequencyDirection,
    ) -> Result<Self, UnitError> {
        if source.role() != Role::Difference || target.role() != Role::Difference {
            return Err(UnitError::AffineRole);
        }
        let mut ledger = budget()?;
        let coherent = match direction {
            FrequencyDirection::ToAngular => source
                .measure()
                .scale()
                .mul(cycle.measure().scale(), &mut ledger)?,
            FrequencyDirection::ToCyclic => source
                .measure()
                .scale()
                .div(cycle.measure().scale(), &mut ledger)?,
        };
        let scale = coherent.div(target.measure().scale(), &mut ledger)?;
        Ok(Self {
            source: source.clone(),
            target: target.clone(),
            cycle: cycle.clone(),
            direction,
            conversion: Conversion {
                scale,
                shift: ExactConstant::zero(),
            },
        })
    }

    /// Source kind and measure captured when checking this relation.
    #[must_use]
    pub const fn source(&self) -> &Descriptor {
        &self.source
    }

    /// Target kind and measure; equal dimensions never erase its distinct kind.
    #[must_use]
    pub const fn target(&self) -> &Descriptor {
        &self.target
    }

    /// The full-cycle angle definition, retained independently of a reporting name.
    #[must_use]
    pub const fn cycle(&self) -> &Descriptor {
        &self.cycle
    }

    /// Explicit direction of this descriptor relation.
    #[must_use]
    pub const fn direction(&self) -> FrequencyDirection {
        self.direction
    }

    /// Exact coefficient graph for this typed relation, with zero shift.
    #[must_use]
    pub const fn conversion(&self) -> &Conversion {
        &self.conversion
    }

    /// Apply to an exact source-free coordinate with the captured source descriptor.
    /// # Errors
    /// Refuses a changed source schema or exhausted exact algebra budgets.
    pub fn apply_exact(
        &self,
        source: &Descriptor,
        coordinate: &ExactConstant,
    ) -> Result<ExactConstant, UnitError> {
        if source != &self.source {
            return Err(UnitError::RelationSource);
        }
        self.conversion.apply_exact(coordinate)
    }

    /// Reverse the relation while retaining the same full-cycle angle and both measures.
    /// # Errors
    /// Refuses exhausted exact coefficient arithmetic.
    pub fn inverse(&self) -> Result<Self, UnitError> {
        let direction = match self.direction {
            FrequencyDirection::ToAngular => FrequencyDirection::ToCyclic,
            FrequencyDirection::ToCyclic => FrequencyDirection::ToAngular,
        };
        Self::checked(&self.target, &self.source, &self.cycle, direction)
    }
}
