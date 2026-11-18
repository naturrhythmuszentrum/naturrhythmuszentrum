// SPDX-License-Identifier: AGPL-3.0-only
//! Borrowed mapped-copy provenance for the private LC bridge, without retaining edges.
use super::{Declaration, Identity, LoanError, Origin, QualifiedMaskVersion};

/// The original source and projection, borrowed from authenticated application evidence.
#[repr(C)]
pub(in crate::loan) struct LcMappedSource<'a> {
    /// Complete original source declaration, including private symbols.
    pub(in crate::loan) declaration: &'a Declaration,
    /// Original host occupation captured by the mapped copy.
    pub(in crate::loan) identity: Identity,
    /// Three qualified source companions at mapping admission.
    pub(in crate::loan) attachments: &'a [Option<QualifiedMaskVersion>; 3],
    /// Exact original-instance projection of the combined tuple.
    pub(in crate::loan) projection: crate::CellProduct,
}

impl Origin {
    /// Read only the closed mapped-copy source after current native authentication.
    pub(in crate::loan) fn lc_source(&self) -> Result<LcMappedSource<'_>, LoanError> {
        let evidence = self
            .evidence
            .as_ref()
            .ok_or(LoanError::UnsupportedContract)?;
        let crate::kernels::contract::Recipe::Transform(map) = &evidence.law.recipe else {
            return Err(LoanError::UnsupportedContract);
        };
        let Some((projection, crate::CellFactor::Outer)) = map.product() else {
            return Err(LoanError::LayoutMismatch);
        };
        Ok(LcMappedSource {
            declaration: &evidence.source,
            identity: evidence.source_identity,
            attachments: &evidence.attachments,
            projection,
        })
    }
}
