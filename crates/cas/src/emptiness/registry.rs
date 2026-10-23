// SPDX-License-Identifier: AGPL-3.0-only
//! Formal schema identities are distinct from anonymous, fully scoped theorem instances.

/// A named formal schema; an application keeps its own complete statement identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LemmaSchema {
    /// Formal schema name, independent of display text and local proof handles.
    pub name: &'static str,
    /// Estate label of the theorem or hypothesis kind in the CAS guide.
    pub label: &'static str,
    /// Semantic revision of this schema's stated meaning and premises.
    pub version: u32,
    /// Proved inference with its provenance, or an assumed hypothesis shape.
    pub standing: crate::proof::Standing,
    /// Mathematical meaning of the formal schema.
    pub statement: &'static str,
    /// Required checked premise shapes, including exact regional qualification.
    pub premises: &'static str,
}

/// Closed register of the touched emptiness lemmas and their formal hypothesis kinds.
pub const EMPTY_SCHEMAS: [LemmaSchema; 12] = [
    LemmaSchema {
        name: "empty.split",
        label: "thm:cas:empty-split",
        version: 1,
        standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
        statement: "A complete exact cover of empty members has an empty parent.",
        premises: "Checked cover and one earlier empty node for every member.",
    },
    LemmaSchema {
        name: "empty.sign",
        label: "thm:cas:empty-sign",
        version: 1,
        standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
        statement: "A true zero guard contradicts a strict sign or nonzero fact for the identical subject.",
        premises: "True zero guard and a checked containing strict fact.",
    },
    LemmaSchema {
        name: "empty.factor",
        label: "thm:cas:empty-factor",
        version: 1,
        standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
        statement: "A defined complete product of nonzero factors cannot equal zero.",
        premises: "Defined full factor identity, nonzero scalar and all positive-multiplicity factors.",
    },
    LemmaSchema {
        name: "empty.lemma",
        label: "thm:cas:empty-lemma",
        version: 1,
        standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
        statement: "An earlier empty-region theorem proves the identical region empty.",
        premises: "Checked empty theorem in the identical context and premise frame.",
    },
    LemmaSchema {
        name: "empty.zero-set",
        label: "thm:cas:empty-zero-set",
        version: 1,
        standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
        statement: "A defined scalar whose exact zero set is empty is nonzero on its original base.",
        premises: "Checked empty zero set and independently checked subject definedness.",
    },
    LemmaSchema {
        name: "empty.monomial",
        label: "thm:cas:empty-monomial",
        version: 1,
        standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
        statement: "A nonzero monomial supplies nonzero factors for another exact monomial.",
        premises: "Defined nonzero monomials in one formal frame and a nonzero ground coefficient.",
    },
    LemmaSchema {
        name: "empty.hyp.zero",
        label: "assum:cas:empty-zero",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The identical subject equals literal zero on a true ancestor guard.",
        premises: "Qualified region ancestry and exact subject syntax.",
    },
    LemmaSchema {
        name: "empty.hyp.strict",
        label: "assum:cas:empty-strict",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "A strictly positive or negative Real subject is defined and nonzero.",
        premises: "Checked strict sign or literal true strict guard on a containing region.",
    },
    LemmaSchema {
        name: "empty.hyp.nonzero",
        label: "assum:cas:empty-nonzero",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The identical subject is defined and nonzero on a containing region.",
        premises: "Checked nonzero premise or literal true nonzero guard.",
    },
    LemmaSchema {
        name: "empty.hyp.identity",
        label: "assum:cas:empty-identity",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The exact subject and displayed product are both defined and equal.",
        premises: "Checked defined equality in the identical context on a containing region.",
    },
    LemmaSchema {
        name: "empty.hyp.cover",
        label: "assum:cas:empty-cover",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The exact parent is covered by every retained member.",
        premises: "Independently checked cover with no missing or duplicate member.",
    },
    LemmaSchema {
        name: "empty.hyp.empty",
        label: "assum:cas:empty-empty",
        version: 1,
        standing: crate::proof::Standing::Hypothesis,
        statement: "The exact region has no assignments under the retained conditions.",
        premises: "Earlier checked empty-region theorem in the identical context.",
    },
];
