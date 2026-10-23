// SPDX-License-Identifier: AGPL-3.0-only
//! Source contracts extend canonical expression identity without a second graph encoding.

use super::{AT, SourceCoordinates, invalid, storage};
use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Handle, size};

/// Immutable local request with portable original-expression and ordered-source identity.
///
/// Keys from independently reconstructed stores compare by canonical bytes.
/// They cannot authorize foreign handles; build a local key before deriving there.
#[derive(Clone, Debug)]
pub struct SourceJacobianKey {
    /// Complete mathematical context in the originating graph.
    pub(super) context: Handle,
    /// Exact original, including every operand beneath zero and cancellation.
    pub(super) original: Handle,
    /// Explicit source and fixed declaration keys.
    pub(super) coordinates: SourceCoordinates,
    /// Canonical expression records followed by existing count/name encodings.
    pub(super) bytes: Vec<u8>,
}

impl SourceJacobianKey {
    /// Original expression whose domains and syntax participate in the key.
    #[must_use]
    pub const fn original(&self) -> &Handle {
        &self.original
    }

    /// Ordered source and fixed declarations.
    #[must_use]
    pub const fn coordinates(&self) -> &SourceCoordinates {
        &self.coordinates
    }

    /// Collision-free portable bytes; numerical operating points are absent.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }

    /// Bound the simultaneous key copy, owned names and result metadata.
    pub(super) fn payload(&self) -> u64 {
        u64::try_from(self.bytes.len())
            .unwrap_or(u64::MAX)
            .saturating_mul(4)
            .saturating_add(512)
    }
}

impl PartialEq for SourceJacobianKey {
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes
    }
}

impl Eq for SourceJacobianKey {}

impl Cas {
    /// Key a source Jacobian before deriving any partials.
    ///
    /// The canonical expression key retains declarations, guards, strata,
    /// denominators, original representation and compiled proof revisions. The
    /// coordinate contract adds ordered sources and explicitly fixed declarations.
    /// # Errors
    /// Refuses fixed sources, duplicates, undeclared/non-Real coordinates, foreign
    /// handles, refuted original domains and exhausted cumulative resources.
    pub fn source_jacobian_key(
        &mut self,
        context: &Handle,
        original: &Handle,
        coordinates: &SourceCoordinates,
        budget: &mut Budget,
    ) -> Result<SourceJacobianKey, Fault> {
        self.proof_limits(budget, |cas, budget| {
            cas.transaction(budget, |cas, budget| {
                cas.check_source_coordinates(context, coordinates, budget)?;
                let expression = cas.expression_reuse_key(context, original, budget)?;
                let retained = size(expression.as_bytes().len(), AT)?
                    .saturating_mul(4)
                    .saturating_add(512);
                storage::retaining(retained, budget, |budget| {
                    let mut out = Encoder::new(budget, AT);
                    out.raw(b"NRZSRC04")?;
                    out.count(expression.as_bytes().len())?;
                    out.raw(expression.as_bytes())?;
                    names(&coordinates.sources, &mut out)?;
                    names(&coordinates.fixed, &mut out)?;
                    let bytes = out.finish();
                    budget.check(BudgetKind::OutputBytes, size(bytes.len(), AT)?, AT)?;
                    budget.work(size(bytes.len(), AT)?, AT)?;
                    Ok(SourceJacobianKey {
                        context: context.clone(),
                        original: original.clone(),
                        coordinates: coordinates.clone(),
                        bytes,
                    })
                })
            })
        })
    }

    /// Fixed declarations never add a fictitious differentiation direction.
    fn check_source_coordinates(
        &self,
        context: &Handle,
        coordinates: &SourceCoordinates,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let home = self.checked(context)?;
        let width = size(self.state.context(home)?.header.len(), AT)?.saturating_add(1);
        let count = coordinates
            .sources
            .len()
            .saturating_add(coordinates.fixed.len());
        let count64 = size(count, AT)?;
        budget.check(BudgetKind::FamilyParameters, count64, AT)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            count64.saturating_mul(width).saturating_mul(2),
            AT,
        )?;
        budget.work(
            count64
                .saturating_mul(count64.saturating_add(1))
                .saturating_mul(width),
            AT,
        )?;
        for values in [&coordinates.sources, &coordinates.fixed] {
            for (position, key) in values.iter().enumerate() {
                crate::derivative_check::variable(&self.state, home, key, budget)?;
                if values
                    .get(..position)
                    .is_some_and(|previous| previous.contains(key))
                {
                    return Err(invalid(InvalidCode::JacobianDuplicateSource));
                }
            }
        }
        for key in &coordinates.sources {
            if coordinates.fixed.contains(key) {
                return Err(invalid(InvalidCode::JacobianFixedSource));
            }
        }
        Ok(())
    }
}

/// Existing wire primitives delimit every name and list; concatenation is unambiguous.
fn names(values: &[String], out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.count(values.len())?;
    for value in values {
        out.name(value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::names;
    use crate::budget::{Budget, BudgetKind, Limits};
    use crate::encoding::Encoder;
    use crate::error::Location;
    use crate::wire_read::Reader;

    /// Ordered source and fixed lists use matching four-byte counts and UTF-8 name lengths.
    /// ´test:unit:source-coordinate-key-fields-round-trip´
    #[test]
    fn source_coordinate_key_fields_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let sources = vec!["coordinate".repeat(25), "λ".into()];
        let fixed = vec!["reference".into()];
        let mut budget = Budget::new(Limits::default())?;
        let mut out = Encoder::new(&mut budget, Location::new("coordinate round trip"));
        names(&sources, &mut out)?;
        names(&fixed, &mut out)?;
        let bytes = out.finish();
        let mut reader = Reader::new(&bytes, &mut budget)?;
        for expected in [&sources, &fixed] {
            let count = reader.count(BudgetKind::FamilyParameters, 4, 32)?;
            let mut actual = Vec::new();
            for _ in 0..count {
                actual.push(reader.name()?);
            }
            assert_eq!(&actual, expected);
        }
        reader.finished()?;
        Ok(())
    }
}
