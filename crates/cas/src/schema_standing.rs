// SPDX-License-Identifier: AGPL-3.0-only
//! Documentary proof provenance and assumed standing never confer checker authority.

/// How a theorem instance is established, separate from the statement it proves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LemmaProvenance {
    /// An instance is established by the CAS's closed independent checker.
    CasChecked,
    /// A cited external proof certificate, with its exact provenance retained as metadata.
    ///
    /// No entry currently uses this form and it enables no certificate reader or trusted callback.
    ExternalCertificate {
        /// Exact public certificate or publication identity, including its revision or stated bound.
        citation: &'static str,
        /// Origin and verification method of that certificate.
        provenance: &'static str,
    },
}

/// Whether a formal register entry describes a proved inference or an assumed shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Standing {
    /// Instances require the stated proof provenance and retain every used condition.
    Lemma(LemmaProvenance),
    /// This shape is assumed until a checked instance discharges its use.
    Hypothesis,
}

impl Standing {
    /// The exact standing text pinned alongside each formal statement in the guide.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Lemma(LemmaProvenance::CasChecked) => "Lemma; instances are CAS-checked.",
            Self::Lemma(LemmaProvenance::ExternalCertificate { .. }) => {
                "Lemma; cited external certificate with retained provenance."
            }
            Self::Hypothesis => "Hypothesis; every undischarged use remains conditional.",
        }
    }
}
