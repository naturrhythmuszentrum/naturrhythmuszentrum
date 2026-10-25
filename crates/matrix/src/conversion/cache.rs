// SPDX-License-Identifier: AGPL-3.0-only
//! Shared immutable lowering keeps large exact maps off consumer stacks.

use super::{ConversionError, PiConstant, PiConstantEvidence, pi::CertifiedMap};
use crate::coordinate_numeric::LoweredMap;
use nrz_bindings::CheckedMap;
use nrz_scalar::{ArithmeticContext, LiteralLimits, Precision, Scalar};
use std::{any::Any, rc::Rc};

/// A format-specific shared numerical owner; certified entries also retain pi.
#[derive(Clone, Debug)]
enum Lowered {
    /// Ordinary rational coefficients in binary32.
    Single(Rc<LoweredMap<f32>>),
    /// Ordinary rational coefficients in binary64.
    Double(Rc<LoweredMap<f64>>),
    /// Certified pi coefficients in binary32.
    CertifiedSingle(Rc<CertifiedMap<f32>>),
    /// Certified pi coefficients in binary64.
    CertifiedDouble(Rc<CertifiedMap<f64>>),
}

impl Lowered {
    /// Borrow the concrete shared coefficient owner through sealed format erasure.
    fn erased_map(&self) -> &dyn Any {
        match self {
            Self::Single(value) => value,
            Self::Double(value) => value,
            Self::CertifiedSingle(value) => &value.map,
            Self::CertifiedDouble(value) => &value.map,
        }
    }
    /// Borrow a certified owner when pi admission supplied its evidence.
    fn erased_pi(&self) -> Option<&dyn Any> {
        match self {
            Self::CertifiedSingle(value) => Some(value),
            Self::CertifiedDouble(value) => Some(value),
            Self::Single(_) | Self::Double(_) => None,
        }
    }
    /// Return the sealed format without inspecting numerical data.
    const fn precision(&self) -> Precision {
        match self {
            Self::Single(_) | Self::CertifiedSingle(_) => Precision::F32,
            Self::Double(_) | Self::CertifiedDouble(_) => Precision::F64,
        }
    }
}

/// At most two immutable numerical owners, one per sealed format.
#[derive(Debug, Default)]
pub(crate) struct ConstantCache {
    /// At most one admitted numerical owner for each sealed format.
    entries: Vec<Lowered>,
}

/// A fully allocated and type-checked entry awaiting the one ledger commit.
#[derive(Debug)]
pub(crate) struct PreparedPi {
    /// Prepared immutable entry.
    entry: Lowered,
}

impl ConstantCache {
    /// Reuse matching coefficients or lower rational coefficients with the ordinary decoder.
    pub(crate) fn get<P: Scalar + 'static>(
        &mut self,
        map: &Rc<CheckedMap>,
        limits: LiteralLimits,
        context: &ArithmeticContext,
    ) -> Result<Rc<LoweredMap<P>>, ConversionError> {
        for entry in &self.entries {
            if let Some(value) = entry.erased_map().downcast_ref::<Rc<LoweredMap<P>>>()
                && value.revision == context.revision()
                && value.exact == *map
            {
                return Ok(Rc::clone(value));
            }
        }
        let lowered = Rc::new(LoweredMap::<P>::new(map, limits, context)?);
        let erased = &lowered as &dyn Any;
        let entry = match P::PRECISION {
            Precision::F32 => Lowered::Single(Rc::clone(
                erased
                    .downcast_ref::<Rc<LoweredMap<f32>>>()
                    .ok_or(ConversionError::Contract)?,
            )),
            Precision::F64 => Lowered::Double(Rc::clone(
                erased
                    .downcast_ref::<Rc<LoweredMap<f64>>>()
                    .ok_or(ConversionError::Contract)?,
            )),
        };
        self.reserve()?;
        self.entries.push(entry);
        Ok(lowered)
    }

    /// Find a compatible map retaining the identical original pi allocation.
    pub(crate) fn find_pi<P: Scalar + 'static>(
        &self,
        map: &Rc<CheckedMap>,
        pi: &PiConstant<P>,
        context: &ArithmeticContext,
    ) -> Option<Rc<CertifiedMap<P>>> {
        self.entries.iter().find_map(|entry| {
            let value = entry.erased_pi()?.downcast_ref::<Rc<CertifiedMap<P>>>()?;
            let same_map = value.map.exact == *map;
            let same_region = value.pi.same_region(pi);
            let same_revision = value.map.revision == context.revision();
            (same_map && same_region && same_revision).then(|| Rc::clone(value))
        })
    }

    /// Retain pi evidence only for this cache's exact coefficient allocation.
    pub(crate) fn evidence<P: Scalar + 'static>(
        &self,
        map: &Rc<LoweredMap<P>>,
    ) -> Option<PiConstantEvidence> {
        self.entries.iter().find_map(|entry| {
            let value = entry.erased_pi()?.downcast_ref::<Rc<CertifiedMap<P>>>()?;
            Rc::ptr_eq(map, &value.map).then(|| value.pi.evidence())
        })
    }

    /// Permit local sharing only when rational or backed by this cache's pi owner.
    pub(crate) fn compatible<P: Scalar + 'static>(&self, map: &Rc<LoweredMap<P>>) -> bool {
        let rational = map.exact.scale().exact().as_rational().is_some()
            && map.exact.shift().exact().as_rational().is_some();
        rational || self.evidence(map).is_some()
    }

    /// Charge one lookup entry plus each currently retained format.
    pub(crate) fn lookup_work(&self) -> Result<u64, ConversionError> {
        u64::try_from(self.entries.len() + 1).map_err(|_| ConversionError::ResourceLimit)
    }

    /// Logical bytes of one retained cache slot.
    pub(crate) const fn entry_bytes() -> usize {
        size_of::<Lowered>()
    }

    /// Whether this cache already owns the one slot for the sealed format.
    pub(crate) fn contains<P: Scalar>(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.precision() == P::PRECISION)
    }

    /// Reserve a missing format slot before committing caller Work.
    pub(crate) fn prepare_pi<P: Scalar + 'static>(&mut self) -> Result<(), ConversionError> {
        if self
            .entries
            .iter()
            .any(|entry| entry.precision() == P::PRECISION)
        {
            return Ok(());
        }
        self.reserve()
    }

    /// Complete fallible allocation and sealed format erasure before publication.
    pub(crate) fn stage_pi<P: Scalar + 'static>(
        &mut self,
        value: &Rc<CertifiedMap<P>>,
    ) -> Result<PreparedPi, ConversionError> {
        self.prepare_pi::<P>()?;
        let erased = value as &dyn Any;
        let entry = match P::PRECISION {
            Precision::F32 => Lowered::CertifiedSingle(Rc::clone(
                erased
                    .downcast_ref::<Rc<CertifiedMap<f32>>>()
                    .ok_or(ConversionError::Contract)?,
            )),
            Precision::F64 => Lowered::CertifiedDouble(Rc::clone(
                erased
                    .downcast_ref::<Rc<CertifiedMap<f64>>>()
                    .ok_or(ConversionError::Contract)?,
            )),
        };
        Ok(PreparedPi { entry })
    }

    /// Publish a previously prepared entry without further fallible work.
    pub(crate) fn commit_pi(&mut self, prepared: PreparedPi) {
        if let Some(previous) = self
            .entries
            .iter_mut()
            .find(|entry| entry.precision() == prepared.entry.precision())
        {
            *previous = prepared.entry;
        } else {
            self.entries.push(prepared.entry);
        }
    }

    /// Reserve one logical format slot, bounded by the two sealed formats.
    fn reserve(&mut self) -> Result<(), ConversionError> {
        if self.entries.len() >= 2 {
            return Err(ConversionError::ResourceLimit);
        }
        self.entries
            .try_reserve_exact(1)
            .map_err(|_| ConversionError::ResourceLimit)
    }
}
