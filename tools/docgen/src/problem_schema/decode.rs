// SPDX-License-Identifier: AGPL-3.0-only
//! Exact tagged alternatives and source-level dependency declarations.

use std::collections::BTreeSet;

use serde_json::Value;

use super::record::Record;
use crate::{envelope, error::Error};

/// The executable status selected by the author, never by a failed check.
#[derive(Clone, Debug)]
pub(super) enum Formula<'a> {
    /// An independent quantity coordinate.
    Input,
    /// A structural or categorical value outside quantity algebra.
    Metadata,
    /// An exact typed constant.
    Constant,
    /// An independently declared input uncertainty magnitude.
    Magnitude,
    /// Original source operations in the bounded algebra grammar.
    Algebra(Record<'a>),
    /// An explicit limitation with dependencies and a precise evidence scope.
    Unsupported(Record<'a>),
}

/// One globally named declaration, retaining its source and relationship record.
#[derive(Clone, Debug)]
pub(super) struct Item<'a> {
    /// Full source record.
    pub(super) record: Record<'a>,
    /// Identifier shared across all declaration categories.
    pub(super) name: String,
    /// Stable inventory category.
    pub(super) category: &'static str,
    /// Input or executable formula contract.
    pub(super) formula: Formula<'a>,
    /// Independent uncertainty relationship when present.
    pub(super) companion: Option<Record<'a>>,
}

/// Decode one expression's exact alternative and its unsupported scope.
pub(super) fn formula<'a>(record: &Record<'a>) -> Result<Formula<'a>, Error> {
    match record.string("kind")? {
        "algebra" => {
            record.keys(&["kind", "text"], &[])?;
            let text = record.at("text")?;
            text.text()?;
            Ok(Formula::Algebra(text))
        }
        "unsupported" => {
            record.keys(
                &["kind", "text", "code", "reason", "dependencies", "scope"],
                &[],
            )?;
            for key in ["text", "code", "reason"] {
                record.string(key)?;
            }
            let mut names = BTreeSet::new();
            for dependency in record.array("dependencies")? {
                if !names.insert(dependency.symbol()?) {
                    return Err(dependency.error("duplicate dependency"));
                }
            }
            let mut scopes = BTreeSet::new();
            for scope in record.array("scope")? {
                let text = scope.text()?;
                if !matches!(text, "expression" | "relationship" | "descriptor")
                    || !scopes.insert(text)
                {
                    return Err(scope.error("invalid or duplicate unsupported scope"));
                }
            }
            if !scopes.contains("expression")
                || (scopes.contains("descriptor") && !scopes.contains("relationship"))
            {
                return Err(record.error(
                    "unsupported scope must include expression and any descriptor's relationship",
                ));
            }
            Ok(Formula::Unsupported(record.clone()))
        }
        _ => Err(record.error("unknown expression alternative")),
    }
}

/// Decode entry shape and gather declarations in source order.
pub(super) fn items<'a>(entry: &Record<'a>) -> Result<Vec<Item<'a>>, Error> {
    entry_shape(entry)?;
    let mut items = Vec::new();
    for parameter in entry.array("parameters")? {
        parameter_items(&parameter, &mut items)?;
    }
    for symbol in entry.optional_array("symbols")? {
        let formula = match symbol.string("kind")? {
            "constant" => {
                symbol.keys(
                    &[
                        "name",
                        "description",
                        "descriptor",
                        "source",
                        "assurance",
                        "kind",
                        "value",
                    ],
                    &[],
                )?;
                symbol.at("value")?.rational()?;
                Formula::Constant
            }
            "derived" => {
                symbol.keys(
                    &[
                        "name",
                        "description",
                        "descriptor",
                        "source",
                        "assurance",
                        "kind",
                        "expression",
                    ],
                    &[],
                )?;
                formula(&symbol.at("expression")?)?
            }
            _ => return Err(symbol.error("unknown symbol alternative")),
        };
        for key in ["description", "source", "assurance"] {
            symbol.string(key)?;
        }
        items.push(item(symbol, "symbol", formula, None)?);
    }
    for observable in entry.array("observables")? {
        observable.keys(
            &[
                "name",
                "description",
                "descriptor",
                "expression",
                "closed_form",
                "value",
                "value_kind",
                "derivation",
                "second_route",
            ],
            &["companion"],
        )?;
        for key in ["description", "closed_form", "derivation", "second_route"] {
            observable.string(key)?;
        }
        exactness(&observable)?;
        let companion = observable.optional("companion");
        if let Some(companion) = &companion {
            relationship(companion, false)?;
        }
        let expression = formula(&observable.at("expression")?)?;
        items.push(item(observable, "observable", expression, companion)?);
    }
    if entry.array("parameters")?.is_empty() || entry.array("observables")?.is_empty() {
        return Err(entry.error("parameters and observables must be nonempty"));
    }
    let mut names = BTreeSet::new();
    for item in &items {
        if !names.insert(item.name.clone()) {
            return Err(item.record.error("duplicate declaration name"));
        }
    }
    Ok(items)
}

/// Validate documentary fields, entry keys and label arrays before declaration decoding.
fn entry_shape(entry: &Record<'_>) -> Result<(), Error> {
    entry.keys(
        &[
            "problem",
            "exponent_policy",
            "kinds",
            "kind_rules",
            "parameters",
            "observables",
            "errors",
            "stress",
        ],
        &["symbols", "equations", "variants"],
    )?;
    let problem = entry.at("problem")?;
    problem.keys(
        &["slug", "title", "register", "kind", "model", "instance"],
        &[],
    )?;
    for key in ["slug", "title", "register", "kind", "model", "instance"] {
        problem.string(key)?;
    }
    if !envelope::valid_label(problem.string("register")?) {
        return Err(problem.error("invalid register label"));
    }
    let errors = entry.at("errors")?;
    errors.keys(&["bounded", "estimated", "excluded"], &[])?;
    for key in ["bounded", "estimated", "excluded"] {
        for text in errors.array(key)? {
            text.text()?;
        }
    }
    let stress = entry.at("stress")?;
    stress.keys(&["claims", "bites"], &[])?;
    for key in ["claims", "bites"] {
        for label in stress.array(key)? {
            let text = label.text()?;
            if !envelope::valid_label(text) || (key == "bites" && !text.starts_with("step:nrz:")) {
                return Err(label.error("invalid stress label"));
            }
        }
    }
    Ok(())
}

/// Create a located named item with no inferred numerical meaning.
fn item<'a>(
    mut record: Record<'a>,
    category: &'static str,
    formula: Formula<'a>,
    companion: Option<Record<'a>>,
) -> Result<Item<'a>, Error> {
    let name = record.at("name")?.symbol()?;
    record.path = format!("{}({})", record.path, name.as_str());
    Ok(Item {
        record,
        name,
        category,
        formula,
        companion,
    })
}

/// Decode a parameter and all independently declared uncertainty components.
fn parameter_items<'a>(record: &Record<'a>, items: &mut Vec<Item<'a>>) -> Result<(), Error> {
    record.string("description")?;
    let role = record.string("role")?;
    let parameter_formula = match role {
        "metadata" => {
            record.keys(
                &[
                    "name",
                    "description",
                    "value",
                    "role",
                    "metadata",
                    "uncertainty",
                ],
                &[],
            )?;
            metadata(&record.at("metadata")?, &record.at("value")?)?;
            Formula::Metadata
        }
        "quantity" | "count" => {
            record.keys(
                &[
                    "name",
                    "description",
                    "value",
                    "role",
                    "descriptor",
                    "nonzero",
                    "uncertainty",
                ],
                &[],
            )?;
            let value = record.at("value")?.rational()?;
            if record.at("nonzero")?.boolean()? && value.is_zero() {
                return Err(record
                    .at("value")?
                    .error("value contradicts nonzero assumption"));
            }
            Formula::Input
        }
        _ => return Err(record.error("unknown parameter role")),
    };
    items.push(item(record.clone(), "parameter", parameter_formula, None)?);
    let uncertainty = record.at("uncertainty")?;
    match uncertainty.string("kind")? {
        "none" => uncertainty.keys(&["kind"], &[])?,
        "components" => {
            uncertainty.keys(&["kind", "components"], &[])?;
            let components = uncertainty.array("components")?;
            if role == "metadata" || components.is_empty() {
                return Err(
                    uncertainty.error("components require a quantity or count and a nonempty list")
                );
            }
            for component in components {
                relationship(&component, true)?;
                if component.string("target")? != record.string("name")? {
                    return Err(component.error("input component target differs from its owner"));
                }
                let formula = match (
                    component.optional("magnitude"),
                    component.optional("expression"),
                ) {
                    (Some(magnitude), None) => {
                        magnitude_value(&component, &magnitude)?;
                        Formula::Magnitude
                    }
                    (None, Some(expression)) => formula(&expression)?,
                    _ => {
                        return Err(
                            component.error("exactly one of magnitude and expression is required")
                        );
                    }
                };
                items.push(item(
                    component.clone(),
                    "uncertainty",
                    formula,
                    Some(component),
                )?);
            }
        }
        _ => return Err(uncertainty.error("unknown uncertainty alternative")),
    }
    Ok(())
}

/// Validate the exact mode-dependent relationship fields without assuming independence.
pub(super) fn relationship(record: &Record<'_>, component: bool) -> Result<(), Error> {
    if component {
        record.keys(
            &[
                "name",
                "class",
                "mode",
                "target",
                "descriptor",
                "source",
                "assurance",
            ],
            &["other", "reference", "magnitude", "expression"],
        )?;
    } else {
        record.keys(
            &["class", "mode", "target", "source", "assurance"],
            &["other", "reference"],
        )?;
    }
    if !matches!(record.string("class")?, "measurement" | "computational") {
        return Err(record.error("unknown uncertainty class"));
    }
    for key in ["source", "assurance"] {
        record.string(key)?;
    }
    record.at("target")?.symbol()?;
    let mode = record.string("mode")?;
    super::kinds::mode(&record.at("mode")?)?;
    if (mode == "covariance") != record.optional("other").is_some()
        || matches!(mode, "relative-radius" | "relative-standard")
            != record.optional("reference").is_some()
    {
        return Err(record.error("mode requires exactly its other/reference fields"));
    }
    for key in ["other", "reference"] {
        if let Some(value) = record.optional(key) {
            value.symbol()?;
        }
    }
    Ok(())
}

/// Admit a magnitude in its own continuous uncertainty domain.
fn magnitude_value(component: &Record<'_>, magnitude: &Record<'_>) -> Result<(), Error> {
    let key = match component.string("mode")? {
        "radius" => "half_width",
        "standard" => "sigma",
        "relative-radius" => "relative_radius",
        "relative-standard" => "relative_standard",
        "variance" => "variance",
        "covariance" => "covariance",
        _ => return Err(component.error("unknown uncertainty mode")),
    };
    magnitude.keys(&[key], &[])?;
    if magnitude.at(key)?.rational()?.is_negative() && key != "covariance" {
        return Err(magnitude.error("uncertainty magnitude must be nonnegative"));
    }
    Ok(())
}

/// Admit metadata categories and integer ranges separately from physical quantities.
pub(super) fn metadata(record: &Record<'_>, value: &Record<'_>) -> Result<(), Error> {
    match record.string("kind")? {
        "category" => {
            record.keys(&["kind", "values"], &[])?;
            let mut permitted = BTreeSet::new();
            for choice in record.array("values")? {
                if !permitted.insert(choice.text()?) {
                    return Err(choice.error("duplicate metadata category"));
                }
            }
            if !permitted.contains(value.text()?) {
                return Err(value.error("value is outside metadata categories"));
            }
        }
        "integer_range" => {
            record.keys(&["kind", "minimum"], &["maximum"])?;
            let minimum = record.at("minimum")?.integer()?;
            let maximum = record.bound("maximum")?;
            let number = value.rational()?;
            if nrz_units::CountDomain::new(Some(minimum), maximum)
                .and_then(|domain| domain.check(&number))
                .is_err()
            {
                return Err(value.error("value is outside metadata integer range"));
            }
        }
        _ => return Err(record.error("unknown metadata alternative")),
    }
    Ok(())
}

/// Preserve existing expected-value precision rules while admitting exact numeric domains.
fn exactness(record: &Record<'_>) -> Result<(), Error> {
    let value = record.at("value")?;
    value.rational()?;
    let text = value.text()?;
    let digits = match record.string("value_kind")? {
        "exact" => {
            if text.contains(['e', 'E']) {
                return Err(value.error("exact value uses scientific notation"));
            }
            return Ok(());
        }
        "decimal-12" => 12,
        "decimal-4" => 4,
        _ => return Err(record.error("unknown value_kind")),
    };
    let significant = text
        .split(['e', 'E'])
        .next()
        .unwrap_or("")
        .bytes()
        .filter(u8::is_ascii_digit)
        .skip_while(|byte| *byte == b'0')
        .count();
    if significant != digits {
        return Err(value.error("wrong significant-digit count"));
    }
    Ok(())
}

/// Inspect a deliberately unsupported evidence scope.
pub(super) fn unsupported_scope(formula: &Formula<'_>, scope: &str) -> bool {
    if let Formula::Unsupported(record) = formula {
        record
            .value
            .get("scope")
            .and_then(Value::as_array)
            .is_some_and(|values| values.iter().any(|value| value.as_str() == Some(scope)))
    } else {
        false
    }
}
