// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit semantic identities, measures and operation rules.

use std::collections::{BTreeMap, BTreeSet};

use nrz_cas::Rational;
use nrz_units::{
    BinaryOp, CountDomain, Descriptor, Dimension, ExponentPolicy, Kind, Measure, Registry, Role,
    Scale, UncertaintyMode,
};
use serde_json::{Value, json};

use super::record::{Record, exact};
use crate::error::Error;

/// Declared names mapped to independently constructed semantic kinds.
pub(super) type Kinds = BTreeMap<String, Kind>;

/// Decode all seven independently specified exact rational exponents.
pub(super) fn dimension(record: &Record<'_>) -> Result<Dimension, Error> {
    let values = record.list()?;
    if values.len() != 7 {
        return Err(record.error("dimensions must have seven exact rational strings"));
    }
    let mut exponents = std::array::from_fn(|_| Rational::zero());
    for (index, value) in values.iter().enumerate() {
        exponents[index] = value.rational()?;
    }
    Dimension::new(exponents).map_err(|error| record.error(error))
}

/// Decode a mode without conflating standard uncertainty and deterministic radius.
pub(super) fn mode(record: &Record<'_>) -> Result<UncertaintyMode, Error> {
    match record.text()? {
        "radius" => Ok(UncertaintyMode::Radius),
        "standard" => Ok(UncertaintyMode::Standard),
        "relative-radius" => Ok(UncertaintyMode::RelativeRadius),
        "relative-standard" => Ok(UncertaintyMode::RelativeStandard),
        "variance" => Ok(UncertaintyMode::Variance),
        "covariance" => Ok(UncertaintyMode::Covariance),
        _ => Err(record.error("unknown uncertainty mode")),
    }
}

/// Construct a bounded acyclic kind graph and register only source-declared rules.
pub(super) fn registry(entry: &Record<'_>) -> Result<(Registry, Kinds), Error> {
    let policy = match entry.string("exponent_policy")? {
        "integral" => ExponentPolicy::Integral,
        "rational" => ExponentPolicy::Rational,
        _ => {
            return Err(entry
                .at("exponent_policy")?
                .error("unknown exponent policy"));
        }
    };
    let mut records = BTreeMap::new();
    for record in entry.array("kinds")? {
        let name = record.at("name")?.symbol()?.as_str().to_owned();
        if records.insert(name, record.clone()).is_some() {
            return Err(record.error("duplicate kind name"));
        }
    }
    let mut kinds = BTreeMap::new();
    for name in records.keys() {
        resolve(name, &records, &mut kinds, &mut BTreeSet::new())?;
    }
    let mut registry = Registry::new(policy);
    for (name, kind) in &kinds {
        kind.dimension()
            .and_then(|value| value.admit(policy))
            .map_err(|error| records[name].error(error))?;
    }
    for rule in entry.array("kind_rules")? {
        let operation = rule.string("operation")?;
        rule.string("reason")?;
        if operation == "power" {
            rule.keys(&["operation", "base", "exponent", "result", "reason"], &[])?;
            registry
                .register_power(
                    lookup(&kinds, &rule.at("base")?)?,
                    rule.at("exponent")?.rational()?,
                    lookup(&kinds, &rule.at("result")?)?,
                )
                .map_err(|error| rule.error(error))?;
        } else {
            rule.keys(&["operation", "left", "right", "result", "reason"], &[])?;
            let operation = match operation {
                "multiply" => BinaryOp::Multiply,
                "divide" => BinaryOp::Divide,
                _ => return Err(rule.error("unknown kind operation")),
            };
            registry
                .register_binary(
                    operation,
                    lookup(&kinds, &rule.at("left")?)?,
                    lookup(&kinds, &rule.at("right")?)?,
                    lookup(&kinds, &rule.at("result")?)?,
                )
                .map_err(|error| rule.error(error))?;
        }
    }
    Ok((registry, kinds))
}

/// Resolve one explicitly named kind, preserving forward references and cycle refusals.
fn resolve(
    name: &str,
    records: &BTreeMap<String, Record<'_>>,
    kinds: &mut Kinds,
    active: &mut BTreeSet<String>,
) -> Result<Kind, Error> {
    if let Some(kind) = kinds.get(name) {
        return Ok(kind.clone());
    }
    let record = records.get(name).ok_or_else(|| Error::Payload {
        path: "kinds".to_owned(),
        detail: format!("unknown kind {name}"),
    })?;
    if active.len() >= 128 || !active.insert(name.to_owned()) {
        return Err(record.error("kind cycle or depth limit"));
    }
    let dimensions = dimension(&record.at("dimensions")?)?;
    let kind = match record.string("type")? {
        "ratio" | "angle" | "named" => {
            record.keys(&["name", "type", "dimensions"], &[])?;
            match record.string("type")? {
                "ratio" => Kind::Ratio,
                "angle" => Kind::Angle,
                _ => Kind::Named(record.at("name")?.symbol()?, Box::new(dimensions.clone())),
            }
        }
        "count" => {
            record.keys(&["name", "type", "dimensions", "thing", "domain"], &[])?;
            let domain = record.at("domain")?;
            domain.keys(&[], &["minimum", "maximum"])?;
            Kind::Count(
                record.at("thing")?.symbol()?,
                CountDomain::new(domain.bound("minimum")?, domain.bound("maximum")?)
                    .map_err(|error| domain.error(error))?,
            )
        }
        "uncertainty" => {
            record.keys(
                &["name", "type", "dimensions", "mode", "value_kind"],
                &["other_kind"],
            )?;
            let value = resolve(record.string("value_kind")?, records, kinds, active)
                .map_err(|error| record.error(error))?;
            let other = record
                .optional("other_kind")
                .map(|other| resolve(other.text()?, records, kinds, active).map(Box::new))
                .transpose()?;
            Kind::Uncertainty(mode(&record.at("mode")?)?, Box::new(value), other)
        }
        _ => return Err(record.error("unknown kind type")),
    };
    if kind.dimension().map_err(|error| record.error(error))? != dimensions {
        return Err(record.error("dimensions differ from the independently derived kind"));
    }
    active.remove(name);
    kinds.insert(name.to_owned(), kind.clone());
    Ok(kind)
}

/// Look up a kind by explicit identity, never by its dimension vector.
pub(super) fn lookup(kinds: &Kinds, record: &Record<'_>) -> Result<Kind, Error> {
    kinds
        .get(record.text()?)
        .cloned()
        .ok_or_else(|| record.error("unknown kind name"))
}

/// Validate the complete intended descriptor, including unavailable relative descriptors.
pub(super) fn parts(record: &Record<'_>, kinds: &Kinds) -> Result<(Kind, Measure, Role), Error> {
    record.keys(&["dimensions", "kind", "measure", "affine_role"], &[])?;
    let kind = lookup(kinds, &record.at("kind")?)?;
    if dimension(&record.at("dimensions")?)?
        != kind.dimension().map_err(|error| record.error(error))?
    {
        return Err(record.error("descriptor dimensions differ from its declared kind"));
    }
    let measure = record.at("measure")?;
    measure.keys(&["display", "scale", "offset"], &[])?;
    let measure = Measure::new(
        measure.string("display")?,
        Scale::Exact(measure.at("scale")?.rational()?),
        measure.at("offset")?.rational()?,
    )
    .map_err(|error| measure.error(error))?;
    let role = match record.string("affine_role")? {
        "point" => Role::Point,
        "difference" => Role::Difference,
        _ => return Err(record.error("unknown affine role")),
    };
    Ok((kind, measure, role))
}

/// Construct an ordinary descriptor through the units authority.
pub(super) fn descriptor(record: &Record<'_>, kinds: &Kinds) -> Result<Descriptor, Error> {
    let (kind, measure, role) = parts(record, kinds)?;
    Descriptor::new(kind, measure, role).map_err(|error| record.error(error))
}

/// Serialize public descriptor evidence using a source-declared kind identity.
pub(super) fn evidence(
    value: &Descriptor,
    kinds: &Kinds,
    record: &Record<'_>,
) -> Result<Value, Error> {
    let name = kinds
        .iter()
        .find(|(_, kind)| *kind == value.kind())
        .map(|(name, _)| name)
        .ok_or_else(|| record.error("checked kind has no declared identity"))?;
    Ok(json!({
        "dimensions": value.dimension().exponents().iter().map(|value| exact(value, record)).collect::<Result<Vec<_>, _>>()?,
        "kind": name,
        "measure": {
            "display": value.measure().name(),
            "scale": exact(value.measure().scale(), record)?,
            "offset": exact(value.measure().offset(), record)?,
        },
        "affine_role": if value.role() == Role::Point { "point" } else { "difference" },
    }))
}
