// SPDX-License-Identifier: AGPL-3.0-only
//! Version two problem declarations checked by the authoritative units context.

mod decode;
mod kinds;
mod record;

use std::collections::{BTreeMap, BTreeSet};

use nrz_algebra::{Limits, Rational, Symbol, parse};
use nrz_units::{
    CheckedExpression, Companion, CompanionEvidence, Context, Declaration, Descriptor, Evidence,
    Kind, Observable, Role, uncertainty,
};
use serde_json::{Value, json};

use crate::error::Error;
use decode::{Formula, Item};
use kinds::Kinds;
use record::Record;

/// Validate one source payload and attach deterministically regenerated evidence.
pub(crate) fn validate(
    value: &Value,
    path: &str,
    slug: &str,
    kind: &str,
    label: &str,
) -> Result<Value, Error> {
    let entry = Record::new(value, path);
    let items = decode::items(&entry)?;
    let problem = entry.at("problem")?;
    if problem.string("slug")? != slug
        || problem.string("kind")? != kind
        || problem.string("register")? != label
    {
        return Err(problem.error("slug, kind or register differs from its location or envelope"));
    }
    let (registry, kinds) = kinds::registry(&entry)?;
    let by_name: BTreeMap<_, _> = items.iter().map(|item| (item.name.clone(), item)).collect();
    let mut descriptors = BTreeMap::new();
    for item in &items {
        resolve_descriptor(
            item,
            &by_name,
            &kinds,
            &mut descriptors,
            &mut BTreeSet::new(),
        )?;
    }
    dependencies(&by_name)?;
    let mut context = Context::new(registry, Limits::default());
    for item in &items {
        if let Some(declaration) = declaration(item, &descriptors)? {
            context
                .declare(item.name.clone(), declaration)
                .map_err(|error| item.record.error(error))?;
        }
    }
    let mut records = Vec::new();
    for item in &items {
        records.push(check_item(item, &context, &descriptors, &kinds)?);
    }
    // Also probe unused declarations through the authority's complete inventory API.
    context.check_all().map_err(|error| entry.error(error))?;
    records.extend(equations(&entry, &context, &by_name, &kinds)?);
    variants(&entry, &by_name, &descriptors)?;
    let checked = items
        .iter()
        .filter(|item| item.category == "observable" && matches!(item.formula, Formula::Algebra(_)))
        .count();
    let unsupported = items
        .iter()
        .filter(|item| {
            item.category == "observable" && matches!(item.formula, Formula::Unsupported(_))
        })
        .count();
    let mut result = value.clone();
    if let Some(variants) = result.get_mut("variants").and_then(Value::as_array_mut) {
        for variant in variants {
            if variant.get("overrides").is_none() {
                variant["overrides"] = json!({});
            }
        }
    }
    for key in ["symbols", "equations", "variants"] {
        if result.get(key).is_none() {
            result[key] = json!([]);
        }
    }
    let checked_expressions = records
        .iter()
        .filter(|record| record["expression_status"] == "checked")
        .count();
    let unsupported_expressions = records
        .iter()
        .filter(|record| record["expression_status"] == "unsupported")
        .count();
    result["validation"] = json!({
        "records": records,
        "checked_observables": checked,
        "unsupported_observables": unsupported,
        "checked_expressions": checked_expressions,
        "unsupported_expressions": unsupported_expressions,
    });
    Ok(result)
}

/// Recompute all evidence rather than trusting a committed checked flag.
pub(crate) fn revalidate(value: &Value, path: &str, slug: &str, kind: &str) -> Result<(), Error> {
    let mut source = value.clone();
    let object = source
        .as_object_mut()
        .ok_or_else(|| Record::new(value, path).error("expected entry object"))?;
    let stored = object
        .remove("validation")
        .ok_or_else(|| Record::new(value, path).error("missing validation evidence"))?;
    let entry = Record::new(&source, path);
    let label = entry.at("problem")?.string("register")?;
    let regenerated = validate(&source, path, slug, kind, label)?;
    if regenerated.get("validation") != Some(&stored) {
        return Err(entry.error("validation evidence is stale or forged; regenerate data"));
    }
    Ok(())
}

/// All explicitly named edges, including unsupported and relationship dependencies.
fn edges(item: &Item<'_>) -> Result<BTreeSet<Symbol>, Error> {
    let mut names = match &item.formula {
        Formula::Algebra(expression) => expression.expression()?.symbols(),
        Formula::Unsupported(record) => record
            .array("dependencies")?
            .iter()
            .map(Record::symbol)
            .collect::<Result<_, _>>()?,
        _ => BTreeSet::new(),
    };
    if let Some(companion) = &item.companion {
        for key in ["target", "other", "reference"] {
            if let Some(record) = companion.optional(key) {
                names.insert(record.symbol()?);
            }
        }
    }
    Ok(names)
}

/// Bound all declaration cycles before descriptor or expression resolution.
fn dependencies(items: &BTreeMap<Symbol, &Item<'_>>) -> Result<(), Error> {
    let mut complete = BTreeSet::new();
    for item in items.values() {
        visit(item, items, &mut BTreeSet::new(), &mut complete)?;
    }
    Ok(())
}

/// Walk named dependencies without allowing unsupported values to become substitute inputs.
fn visit(
    item: &Item<'_>,
    items: &BTreeMap<Symbol, &Item<'_>>,
    active: &mut BTreeSet<Symbol>,
    complete: &mut BTreeSet<Symbol>,
) -> Result<(), Error> {
    if complete.contains(&item.name) {
        return Ok(());
    }
    if active.len() >= 128 || !active.insert(item.name.clone()) {
        return Err(item.record.error("declaration cycle or depth limit"));
    }
    for name in edges(item)? {
        let dependency = items.get(&name).ok_or_else(|| {
            item.record
                .error(format!("unknown symbol {}", name.as_str()))
        })?;
        if !matches!(item.formula, Formula::Unsupported(_))
            && matches!(dependency.formula, Formula::Unsupported(_))
        {
            return Err(item.record.error(format!(
                "supported declaration depends on unsupported {}",
                name.as_str()
            )));
        }
        visit(dependency, items, active, complete)?;
    }
    active.remove(&item.name);
    complete.insert(item.name.clone());
    Ok(())
}

/// Construct a companion's identity contract from its declared target names.
fn companion(record: &Record<'_>) -> Result<Companion, Error> {
    Ok(Companion {
        mode: kinds::mode(&record.at("mode")?)?,
        value: record.at("target")?.symbol()?,
        other: record
            .optional("other")
            .map(|value| value.symbol())
            .transpose()?,
        reference: record
            .optional("reference")
            .map(|value| value.symbol())
            .transpose()?,
    })
}

/// Find a relationship target without conflating metadata with a numerical value.
fn target<'a, 'data>(
    record: &Record<'_>,
    key: &str,
    items: &BTreeMap<Symbol, &'a Item<'data>>,
) -> Result<Option<&'a Item<'data>>, Error> {
    record
        .optional(key)
        .map(|name| {
            items
                .get(&name.symbol()?)
                .copied()
                .ok_or_else(|| name.error("unknown relationship target"))
        })
        .transpose()
}

/// Resolve descriptors independently of expression success, including unused declarations.
fn resolve_descriptor(
    item: &Item<'_>,
    items: &BTreeMap<Symbol, &Item<'_>>,
    kinds: &Kinds,
    resolved: &mut BTreeMap<Symbol, Descriptor>,
    active: &mut BTreeSet<Symbol>,
) -> Result<Option<Descriptor>, Error> {
    if let Some(value) = resolved.get(&item.name) {
        return Ok(Some(value.clone()));
    }
    if matches!(item.formula, Formula::Metadata) {
        return Ok(None);
    }
    if active.len() >= 128 || !active.insert(item.name.clone()) {
        return Err(item
            .record
            .error("descriptor relationship cycle or depth limit"));
    }
    let record = item.record.at("descriptor")?;
    let descriptor = if let Some(relationship) = &item.companion {
        companion_descriptor(item, relationship, items, kinds, resolved, active)?
    } else {
        if decode::unsupported_scope(&item.formula, "descriptor")
            || decode::unsupported_scope(&item.formula, "relationship")
        {
            return Err(record.error(
                "unsupported descriptor/relationship scope requires an uncertainty companion",
            ));
        }
        let descriptor = kinds::descriptor(&record, kinds)?;
        if matches!(descriptor.kind(), Kind::Uncertainty(..)) {
            return Err(
                record.error("uncertainty descriptors require an explicit companion relationship")
            );
        }
        Some(descriptor)
    };
    active.remove(&item.name);
    if let Some(descriptor) = &descriptor {
        if item.category == "parameter" {
            let count = item.record.string("role")? == "count";
            if count != descriptor.kind().count_domain().is_some() {
                return Err(record
                    .error("physical count and quantity roles must match their semantic kind"));
            }
        }
        if descriptor.kind().count_domain().is_some() {
            if let Some(value) = item.record.optional("value") {
                descriptor
                    .check_count_literal(value.rational()?)
                    .map_err(|error| value.error(error))?;
            }
            if item.category == "observable" && item.record.string("value_kind")? != "exact" {
                return Err(record.error("count observables require exact values"));
            }
        }
        resolved.insert(item.name.clone(), descriptor.clone());
    }
    Ok(descriptor)
}

/// Resolve one numerical target's independently checked descriptor.
fn target_descriptor(
    item: &Item<'_>,
    items: &BTreeMap<Symbol, &Item<'_>>,
    kinds: &Kinds,
    resolved: &mut BTreeMap<Symbol, Descriptor>,
    active: &mut BTreeSet<Symbol>,
) -> Result<Descriptor, Error> {
    resolve_descriptor(item, items, kinds, resolved, active)?.ok_or_else(|| {
        item.record
            .error("relationship target has no admitted numerical descriptor")
    })
}

/// Derive each uncertainty's scale and kind through its actual target relationship.
fn companion_descriptor(
    item: &Item<'_>,
    relationship: &Record<'_>,
    items: &BTreeMap<Symbol, &Item<'_>>,
    kinds: &Kinds,
    resolved: &mut BTreeMap<Symbol, Descriptor>,
    active: &mut BTreeSet<Symbol>,
) -> Result<Option<Descriptor>, Error> {
    let primary = target(relationship, "target", items)?
        .ok_or_else(|| relationship.error("missing target"))?;
    let primary = target_descriptor(primary, items, kinds, resolved, active)?;
    let other = target(relationship, "other", items)?
        .map(|value| target_descriptor(value, items, kinds, resolved, active))
        .transpose()?;
    let reference_item = target(relationship, "reference", items)?;
    let reference = reference_item
        .map(|value| target_descriptor(value, items, kinds, resolved, active))
        .transpose()?;
    let nonzero = reference_item.map(is_nonzero).transpose()?.unwrap_or(false);
    if relationship.string("mode")? != "covariance"
        && let Some(value) = item.record.optional("value")
        && value.rational()? < Rational::ZERO
    {
        return Err(value.error("uncertainty value must be nonnegative"));
    }
    let mode = kinds::mode(&relationship.at("mode")?)?;
    let intended = item.record.at("descriptor")?;
    let (kind, measure, role) = kinds::parts(&intended, kinds)?;
    let expected_kind = Kind::Uncertainty(
        mode,
        Box::new(primary.kind().clone()),
        other.as_ref().map(|value| Box::new(value.kind().clone())),
    );
    if kind != expected_kind || role != Role::Difference || measure.offset() != Rational::ZERO {
        return Err(intended.error("uncertainty kind, role or origin disagrees with its targets"));
    }
    let expected = uncertainty(
        mode,
        &primary,
        other.as_ref(),
        reference.as_ref().map(|value| (value, nonzero)),
    );
    if decode::unsupported_scope(&item.formula, "descriptor") {
        // The current missing surface is a relative reference without nonzero evidence.
        // Retain the intended schema, but never construct an admitted substitute.
        if !kind.is_relative() || expected.is_ok() || measure.scale() != Rational::ONE {
            return Err(intended.error(
                "descriptor scope is only available for an unadmitted relative relationship",
            ));
        }
        if reference.as_ref().is_none_or(|value| {
            value.role() != Role::Difference || primary.compatible_kind(value).is_err()
        }) {
            return Err(intended.error(
                "relative reference must still have the intended compatible difference kind",
            ));
        }
        return Ok(None);
    }
    let expected = expected.map_err(|error| intended.error(error))?;
    if measure.scale() != expected.measure().scale() {
        return Err(intended.error("uncertainty scale differs from its targets' derived scale"));
    }
    if kind.is_relative() {
        Ok(Some(expected))
    } else {
        Ok(Some(
            Descriptor::new(kind, measure, role).map_err(|error| intended.error(error))?,
        ))
    }
}

/// Expose only source-declared nonzero evidence, never a derived numerical guess.
fn is_nonzero(item: &Item<'_>) -> Result<bool, Error> {
    match item.formula {
        Formula::Input => item.record.at("nonzero")?.boolean(),
        Formula::Constant => Ok(item.record.at("value")?.rational()? != Rational::ZERO),
        _ => Ok(false),
    }
}

/// Keep source and assurance claims separate from checked descriptor evidence.
fn provenance(item: &Item<'_>) -> Result<Evidence, Error> {
    let record = item.companion.as_ref().unwrap_or(&item.record);
    Ok(Evidence {
        source: record
            .optional("source")
            .map(|value| value.text().map(str::to_owned))
            .transpose()?
            .unwrap_or_else(|| item.record.path.clone()),
        assurance: record
            .optional("assurance")
            .map(|value| value.text().map(str::to_owned))
            .transpose()?
            .unwrap_or_else(|| "Declared reference data; no numerical certification".to_owned()),
    })
}

/// Map each admitted schema alternative directly onto the public declaration API.
fn declaration(
    item: &Item<'_>,
    descriptors: &BTreeMap<Symbol, Descriptor>,
) -> Result<Option<Declaration>, Error> {
    if matches!(item.formula, Formula::Unsupported(_)) {
        return Ok(None);
    }
    if matches!(item.formula, Formula::Metadata) {
        return Ok(Some(Declaration::Metadata(
            item.record.string("value")?.to_owned(),
        )));
    }
    let descriptor = descriptors
        .get(&item.name)
        .cloned()
        .ok_or_else(|| item.record.error("missing admitted descriptor"))?;
    let evidence = provenance(item)?;
    let relation = item.companion.as_ref().map(companion).transpose()?;
    let result = match &item.formula {
        Formula::Input => Declaration::Input {
            descriptor,
            nonzero: is_nonzero(item)?,
            evidence,
        },
        Formula::Constant => Declaration::Constant {
            descriptor,
            value: item.record.at("value")?.rational()?,
            evidence,
        },
        Formula::Magnitude => Declaration::UncertaintyInput {
            descriptor,
            companion: relation.ok_or_else(|| item.record.error("missing companion"))?,
            evidence,
        },
        Formula::Algebra(expression) => {
            let observable = Observable {
                descriptor,
                expression: expression.expression()?,
                evidence,
            };
            match relation {
                Some(companion) => Declaration::Uncertainty {
                    observable,
                    companion,
                },
                None => Declaration::Derived(observable),
            }
        }
        Formula::Metadata | Formula::Unsupported(_) => {
            return Err(item.record.error("invalid numerical declaration"));
        }
    };
    Ok(Some(result))
}

/// Check a declaration probe, its original expression and the declared output contract.
fn check_item(
    item: &Item<'_>,
    context: &Context,
    descriptors: &BTreeMap<Symbol, Descriptor>,
    kinds: &Kinds,
) -> Result<Value, Error> {
    let mut record = json!({
        "name": item.name.as_str(), "category": item.category,
        "descriptor_status": "checked", "expression_status": "not-applicable",
        "relationship_status": "not-applicable", "used_declarations": [],
    });
    if matches!(item.formula, Formula::Metadata) {
        record["descriptor_status"] = json!("metadata");
        return Ok(record);
    }
    if let Some(descriptor) = descriptors.get(&item.name) {
        record["checked_descriptor"] = kinds::evidence(descriptor, kinds, &item.record)?;
    } else {
        record["descriptor_status"] = json!("unsupported");
    }
    if item.companion.is_some() {
        record["relationship_status"] = json!("checked");
    }
    if let Formula::Unsupported(unsupported) = &item.formula {
        record["expression_status"] = json!("unsupported");
        if decode::unsupported_scope(&item.formula, "relationship") {
            record["relationship_status"] = json!("unsupported");
        } else if item.companion.is_some() {
            record["relationship_status"] = json!("declared");
        }
        record["unsupported"] = unsupported.value.clone();
        return Ok(record);
    }
    let probe =
        parse(item.name.as_str(), Limits::default()).map_err(|error| item.record.error(error))?;
    let checked = context
        .check(&probe)
        .map_err(|error| item.record.error(error))?;
    if let Formula::Algebra(expression) = &item.formula {
        let descriptor = descriptors
            .get(&item.name)
            .ok_or_else(|| item.record.error("missing output descriptor"))?;
        let output = context
            .check_against(&expression.expression()?, descriptor)
            .map_err(|error| item.record.error(format!("expression.text: {error}")))?;
        record["expression_status"] = json!("checked");
        checked_fields(&mut record, &output);
    } else {
        checked_fields(&mut record, &checked);
    }
    if let Some(evidence) = checked.companion_evidence() {
        record["companion_evidence"] = match evidence {
            CompanionEvidence::Established(companion) => json!({
                "kind": "established",
                "companion": companion_json(companion)
            }),
            CompanionEvidence::Propagated {
                companion,
                contributors,
            } => json!({
                "kind": "propagated", "companion": companion_json(companion),
                "contributors": contributors.iter().map(|(
                    name,
                    relation
                )| (name.as_str(), companion_json(relation))).collect::<BTreeMap<_, _>>(),
            }),
        };
    }
    Ok(record)
}

/// Serialize stable relationship fields rather than debug output.
fn companion_json(companion: &Companion) -> Value {
    let mode = match companion.mode {
        nrz_units::UncertaintyMode::Radius => "radius",
        nrz_units::UncertaintyMode::Standard => "standard",
        nrz_units::UncertaintyMode::RelativeRadius => "relative-radius",
        nrz_units::UncertaintyMode::RelativeStandard => "relative-standard",
        nrz_units::UncertaintyMode::Variance => "variance",
        nrz_units::UncertaintyMode::Covariance => "covariance",
    };
    let mut value = json!({"mode": mode, "target": companion.value.as_str()});
    if let Some(other) = &companion.other {
        value["other"] = json!(other.as_str());
    }
    if let Some(reference) = &companion.reference {
        value["reference"] = json!(reference.as_str());
    }
    value
}

/// Retain the actual dependency inventory and coordinate conversion evidence.
fn checked_fields(record: &mut Value, checked: &CheckedExpression) {
    record["used_declarations"] = json!(
        checked
            .declarations()
            .keys()
            .map(Symbol::as_str)
            .collect::<Vec<_>>()
    );
    if let Some(conversion) = checked.output_conversion() {
        record["output_conversion"] = json!({
            "scale": conversion.scale.to_string(),
            "shift": conversion.shift.to_string()
        });
    }
}

/// Check every equation's original sides with the authority's side-indexed diagnostics.
fn equations(
    entry: &Record<'_>,
    context: &Context,
    items: &BTreeMap<Symbol, &Item<'_>>,
    kinds: &Kinds,
) -> Result<Vec<Value>, Error> {
    let mut names = BTreeSet::new();
    let mut records = Vec::new();
    for equation in entry.optional_array("equations")? {
        let name = equation.at("name")?.symbol()?;
        if !names.insert(name.clone()) || items.contains_key(&name) {
            return Err(equation.error("duplicate equation name"));
        }
        for key in ["source", "assurance"] {
            equation.string(key)?;
        }
        let mut record = json!({"name": name.as_str(), "category": "equation"});
        match equation.string("kind")? {
            "algebra" => {
                equation.keys(
                    &["name", "kind", "left", "right", "source", "assurance"],
                    &[],
                )?;
                let left = decode::formula(&equation.at("left")?)?;
                let right = decode::formula(&equation.at("right")?)?;
                let (Formula::Algebra(left), Formula::Algebra(right)) = (left, right) else {
                    return Err(equation.error("algebra equations require two algebra sides"));
                };
                let (left, right) = context
                    .check_equation(&left.expression()?, &right.expression()?)
                    .map_err(|error| equation.error(error))?;
                record["expression_status"] = json!("checked");
                record["left_descriptor"] = kinds::evidence(left.descriptor(), kinds, &equation)?;
                record["right_descriptor"] = kinds::evidence(right.descriptor(), kinds, &equation)?;
                checked_fields(&mut record, &right);
            }
            "unsupported" => {
                equation.keys(&["name", "kind", "expression", "source", "assurance"], &[])?;
                let Formula::Unsupported(expression) =
                    decode::formula(&equation.at("expression")?)?
                else {
                    return Err(
                        equation.error("unsupported equation requires an explicit limitation")
                    );
                };
                for dependency in expression.array("dependencies")? {
                    if !items.contains_key(&dependency.symbol()?) {
                        return Err(dependency.error("unknown equation dependency"));
                    }
                }
                if expression.array("scope")?.len() != 1 {
                    return Err(expression.error("equation scope is expression only"));
                }
                record["expression_status"] = json!("unsupported");
                record["unsupported"] = expression.value.clone();
            }
            _ => return Err(equation.error("unknown equation alternative")),
        }
        records.push(record);
    }
    Ok(records)
}

/// Admit all variant replacements against unchanged parameter contracts.
fn variants(
    entry: &Record<'_>,
    items: &BTreeMap<Symbol, &Item<'_>>,
    descriptors: &BTreeMap<Symbol, Descriptor>,
) -> Result<(), Error> {
    let mut names = BTreeSet::new();
    for variant in entry.optional_array("variants")? {
        variant.keys(&["name", "description", "expected"], &["overrides"])?;
        for key in ["name", "description", "expected"] {
            variant.string(key)?;
        }
        if !names.insert(variant.string("name")?) {
            return Err(variant.error("duplicate variant name"));
        }
        if let Some(overrides) = variant.optional("overrides") {
            let values = overrides
                .value
                .as_object()
                .ok_or_else(|| overrides.error("expected override object"))?;
            for (name, value) in values {
                let value = Record::new(value, format!("{}.overrides.{name}", variant.path));
                let symbol = Symbol::new(name).map_err(|error| value.error(error))?;
                let item = items
                    .get(&symbol)
                    .filter(|item| item.category == "parameter")
                    .ok_or_else(|| value.error("override names an absent parameter"))?;
                if matches!(item.formula, Formula::Metadata) {
                    decode::metadata(&item.record.at("metadata")?, &value)?;
                } else {
                    let coordinate = value.rational()?;
                    if coordinate == Rational::ZERO && is_nonzero(item)? {
                        return Err(value.error("replacement contradicts nonzero assumption"));
                    }
                    let descriptor = descriptors
                        .get(&symbol)
                        .ok_or_else(|| value.error("missing parameter descriptor"))?;
                    if descriptor.kind().count_domain().is_some() {
                        descriptor
                            .check_count_literal(coordinate)
                            .map_err(|error| value.error(error))?;
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
