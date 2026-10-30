// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded certificate bytes contain no runtime store identity.

use super::{AT, EnclosureCertificate, Interval};
use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::error::{Fault, InvalidCode};
use crate::wire_read::Reader;

impl EnclosureCertificate {
    /// Encode the sole implemented schema and its complete original judgment.
    /// # Errors
    /// Refuses unknown versions or exhausted witness, component and Work limits.
    pub fn to_bytes(&self, budget: &mut Budget) -> Result<Vec<u8>, Fault> {
        if self.version != 1 {
            return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
        }
        budget.check(
            BudgetKind::ProofNodes,
            crate::graph::size(self.records.len(), AT)?,
            AT,
        )?;
        let mut out = Encoder::witness(budget, AT);
        out.raw(b"NRZENCW1")?;
        out.name(self.law.formal_schema().name)?;
        out.unsigned(u64::from(self.version))?;
        out.count(self.scope.len())?;
        out.raw(&self.scope)?;
        out.unsigned(u64::from(self.grid))?;
        out.unsigned(u64::from(self.presentation))?;
        out.count(self.records.len())?;
        for record in &self.records {
            out.number(record.lower())?;
            out.number(record.upper())?;
        }
        if self.law == super::EnclosureLaw::LogAtanh {
            super::log::encode(&self.logarithms, &mut out)?;
        } else if !self.logarithms.is_empty() {
            return Err(Fault::invalid(InvalidCode::Encoding, AT));
        }
        Ok(out.finish())
    }

    /// Decode a proposed certificate with bounded allocation and canonical rational checks.
    /// The result remains untrusted until `Cas::check_enclosure` checks every relation.
    /// # Errors
    /// Refuses malformed framing, unknown versions, noncanonical numbers or exhausted limits.
    pub fn from_bytes(bytes: &[u8], budget: &mut Budget) -> Result<Self, Fault> {
        let mut reader = Reader::new(bytes, budget)?;
        if reader.take(8)? != b"NRZENCW1" {
            return Err(Fault::invalid(InvalidCode::Encoding, AT));
        }
        let law = match reader.name()?.as_str() {
            "enclosure.rational-inclusion" => super::EnclosureLaw::RationalInclusion,
            "enclosure.root-grid" => super::EnclosureLaw::RootGrid,
            "enclosure.log-atanh" => super::EnclosureLaw::LogAtanh,
            _ => return Err(Fault::invalid(InvalidCode::Encoding, AT)),
        };
        let version = reader.small()?;
        if version != 1 {
            return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
        }
        let count = reader.count(BudgetKind::WitnessBytes, 1, 1)?;
        let scope = reader.take(count)?.to_vec();
        let grid = reader.small()?;
        let presentation = reader.small()?;
        let count = reader.count(BudgetKind::ProofNodes, 18, 256)?;
        let mut records = Vec::new();
        records
            .try_reserve(count)
            .map_err(|_| Fault::allocation(AT))?;
        for _ in 0..count {
            let lower = reader.rational()?;
            let upper = reader.rational()?;
            records.push(Interval::new(lower, upper, reader.budget)?);
        }
        let logarithms = if law == super::EnclosureLaw::LogAtanh {
            super::log::decode(&mut reader)?
        } else {
            Vec::new()
        };
        reader.finished()?;
        Ok(Self {
            law,
            version,
            scope,
            grid,
            presentation,
            records,
            logarithms,
        })
    }
}
