// SPDX-License-Identifier: AGPL-3.0-only
//! Checked rational factors of the fixed circle constant retain their original map node.

use super::{DescriptorConstant, MapError};
use crate::{Limits, LimitsOverride, UnitError};
use nrz_cas::{
    ExactConstant, Location, Rational,
    limits::{Budget, BudgetKind},
    syntax::Constant,
};

/// Admitted numerical operation on the exported exact rational factor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PiFactor {
    /// The coefficient is exactly the rational factor, with no pi evaluation.
    Rational,
    /// The coefficient is the rational factor multiplied by mathematical pi.
    Multiply,
    /// The coefficient is the rational factor divided by mathematical pi.
    Divide,
}

/// A checked decomposition borrowed from its exact descriptor and graph identity.
///
/// Only the rational spelling is owned. The original descriptor/map owner must
/// outlive this value; exporting does not copy or reidentify its constant graph.
#[derive(Debug)]
pub struct ConstantEnclosureExport<'a> {
    /// Original checked coefficient, including its scale/shift and complete map identity.
    definition: &'a DescriptorConstant,
    /// Bounded exact integer or fraction; no approximate pi spelling is emitted.
    factor: String,
    /// The one admitted operation on the rational factor.
    pi: PiFactor,
    /// Complete exact recognition and spelling Work for this independent request.
    work: u64,
    /// Logical result metadata plus the owned string capacity.
    bytes: usize,
}

impl DescriptorConstant {
    /// Export a checked rational, rational-times-pi or rational-over-pi definition.
    ///
    /// Recognition uses exact CAS normalization, never reporting names or decimal
    /// matching. The result borrows the original definition and reports its Work;
    /// a numerical owner must enclose the indicated operation and retain identity.
    /// # Errors
    /// Unsupported expressions retain `UnsupportedExactConstantExport`. Exact
    /// arithmetic and caller Work/byte limits refuse without publishing a prefix.
    pub fn enclosure_export(
        &self,
        limits: &LimitsOverride,
    ) -> Result<ConstantEnclosureExport<'_>, MapError> {
        let (factor, pi, work, bytes) = export(
            self.exact(),
            size_of::<ConstantEnclosureExport<'_>>(),
            limits,
        )?;
        Ok(ConstantEnclosureExport {
            definition: self,
            factor,
            pi,
            work,
            bytes,
        })
    }
}

/// Borrowed exact coefficient export for live maps and typed structural constants.
///
/// The caller retains the original typed owner and graph position. This export
/// certifies only the admitted exact rational factor and pi operation.
#[derive(Debug)]
pub struct ExactEnclosureExport<'a> {
    /// Original immutable exact coefficient; no graph is copied or reidentified.
    definition: &'a ExactConstant,
    /// Exact integer or fraction, with no approximate spelling of pi.
    factor: String,
    /// Admitted operation on the rational factor.
    pi: PiFactor,
    /// Complete recognition and spelling Work.
    work: u64,
    /// Logical metadata and owned string capacity.
    bytes: usize,
}

impl<'a> ExactEnclosureExport<'a> {
    /// Recognize a rational, rational-times-pi or rational-over-pi constant.
    /// # Errors
    /// Refuses unsupported expressions by name and bounded exact Work or bytes.
    pub fn new(value: &'a ExactConstant, limits: &LimitsOverride) -> Result<Self, MapError> {
        let (factor, pi, work, bytes) = export(value, size_of::<Self>(), limits)?;
        Ok(Self {
            definition: value,
            factor,
            pi,
            work,
            bytes,
        })
    }
    /// Original exact coefficient, including its immutable graph node when irrational.
    #[must_use]
    pub const fn definition(&self) -> &'a ExactConstant {
        self.definition
    }
    /// Exact rational spelling, ready for certified literal import.
    #[must_use]
    pub fn rational_factor(&self) -> &str {
        &self.factor
    }
    /// The admitted pi operation, or the rational-only case.
    #[must_use]
    pub const fn pi_factor(&self) -> PiFactor {
        self.pi
    }
    /// Complete successful exact recognition and spelling Work.
    #[must_use]
    pub const fn work_used(&self) -> u64 {
        self.work
    }
    /// Owned logical metadata and string capacity; exact graph storage is borrowed.
    #[must_use]
    pub const fn owned_bytes(&self) -> usize {
        self.bytes
    }
}

/// One recognition implementation preserves descriptor and live-coefficient owner costs.
fn export(
    value: &ExactConstant,
    metadata: usize,
    limits: &LimitsOverride,
) -> Result<(String, PiFactor, u64, usize), MapError> {
    let mut ledger = Budget::new(Limits::default().resolve(limits)?)?;
    let retained =
        u64::try_from(metadata).map_err(|_| UnitError::UnsupportedExactConstantExport)?;
    let (factor, pi) = ledger.with_retained_payload(
        retained,
        Location::new("constant enclosure export"),
        |ledger| recognize(value, ledger),
    )?;
    let bytes = metadata
        .checked_add(factor.capacity())
        .ok_or(UnitError::UnsupportedExactConstantExport)?;
    let count = u64::try_from(bytes).map_err(|_| UnitError::UnsupportedExactConstantExport)?;
    ledger.check(
        BudgetKind::StagedPayloadBytes,
        count,
        Location::new("constant enclosure export"),
    )?;
    ledger.check(
        BudgetKind::OutputBytes,
        count,
        Location::new("constant enclosure export"),
    )?;
    Ok((factor, pi, ledger.usage().work, bytes))
}

/// Rational inputs never construct or inspect pi; unsupported constants receive no approximation.
fn recognize(value: &ExactConstant, budget: &mut Budget) -> Result<(String, PiFactor), MapError> {
    if let Some(rational) = value.as_rational() {
        return encoded(rational, PiFactor::Rational, budget);
    }
    let circle = ExactConstant::named(Constant::Pi, budget)?;
    let quotient = value.div(&circle, budget)?;
    if let Some(rational) = quotient.as_rational() {
        return encoded(rational, PiFactor::Multiply, budget);
    }
    let product = value.mul(&circle, budget)?;
    if let Some(rational) = product.as_rational() {
        return encoded(rational, PiFactor::Divide, budget);
    }
    Err(UnitError::UnsupportedExactConstantExport.into())
}

/// Spell the recognized rational factor under the same exact ledger.
fn encoded(
    value: &Rational,
    pi: PiFactor,
    budget: &mut Budget,
) -> Result<(String, PiFactor), MapError> {
    Ok((value.exact_string(budget)?, pi))
}

impl<'a> ConstantEnclosureExport<'a> {
    /// The complete original descriptor coefficient and its immutable graph node.
    #[must_use]
    pub const fn definition(&self) -> &'a DescriptorConstant {
        self.definition
    }

    /// Exact rational factor, requiring ordinary certified literal import.
    #[must_use]
    pub fn rational_factor(&self) -> &str {
        &self.factor
    }

    /// Multiplication, reciprocal multiplication, or no pi operation.
    #[must_use]
    pub const fn pi_factor(&self) -> PiFactor {
        self.pi
    }

    /// Exact recognition and spelling Work belonging to this result.
    #[must_use]
    pub const fn work_used(&self) -> u64 {
        self.work
    }

    /// Owned logical result metadata and string capacity; borrowed definitions are separate.
    #[must_use]
    pub const fn owned_bytes(&self) -> usize {
        self.bytes
    }
}
