// SPDX-License-Identifier: AGPL-3.0-only
//! The formal register and both guide sources contain precisely the same named schemas.

use crate::proof::EMPTY_SCHEMAS;

/// Every registered formal name and version has an exact documented statement and premise shape.
/// ´test:crate:emptiness-schema-register-is-documented´
#[test]
fn emptiness_schema_register_is_documented() {
    let actual: Vec<_> = EMPTY_SCHEMAS
        .iter()
        .map(|schema| (schema.name, schema.version))
        .collect();
    assert_eq!(
        actual,
        [
            ("empty.split", 1),
            ("empty.sign", 1),
            ("empty.factor", 1),
            ("empty.lemma", 1),
            ("empty.zero-set", 1),
            ("empty.monomial", 1),
            ("empty.hyp.zero", 1),
            ("empty.hyp.strict", 1),
            ("empty.hyp.nonzero", 1),
            ("empty.hyp.identity", 1),
            ("empty.hyp.cover", 1),
            ("empty.hyp.empty", 1),
        ]
    );
    let expected: Vec<_> = EMPTY_SCHEMAS
        .iter()
        .map(|schema| {
            format!(
                "Formal schema `{}`, version {}. {} Required premises: {}",
                schema.name, schema.version, schema.statement, schema.premises,
            )
        })
        .collect();
    for document in [
        include_str!("../../README.md"),
        include_str!("../../../../templates/crates/cas/README.md.j2"),
    ] {
        let actual: Vec<_> = document
            .lines()
            .filter(|line| line.starts_with("Formal schema `empty."))
            .collect();
        assert_eq!(actual, expected);
        for schema in &EMPTY_SCHEMAS {
            let heads = document
                .lines()
                .filter(|line| {
                    line.starts_with("**") && line.ends_with(&format!("· `{}`", schema.label))
                })
                .count();
            assert_eq!(heads, 1);
        }
    }
}

/// Labels are unique admitted theorem or hypothesis kinds, separate from anonymous instance keys.
/// ´test:crate:emptiness-schema-labels-are-unique´
#[test]
fn emptiness_schema_labels_are_unique() {
    for (index, schema) in EMPTY_SCHEMAS.iter().enumerate() {
        let hypothesis = schema.name.starts_with("empty.hyp.");
        assert_eq!(schema.label.starts_with("assum:cas:"), hypothesis);
        assert_eq!(schema.label.starts_with("thm:cas:"), !hypothesis);
        for earlier in &EMPTY_SCHEMAS[..index] {
            assert_ne!(schema.name, earlier.name);
            assert_ne!(schema.label, earlier.label);
        }
    }
}
