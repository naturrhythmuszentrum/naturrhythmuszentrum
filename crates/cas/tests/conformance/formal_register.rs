// SPDX-License-Identifier: AGPL-3.0-only
//! Public formal metadata is complete, unique and pinned to both guide sources.

use nrz_cas::proof::{PRIMITIVE_SCHEMAS, PRIMITIVES, PrimitiveSchema};

/// Every compiled primitive has exactly one named contract with unchanged typed metadata.
/// ´test:integration:formal-primitive-register-is-complete´
#[test]
fn formal_primitive_register_is_complete() {
    assert_eq!(PRIMITIVES.len(), PRIMITIVE_SCHEMAS.len());
    for (index, primitive) in PRIMITIVES.iter().enumerate() {
        let schema = primitive.formal_schema();
        let rule = primitive.specification();
        assert_eq!(*schema, PRIMITIVE_SCHEMAS[index]);
        assert_eq!(schema.lemma.name, rule.key);
        assert_eq!(schema.lemma.version, rule.version);
        assert_eq!(schema.premises, rule.premises);
        assert_eq!(schema.lemma.label.starts_with("thm:cas:"), rule.available);
        assert_eq!(
            schema.lemma.label.starts_with("assum:cas:"),
            !rule.available
        );
        for earlier in &PRIMITIVE_SCHEMAS[..index] {
            assert_ne!(schema.lemma.name, earlier.lemma.name);
            assert_ne!(schema.lemma.label, earlier.lemma.label);
        }
    }
}

/// Render the exact public mathematical contract independently of the guide text.
fn paragraph(schema: &PrimitiveSchema) -> String {
    let lemma = schema.lemma;
    format!(
        "Formal primitive `{}`, version {}. {} Required premises: {} Standing: {}",
        lemma.name,
        lemma.version,
        lemma.statement,
        lemma.premises,
        lemma.standing.description(),
    )
}

/// Every formal head and its attached statement have one register entry, in both directions.
/// ´test:integration:formal-primitive-register-is-documented´
#[test]
fn formal_primitive_register_is_documented() {
    let expected: Vec<_> = PRIMITIVE_SCHEMAS.iter().map(paragraph).collect();
    for document in [
        include_str!("../../README.md"),
        include_str!("../../../../templates/crates/cas/README.md.j2"),
    ] {
        let actual: Vec<_> = document
            .lines()
            .filter(|line| line.starts_with("Formal primitive `"))
            .collect();
        assert_eq!(actual, expected);
        let heads: Vec<_> = document
            .lines()
            .filter(|line| line.starts_with("**") && line.contains(":cas:primitive-"))
            .collect();
        assert_eq!(heads.len(), PRIMITIVE_SCHEMAS.len());
        for (schema, head) in PRIMITIVE_SCHEMAS.iter().zip(heads) {
            let title = if schema.lemma.label.starts_with("thm:") {
                "Theorem"
            } else {
                "Assumption"
            };
            assert_eq!(
                head,
                format!(
                    "**{title} (Primitive {})** · `{}`",
                    schema.lemma.name, schema.lemma.label
                )
            );
            assert_eq!(
                document
                    .matches(&format!("{head}\n\n{}", paragraph(schema)))
                    .count(),
                1
            );
        }
    }
}
