// SPDX-License-Identifier: AGPL-3.0-only

//! Discovery, validation, and derivation of reference problems and fixtures.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::envelope;
use crate::error::{self, Error};

/// Path of the derived exemplar dataset.
pub(crate) const DATASET: &str = "docs/datasets/problems.json";
/// Dataset namespace.
const DATASET_NAMESPACE: &str = "org.naturrhythmuszentrum.docs.dataset.problems";
/// Problem input namespace.
const PROBLEM_NAMESPACE: &str = "org.naturrhythmuszentrum.problems.problem";
/// Fixture input namespace.
const FIXTURE_NAMESPACE: &str = "org.naturrhythmuszentrum.problems.fixture";

/// One input path and its identity inferred from the tree.
#[derive(Debug)]
struct Location {
    /// Root-relative input path.
    path: String,
    /// Directory name carrying the input.
    slug: String,
    /// Declared entry kind inferred from the location.
    kind: &'static str,
}

/// Metadata and mathematical declaration for one entry.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Problem {
    /// Directory-matching slug.
    slug: String,
    /// Human-readable title.
    title: String,
    /// Minted register label.
    register: String,
    /// Problem or fixture kind.
    kind: String,
    /// Mathematical model and its assumptions.
    model: String,
    /// Concrete reference instance.
    instance: String,
}

/// One named input quantity.
///
/// TODO ´todo:code:declare-parameter-quantity-physical-count-or´: declare parameter quantity, physical count or metadata roles explicitly under rule:vision:counts-and-metadata and req:vision:unit-data-migration.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Parameter {
    /// Unique parameter name.
    ///
    /// TODO ´todo:code:bind-parameter-symbols-to-declared-descriptors´: bind parameter symbols to declared descriptors under req:vision:expression-and-symbol-contract.
    name: String,
    /// Human-readable meaning.
    description: String,
    /// Exact input value syntax.
    value: String,
    /// Unit declaration validated after typed decoding.
    ///
    /// TODO ´todo:code:decode-parameter-dimensions-measure-scales-affine´: decode parameter dimensions, measure scales, affine origins and semantic kinds under def:vision:measures and rule:vision:semantic-kinds-and-roles.
    unit: Value,
    /// Uncertainty declaration validated after typed decoding.
    ///
    /// TODO ´todo:code:declare-input-uncertainty-expressions-with-their´: declare input uncertainty expressions with their own dimensions, measures, kinds and assurance under rule:vision:uncertainty-unit-algebra.
    uncertainty: Value,
}

/// One stated mathematical result.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Observable {
    /// Unique observable name.
    name: String,
    /// Human-readable meaning.
    description: String,
    /// Unit declaration validated after typed decoding.
    ///
    /// TODO ´todo:code:compare-observable-descriptors-with-algebraically-derived´: compare observable descriptors with algebraically derived dimensions, measures and kinds under req:vision:algebraic-unit-checks.
    unit: Value,
    /// Symbolic expression for the observable.
    ///
    /// TODO ´todo:code:migrate-closed-forms-to-the-generic´: migrate closed forms to the generic expression and declared symbol contract without treating legacy strings as checked under req:vision:expression-and-symbol-contract.
    closed_form: String,
    /// Expected value string.
    value: String,
    /// Exact or rounded precision category.
    value_kind: String,
    /// Primary derivation.
    derivation: String,
    /// Independent check where supplied.
    second_route: String,
}

/// One change to the base parameters and its expected outcome.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Variant {
    /// Variant identifier.
    name: String,
    /// Human-readable change.
    description: String,
    /// Replacement parameter value strings.
    ///
    /// TODO ´todo:code:check-variant-replacements-against-count-domains´: check variant replacements against count domains, metadata categories and quantity declarations under req:vision:unit-data-migration.
    #[serde(default)]
    overrides: BTreeMap<String, String>,
    /// Expected mathematical or numerical behavior.
    expected: String,
}

/// Error classes declared by an entry.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Errors {
    /// Bounded error classes.
    bounded: Vec<String>,
    /// Estimated error classes.
    estimated: Vec<String>,
    /// Excluded error classes.
    excluded: Vec<String>,
}

/// Labels whose claims and implementation bites the entry stresses.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Stress {
    /// Claim labels exercised by the entry.
    claims: Vec<String>,
    /// Implementation-bite labels exercised by the entry.
    bites: Vec<String>,
}

/// The strict payload shared by input files and dataset entries.
///
/// TODO ´todo:code:carry-versioned-quantity-and-uncertainty-declarations´: carry versioned quantity and uncertainty declarations through entry serialization under req:vision:unit-data-migration and def:vision:intrinsic-uncertainty-companions.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    /// Core mathematical declaration.
    problem: Problem,
    /// Input quantities in source order.
    parameters: Vec<Parameter>,
    /// Expected results in source order.
    observables: Vec<Observable>,
    /// Optional variants in source order.
    #[serde(default)]
    variants: Vec<Variant>,
    /// Bounded, estimated, and excluded errors.
    errors: Errors,
    /// Cited claims and bites.
    stress: Stress,
}

/// The dataset's only payload key.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DatasetPayload {
    /// Ordered problem and fixture payloads.
    entries: Vec<Entry>,
}

/// Derive the dataset from all validated entry files in catalogue order.
///
/// TODO ´todo:code:derive-migrated-unit-datasets-only-after´: derive migrated unit datasets only after expression and symbol validation under req:vision:unit-data-migration.
pub(crate) fn derive(root: &Path, owners: &BTreeSet<String>) -> Result<(Vec<u8>, Value), Error> {
    let locations = layout(root)?;
    let mut sources = Vec::new();
    let mut entries = Vec::new();
    for location in &locations {
        let value = envelope::read_toml(root, &location.path)?;
        let namespace = if location.kind == "problem" {
            PROBLEM_NAMESPACE
        } else {
            FIXTURE_NAMESPACE
        };
        envelope::validate(&value, &location.path, namespace, owners)?;
        let object = value
            .as_object()
            .ok_or_else(|| invalid(&location.path, "envelope", "entry must be an object"))?;
        let sources_in = envelope::string_array(object, &location.path, "sources")?;
        let labels = envelope::string_array(object, &location.path, "labels")?;
        if !sources_in.is_empty() || labels.len() != 1 {
            return Err(Error::Envelope {
                path: location.path.clone(),
                detail: "sources must be empty and labels must contain exactly one label"
                    .to_owned(),
            });
        }
        let payload = envelope::payload(&value, &location.path)?;
        let entry: Entry = serde_json::from_value(payload)
            .map_err(|problem| invalid(&location.path, "typed payload", &problem.to_string()))?;
        validate_entry(&entry, location, &labels[0])?;
        entries.push(
            serde_json::to_value(entry).map_err(|problem| {
                invalid(&location.path, "typed payload", &problem.to_string())
            })?,
        );
        sources.push(location.path.clone());
    }
    let payload = BTreeMap::from([("entries".to_owned(), Value::Array(entries))]);
    let bytes = envelope::encode(DATASET, DATASET_NAMESPACE, "NRZ", &sources, &[], &payload)?;
    let value = serde_json::from_slice(&bytes).map_err(|problem| Error::JsonSyntax {
        path: DATASET.to_owned(),
        detail: problem.to_string(),
    })?;
    Ok((bytes, value))
}

/// Validate a committed dataset before a template receives its entries.
///
/// TODO ´todo:code:revalidate-committed-quantity-and-uncertainty-expressions´: revalidate committed quantity and uncertainty expressions before rendering under req:vision:algebraic-unit-checks and rule:vision:uncertainty-unit-algebra.
pub(crate) fn validate_dataset(value: &Value, owners: &BTreeSet<String>) -> Result<(), Error> {
    envelope::validate(value, DATASET, DATASET_NAMESPACE, owners)?;
    let object = value
        .as_object()
        .ok_or_else(|| invalid(DATASET, "envelope", "dataset must be an object"))?;
    let sources = envelope::string_array(object, DATASET, "sources")?;
    let labels = envelope::string_array(object, DATASET, "labels")?;
    if value.get("owner").and_then(Value::as_str) != Some("NRZ") || !labels.is_empty() {
        return Err(Error::Envelope {
            path: DATASET.to_owned(),
            detail: "problems dataset owner or labels differ from its declaration".to_owned(),
        });
    }
    let payload = envelope::payload(value, DATASET)?;
    let dataset: DatasetPayload = serde_json::from_value(payload)
        .map_err(|problem| invalid(DATASET, "typed payload", &problem.to_string()))?;
    if sources.len() != dataset.entries.len() {
        return Err(invalid(
            DATASET,
            "identity",
            "sources and entries differ in length",
        ));
    }
    let mut previous: Option<(bool, String)> = None;
    for (source, entry) in sources.iter().zip(&dataset.entries) {
        let location = location_from_source(source)?;
        let order = (location.kind == "fixture", location.slug.clone());
        if previous.as_ref().is_some_and(|old| old >= &order) {
            return Err(invalid(
                DATASET,
                "identity",
                "entries are not in catalogue order",
            ));
        }
        previous = Some(order);
        validate_entry(entry, &location, &entry.problem.register)?;
    }
    Ok(())
}

/// Discover the permitted layout without following symbolic links.
fn layout(root: &Path) -> Result<Vec<Location>, Error> {
    let base = root.join("problems");
    let metadata = fs::symlink_metadata(&base).map_err(|source| {
        invalid(
            "problems",
            "layout",
            &format!("cannot read directory: {source}"),
        )
    })?;
    if !metadata.file_type().is_dir() {
        return Err(invalid("problems", "layout", "expected a real directory"));
    }
    let mut problems = Vec::new();
    let mut fixtures = Vec::new();
    for entry in fs::read_dir(&base).map_err(|source| error::io("problems", source))? {
        let entry = entry.map_err(|source| error::io("problems", source))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid("problems", "layout", "entry name is not UTF-8"))?;
        let path = format!("problems/{name}");
        let kind = entry
            .file_type()
            .map_err(|source| error::io(&path, source))?;
        if kind.is_symlink() {
            return Err(invalid(&path, "layout", "symbolic link is not allowed"));
        }
        if name == "README.md" || name == "README.md.j2" {
            if !kind.is_file() {
                return Err(invalid(&path, "layout", "expected a regular file"));
            }
        } else if name == "fixtures" {
            if !kind.is_dir() {
                return Err(invalid(&path, "layout", "expected a real directory"));
            }
            collect_slugs(root, &path, "fixture", "fixture.toml", &mut fixtures)?;
        } else if valid_slug(&name) && kind.is_dir() {
            exact_file(root, &path, "problem.toml")?;
            problems.push(Location {
                path: format!("{path}/problem.toml"),
                slug: name,
                kind: "problem",
            });
        } else {
            return Err(invalid(
                &path,
                "layout",
                "unexpected entry or malformed slug",
            ));
        }
    }
    problems.sort_by(|left, right| left.slug.cmp(&right.slug));
    fixtures.sort_by(|left, right| left.slug.cmp(&right.slug));
    problems.extend(fixtures);
    Ok(problems)
}

/// Collect fixture slug directories under the fixture root.
fn collect_slugs(
    root: &Path,
    directory: &str,
    kind: &'static str,
    filename: &str,
    output: &mut Vec<Location>,
) -> Result<(), Error> {
    for entry in
        fs::read_dir(root.join(directory)).map_err(|source| error::io(directory, source))?
    {
        let entry = entry.map_err(|source| error::io(directory, source))?;
        let slug = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid(directory, "layout", "entry name is not UTF-8"))?;
        let path = format!("{directory}/{slug}");
        let file_type = entry
            .file_type()
            .map_err(|source| error::io(&path, source))?;
        if !valid_slug(&slug) || !file_type.is_dir() {
            return Err(invalid(&path, "layout", "expected a real slug directory"));
        }
        exact_file(root, &path, filename)?;
        output.push(Location {
            path: format!("{path}/{filename}"),
            slug,
            kind,
        });
    }
    Ok(())
}

/// Require exactly one named regular file inside a slug directory.
fn exact_file(root: &Path, directory: &str, filename: &str) -> Result<(), Error> {
    let entries =
        fs::read_dir(root.join(directory)).map_err(|source| error::io(directory, source))?;
    let mut count = 0;
    for entry in entries {
        let entry = entry.map_err(|source| error::io(directory, source))?;
        let path = format!("{directory}/{}", entry.file_name().to_string_lossy());
        let file_type = entry
            .file_type()
            .map_err(|source| error::io(&path, source))?;
        if entry.file_name() != filename || !file_type.is_file() {
            return Err(invalid(&path, "layout", "unexpected file or symbolic link"));
        }
        count += 1;
    }
    if count != 1 {
        return Err(invalid(
            directory,
            "layout",
            "expected exactly one entry file",
        ));
    }
    Ok(())
}

/// Parse a dataset source path into the same identity as a tree location.
fn location_from_source(source: &str) -> Result<Location, Error> {
    let parts: Vec<&str> = source.split('/').collect();
    let (kind, slug) = match parts.as_slice() {
        ["problems", slug, "problem.toml"] => ("problem", *slug),
        ["problems", "fixtures", slug, "fixture.toml"] => ("fixture", *slug),
        _ => return Err(invalid(source, "identity", "invalid entry source path")),
    };
    if !valid_slug(slug) {
        return Err(invalid(source, "identity", "malformed source slug"));
    }
    Ok(Location {
        path: source.to_owned(),
        slug: slug.to_owned(),
        kind,
    })
}

/// Recognize lowercase ASCII slugs with single internal hyphens.
fn valid_slug(slug: &str) -> bool {
    let mut bytes = slug.bytes();
    if !matches!(bytes.next(), Some(first) if first.is_ascii_lowercase()) {
        return false;
    }
    let mut hyphen = false;
    for byte in bytes {
        if byte == b'-' {
            if hyphen {
                return false;
            }
            hyphen = true;
        } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            hyphen = false;
        } else {
            return false;
        }
    }
    !hyphen
}

/// Apply rules four through nine after strict payload decoding.
///
/// TODO ´todo:code:assemble-declared-symbols-and-check-observable´: assemble declared symbols and check observable formulas, uncertainty expressions and reference cycles under req:vision:expression-and-symbol-contract.
fn validate_entry(entry: &Entry, location: &Location, label: &str) -> Result<(), Error> {
    let path = &location.path;
    if entry.parameters.is_empty() || entry.observables.is_empty() {
        return Err(invalid(
            path,
            "typed payload",
            "parameters and observables must be non-empty",
        ));
    }
    if entry.problem.slug != location.slug
        || entry.problem.kind != location.kind
        || entry.problem.register != label
    {
        return Err(invalid(
            path,
            "identity",
            "slug, kind, or register differs from its location or envelope",
        ));
    }
    let mut names = BTreeSet::new();
    for parameter in &entry.parameters {
        if !names.insert(parameter.name.as_str()) {
            return Err(invalid(path, "identity", "duplicate parameter name"));
        }
    }
    let mut observable_names = BTreeSet::new();
    for observable in &entry.observables {
        if !observable_names.insert(observable.name.as_str()) {
            return Err(invalid(path, "identity", "duplicate observable name"));
        }
    }
    for variant in &entry.variants {
        if variant
            .overrides
            .keys()
            .any(|name| !names.contains(name.as_str()))
        {
            return Err(invalid(
                path,
                "identity",
                "override names an absent parameter",
            ));
        }
    }
    for parameter in &entry.parameters {
        check_value(path, &parameter.value)?;
        nested_value(path, &parameter.unit, "offset")?;
        nested_value(path, &parameter.uncertainty, "half_width")?;
        nested_value(path, &parameter.uncertainty, "sigma")?;
    }
    for observable in &entry.observables {
        check_value(path, &observable.value)?;
        nested_value(path, &observable.unit, "offset")?;
    }
    for variant in &entry.variants {
        for value in variant.overrides.values() {
            check_value(path, value)?;
        }
    }
    for parameter in &entry.parameters {
        check_unit(path, &parameter.unit)?;
    }
    for observable in &entry.observables {
        check_unit(path, &observable.unit)?;
    }
    for parameter in &entry.parameters {
        check_uncertainty(path, &parameter.uncertainty)?;
    }
    check_exactness(path, &entry.observables)?;
    if !envelope::valid_label(&entry.problem.register) {
        return Err(invalid(path, "labels", "malformed register label"));
    }
    for claim in &entry.stress.claims {
        if !envelope::valid_label(claim) {
            return Err(invalid(path, "labels", "malformed claim label"));
        }
    }
    for bite in &entry.stress.bites {
        if !envelope::valid_label(bite) || !bite.starts_with("step:nrz:") {
            return Err(invalid(path, "labels", "bite must be a step:nrz: label"));
        }
    }
    Ok(())
}

/// Check the declared exact or rounded precision of every observable.
///
/// TODO ´todo:code:validate-count-exactness-separately-from-continuous´: validate count exactness separately from continuous uncertainty precision under def:vision:counts and rule:vision:uncertainty-unit-algebra.
fn check_exactness(path: &str, observables: &[Observable]) -> Result<(), Error> {
    for observable in observables {
        let count = match observable.value_kind.as_str() {
            "exact" => {
                if observable.value.contains('e') || observable.value.contains('E') {
                    return Err(invalid(
                        path,
                        "exactness",
                        "exact value uses scientific notation",
                    ));
                }
                None
            }
            "decimal-12" => Some(12),
            "decimal-4" => Some(4),
            _ => return Err(invalid(path, "exactness", "unknown value_kind")),
        };
        if let Some(expected) = count {
            let mantissa = observable.value.split(&['e', 'E'][..]).next().unwrap_or("");
            let digits = mantissa
                .bytes()
                .filter(u8::is_ascii_digit)
                .skip_while(|byte| *byte == b'0')
                .count();
            if digits != expected {
                return Err(invalid(path, "exactness", "wrong significant-digit count"));
            }
        }
    }
    Ok(())
}

/// Validate one optional nested numeric field before unit or uncertainty shape.
///
/// TODO ´todo:code:interpret-scale-factors-affine-offsets-and´: interpret scale factors, affine offsets and uncertainty values according to their declared roles under rule:vision:measure-conversions and rule:vision:uncertainty-unit-algebra.
fn nested_value(path: &str, value: &Value, key: &str) -> Result<(), Error> {
    if let Some(entry) = value.as_object().and_then(|object| object.get(key)) {
        let text = entry
            .as_str()
            .ok_or_else(|| invalid(path, "values", &format!("{key} must be a value string")))?;
        check_value(path, text)?;
    }
    Ok(())
}

/// Recognize the specified grammar for exact decimal, fraction, and exponent strings.
///
/// TODO ´todo:code:apply-value-domain-checks-after-syntax´: apply value-domain checks after syntax for counts, metadata and continuous uncertainty under rule:vision:counts-and-metadata and rule:vision:uncertainty-unit-algebra.
fn check_value(path: &str, value: &str) -> Result<(), Error> {
    if !valid_value(value) {
        return Err(invalid(
            path,
            "values",
            &format!("invalid value string {value}"),
        ));
    }
    Ok(())
}

/// Match unsigned decimal components without leading-sign or whitespace allowances.
fn decimal(text: &str) -> bool {
    let mut parts = text.split('.');
    let first = parts.next().unwrap_or("");
    let second = parts.next();
    !first.is_empty()
        && first.bytes().all(|byte| byte.is_ascii_digit())
        && second.is_none_or(|digits| {
            !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
        })
        && parts.next().is_none()
}

/// Match the value grammar without a regular-expression dependency.
fn valid_value(value: &str) -> bool {
    let body = value.strip_prefix('-').unwrap_or(value);
    if let Some((numerator, denominator)) = body.split_once('/') {
        return !numerator.is_empty()
            && !denominator.is_empty()
            && numerator.bytes().all(|byte| byte.is_ascii_digit())
            && denominator.bytes().all(|byte| byte.is_ascii_digit());
    }
    if let Some(index) = body.find(&['e', 'E'][..]) {
        let (mantissa, exponent) = body.split_at(index);
        let exponent = exponent[1..]
            .strip_prefix('-')
            .or_else(|| exponent[1..].strip_prefix('+'))
            .unwrap_or_else(|| &exponent[1..]);
        return decimal(mantissa)
            && !exponent.is_empty()
            && exponent.bytes().all(|byte| byte.is_ascii_digit());
    }
    decimal(body)
}

/// Require the finite set of unit keys and their declared value types.
///
/// TODO ´todo:code:validate-dimension-algebra-measure-scales-affine´: validate dimension algebra, measure scales, affine roles and semantic kinds beyond table shape under req:vision:algebraic-unit-checks and rule:vision:measure-conversions.
fn check_unit(path: &str, value: &Value) -> Result<(), Error> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(path, "units", "unit must be an inline table"))?;
    if !object.get("display").is_some_and(Value::is_string) {
        return Err(invalid(path, "units", "display must be a string"));
    }
    for (key, value) in object {
        let valid = match key.as_str() {
            "display" | "offset" => value.is_string(),
            "m" | "kg" | "s" | "A" | "K" | "mol" | "cd" => value.as_i64().is_some(),
            _ => false,
        };
        if !valid {
            return Err(invalid(
                path,
                "units",
                &format!("invalid unit key or type {key}"),
            ));
        }
    }
    Ok(())
}

/// Validate exactly the key set associated with each uncertainty kind.
///
/// TODO ´todo:code:derive-uncertainty-dimensions-and-kinds-for´: derive uncertainty dimensions and kinds for absolute, relative and covariance expressions instead of copying the value descriptor under rule:vision:uncertainty-unit-algebra.
fn check_uncertainty(path: &str, value: &Value) -> Result<(), Error> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(path, "uncertainty", "uncertainty must be an inline table"))?;
    let kind = object
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(path, "uncertainty", "kind must be a string"))?;
    let keys: BTreeSet<&str> = object.keys().map(String::as_str).collect();
    let expected = match kind {
        "none" => BTreeSet::from(["kind"]),
        "enclosure" => BTreeSet::from(["kind", "half_width", "source"]),
        "standard" => BTreeSet::from(["kind", "sigma", "source"]),
        _ => return Err(invalid(path, "uncertainty", "unknown kind")),
    };
    if keys != expected {
        return Err(invalid(
            path,
            "uncertainty",
            "keys differ from kind's required shape",
        ));
    }
    if kind != "none" && !object.get("source").is_some_and(Value::is_string) {
        return Err(invalid(path, "uncertainty", "source must be a string"));
    }
    Ok(())
}

/// Construct a path-bearing problem-stage error with its rule name.
fn invalid(path: &str, rule: &str, detail: &str) -> Error {
    Error::Payload {
        path: path.to_owned(),
        detail: format!("{rule}: {detail}"),
    }
}

/// Boundary tests for the ordered problem-stage rules.
///
/// TODO ´todo:code:exercise-algebraic-acceptance-and-rejection-for´: exercise algebraic acceptance and rejection for formulas, symbols, affine roles and count versus ratio versus metadata under req:vision:algebraic-unit-checks.
#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;

    use serde_json::{Value, json};

    use super::{Entry, Location, derive, layout, validate_entry};
    use crate::error::Error;

    /// Build a complete minimal entry for direct rule tests.
    ///
    /// TODO ´todo:code:supply-declared-measures-kinds-uncertainty-descriptors´: supply declared measures, kinds, uncertainty descriptors and bound symbols in rule fixtures under req:vision:unit-data-migration.
    fn sample() -> Value {
        json!({
            "problem": {
                "slug": "alpha", "title": "Alpha", "register": "prob:exemplars:alpha",
                "kind": "problem", "model": "Model", "instance": "Instance"
            },
            "parameters": [{
                "name": "p", "description": "Input", "value": "1/2",
                "unit": {"display": "m", "m": 1}, "uncertainty": {"kind": "none"}
            }],
            "observables": [{
                "name": "answer", "description": "Output", "unit": {"display": "m", "m": 1},
                "closed_form": "p", "value": "0.500000000000", "value_kind": "decimal-12",
                "derivation": "Direct", "second_route": "none"
            }],
            "variants": [{
                "name": "base", "description": "Base", "overrides": {"p": "1/2"},
                "expected": "Half"
            }],
            "errors": {"bounded": [], "estimated": [], "excluded": []},
            "stress": {
                "claims": ["rule:vision:example"],
                "bites": ["step:nrz:bite-exemplar-register"]
            }
        })
    }

    /// Decode and validate a modified sample, returning its rule error.
    fn invalid_sample(value: Value) -> Result<(), Box<dyn std::error::Error>> {
        let entry: Entry = serde_json::from_value(value)?;
        let location = Location {
            path: "problems/alpha/problem.toml".to_owned(),
            slug: "alpha".to_owned(),
            kind: "problem",
        };
        assert!(validate_entry(&entry, &location, "prob:exemplars:alpha").is_err());
        Ok(())
    }

    /// Write one minimal valid entry under its inferred location.
    ///
    /// TODO ´todo:code:write-migrated-problem-and-fixture-declarations´: write migrated problem and fixture declarations for data-time unit cases under req:vision:unit-data-migration.
    fn write_entry(root: &Path, kind: &str, slug: &str) -> Result<(), std::io::Error> {
        let (directory, filename) = if kind == "fixture" {
            (format!("problems/fixtures/{slug}"), "fixture.toml")
        } else {
            (format!("problems/{slug}"), "problem.toml")
        };
        fs::create_dir_all(root.join(&directory))?;
        let source = format!(
            "# SPDX-License-Identifier: CC0-1.0\nnamespace = \"org.naturrhythmuszentrum.problems.{kind}\"\nversion = [1, 0, 0]\nowner = \"NRZ\"\nlicense = \"CC0-1.0\"\nsources = []\nlabels = [\"prob:exemplars:{slug}\"]\n[problem]\nslug = \"{slug}\"\ntitle = \"Sample\"\nregister = \"prob:exemplars:{slug}\"\nkind = \"{kind}\"\nmodel = \"Model\"\ninstance = \"Instance\"\n[[parameters]]\nname = \"p\"\ndescription = \"Input\"\nvalue = \"1\"\nunit = {{ display = \"1\" }}\nuncertainty = {{ kind = \"none\" }}\n[[observables]]\nname = \"answer\"\ndescription = \"Output\"\nunit = {{ display = \"1\" }}\nclosed_form = \"p\"\nvalue = \"1\"\nvalue_kind = \"exact\"\nderivation = \"Direct\"\nsecond_route = \"none\"\n[errors]\nbounded = []\nestimated = []\nexcluded = []\n[stress]\nclaims = []\nbites = [\"step:nrz:bite-exemplar-register\"]\n"
        );
        fs::write(root.join(directory).join(filename), source)
    }

    /// Return the one declared owner for an isolated fixture tree.
    fn owners() -> BTreeSet<String> {
        BTreeSet::from(["NRZ".to_owned()])
    }

    /// Decode one valid problem and one valid fixture into the ordered dataset.
    ///
    /// TODO ´todo:code:verify-valid-derived-units-and-intrinsic´: verify valid derived units and intrinsic uncertainty declarations survive dataset derivation under req:vision:algebraic-unit-checks.
    ///
    /// ´test:unit:decodes-valid-problem-and-fixture´
    #[test]
    fn decodes_valid_problem_and_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_entry(directory.path(), "fixture", "beta")?;
        write_entry(directory.path(), "problem", "alpha")?;
        let (_, value) = derive(directory.path(), &owners())?;
        assert_eq!(value["entries"][0]["problem"]["slug"], "alpha");
        assert_eq!(value["entries"][1]["problem"]["slug"], "beta");
        Ok(())
    }

    /// Reject a stray file at the top of the problems tree.
    /// ´test:unit:layout-refuses-stray-file´
    #[test]
    fn layout_refuses_stray_file() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_entry(directory.path(), "problem", "alpha")?;
        fs::write(directory.path().join("problems/stray.txt"), "stray")?;
        assert!(matches!(
            layout(directory.path()),
            Err(Error::Payload { .. })
        ));
        Ok(())
    }

    /// Reject a second file inside an otherwise valid slug directory.
    /// ´test:unit:layout-refuses-second-file´
    #[test]
    fn layout_refuses_second_file() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_entry(directory.path(), "problem", "alpha")?;
        fs::write(directory.path().join("problems/alpha/extra"), "extra")?;
        assert!(matches!(
            layout(directory.path()),
            Err(Error::Payload { .. })
        ));
        Ok(())
    }

    /// Reject a slug with a repeated hyphen.
    /// ´test:unit:layout-refuses-malformed-slug´
    #[test]
    fn layout_refuses_malformed_slug() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_entry(directory.path(), "problem", "bad--slug")?;
        assert!(matches!(
            layout(directory.path()),
            Err(Error::Payload { .. })
        ));
        Ok(())
    }

    /// Reject an input namespace that disagrees with its tree location.
    /// ´test:unit:envelope-refuses-wrong-namespace´
    #[test]
    fn envelope_refuses_wrong_namespace() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_entry(directory.path(), "problem", "alpha")?;
        let path = directory.path().join("problems/alpha/problem.toml");
        let source = fs::read_to_string(&path)?;
        fs::write(
            &path,
            source.replace("problems.problem", "problems.fixture"),
        )?;
        assert!(matches!(
            derive(directory.path(), &owners()),
            Err(Error::Envelope { .. })
        ));
        Ok(())
    }

    /// Reject a second envelope label even when both labels are well formed.
    /// ´test:unit:envelope-refuses-second-label´
    #[test]
    fn envelope_refuses_second_label() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_entry(directory.path(), "problem", "alpha")?;
        let path = directory.path().join("problems/alpha/problem.toml");
        let source = fs::read_to_string(&path)?;
        fs::write(
            &path,
            source.replace(
                "labels = [\"prob:exemplars:alpha\"]",
                "labels = [\"prob:exemplars:alpha\", \"prob:exemplars:beta\"]",
            ),
        )?;
        assert!(matches!(
            derive(directory.path(), &owners()),
            Err(Error::Envelope { .. })
        ));
        Ok(())
    }

    /// Catch a register that differs from the envelope label.
    /// ´test:unit:identity-refuses-register-mismatch´
    #[test]
    fn identity_refuses_register_mismatch() -> Result<(), Box<dyn std::error::Error>> {
        let mut value = sample();
        value["problem"]["register"] = json!("prob:exemplars:other");
        invalid_sample(value)
    }

    /// Reject a value string outside the exact grammar.
    ///
    /// TODO ´todo:code:reject-fractional-physical-counts-while-admitting´: reject fractional physical counts while admitting continuous ratios and count standard uncertainty under def:vision:counts and rule:vision:uncertainty-unit-algebra.
    ///
    /// ´test:unit:values-refuse-invalid-number´
    #[test]
    fn values_refuse_invalid_number() -> Result<(), Box<dyn std::error::Error>> {
        let mut value = sample();
        value["parameters"][0]["value"] = json!("+1");
        invalid_sample(value)
    }

    /// Reject a unit key outside the seven base exponents.
    ///
    /// TODO ´todo:code:reject-invalid-measure-scales-origins-affine´: reject invalid measure scales, origins, affine roles and mismatched dimensions under rule:vision:measure-conversions and req:vision:algebraic-unit-checks.
    ///
    /// ´test:unit:units-refuse-unknown-key´
    #[test]
    fn units_refuse_unknown_key() -> Result<(), Box<dyn std::error::Error>> {
        let mut value = sample();
        value["parameters"][0]["unit"]["gram"] = json!(1);
        invalid_sample(value)
    }

    /// Reject an enclosure that omits its source.
    ///
    /// TODO ´todo:code:reject-uncertainty-descriptor-mismatches-and-missing´: reject uncertainty descriptor mismatches and missing correlations without treating standard uncertainty as a radius under rule:vision:uncertainty-unit-algebra.
    ///
    /// ´test:unit:uncertainty-refuses-incomplete-shape´
    #[test]
    fn uncertainty_refuses_incomplete_shape() -> Result<(), Box<dyn std::error::Error>> {
        let mut value = sample();
        value["parameters"][0]["uncertainty"] = json!({"kind": "enclosure", "half_width": "1/10"});
        invalid_sample(value)
    }

    /// Reject a decimal with fewer significant digits than declared.
    ///
    /// TODO ´todo:code:test-discrete-count-exactness-independently-of´: test discrete count exactness independently of rounded uncertainty values under def:vision:counts and rule:vision:uncertainty-unit-algebra.
    ///
    /// ´test:unit:exactness-refuses-wrong-precision´
    #[test]
    fn exactness_refuses_wrong_precision() -> Result<(), Box<dyn std::error::Error>> {
        let mut value = sample();
        value["observables"][0]["value"] = json!("0.5");
        invalid_sample(value)
    }

    /// Reject a bite label outside the step and NRZ area.
    /// ´test:unit:labels-refuse-wrong-bite-kind´
    #[test]
    fn labels_refuse_wrong_bite_kind() -> Result<(), Box<dyn std::error::Error>> {
        let mut value = sample();
        value["stress"]["bites"][0] = json!("rule:vision:example");
        invalid_sample(value)
    }

    /// Report an invalid envelope before attempting to decode an invalid payload.
    /// ´test:unit:envelope-precedes-payload-error´
    #[test]
    fn envelope_precedes_payload_error() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_entry(directory.path(), "problem", "alpha")?;
        let path = directory.path().join("problems/alpha/problem.toml");
        let source = fs::read_to_string(&path)?;
        fs::write(
            &path,
            source
                .replace("version = [1, 0, 0]", "version = [2, 0, 0]")
                .replace("title = \"Sample\"", "bad_field = \"Value\""),
        )?;
        assert!(matches!(
            derive(directory.path(), &owners()),
            Err(Error::Envelope { .. })
        ));
        Ok(())
    }
}
