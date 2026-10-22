// SPDX-License-Identifier: AGPL-3.0-only

//! The hand-edited project input and its derived dataset.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::envelope;
use crate::error::{self, Error};

/// Path of the hand-edited project input.
/// The project input path fixes the authored project source in the declared layout (´rule:docgen:input-layout´).
/// ´const:docgen:project-input´ (´[ORCHESTRATION-alg:const:text]´)
/// ´const:docgen:project-input-text-x8d5f8ea5´
const INPUT: &str = "data/inputs/project.toml";
/// Path of the derived project dataset.
/// The project dataset path fixes the generated project artifact consumed by templates (´rule:docgen:dataset-identities´).
/// ´const:docgen:project-dataset´ (´[ORCHESTRATION-alg:const:text]´)
/// ´const:docgen:project-dataset-text-xf75787cf´
pub(crate) const DATASET: &str = "data/datasets/project.json";
/// Namespace of the hand-edited input.
/// The project input namespace distinguishes authored project records from derived datasets (´rule:docgen:dataset-identities´).
/// ´const:docgen:project-input-namespace´ (´[ORCHESTRATION-alg:const:text]´)
/// ´const:docgen:project-input-namespace-text-xe5cb5f8c´
const INPUT_NAMESPACE: &str = "org.naturrhythmuszentrum.docs.input.project";
/// Namespace of the derived project dataset.
/// The project dataset namespace identifies the derived project payload independently of its filename (´rule:docgen:dataset-identities´).
/// ´const:docgen:project-dataset-namespace´ (´[ORCHESTRATION-alg:const:text]´)
/// ´const:docgen:project-dataset-namespace-text-xa1cfe704´
const DATASET_NAMESPACE: &str = "org.naturrhythmuszentrum.docs.dataset.project";

/// One planned implementation bite.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Bite {
    /// Stable bite identifier.
    id: String,
    /// Human-readable bite title.
    title: String,
}

/// Payload common to the input and its derived dataset.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Project {
    /// Project name.
    name: String,
    /// One-sentence description.
    description: String,
    /// Project repository address.
    repository: String,
    /// Planned bites in order.
    bites: Vec<Bite>,
}

/// Derive the deterministic project dataset from its declared input.
pub(crate) fn derive(root: &Path, owners: &BTreeSet<String>) -> Result<(Vec<u8>, Value), Error> {
    let directory = "data/inputs";
    for entry in
        fs::read_dir(root.join(directory)).map_err(|source| error::io(directory, source))?
    {
        let entry = entry.map_err(|source| error::io(directory, source))?;
        if !matches!(
            entry.file_name().to_str(),
            Some("project.toml" | "engineering.toml")
        ) {
            return Err(Error::Payload {
                path: format!("{directory}/{}", entry.file_name().to_string_lossy()),
                detail: "only project.toml and engineering.toml are accepted in data/inputs"
                    .to_owned(),
            });
        }
    }
    let value = envelope::read_toml(root, INPUT)?;
    envelope::validate(&value, INPUT, INPUT_NAMESPACE, owners)?;
    let object = value.as_object().ok_or_else(|| Error::Envelope {
        path: INPUT.to_owned(),
        detail: "input must be an object".to_owned(),
    })?;
    let sources = envelope::string_array(object, INPUT, "sources")?;
    let labels = envelope::string_array(object, INPUT, "labels")?;
    if value.get("owner").and_then(Value::as_str) != Some("NRZ")
        || !sources.is_empty()
        || labels.len() != 1
        || labels[0] != "guide:docgen:overview"
    {
        return Err(Error::Envelope {
            path: INPUT.to_owned(),
            detail: "project input owner, sources, or labels differ from its declaration"
                .to_owned(),
        });
    }
    let payload = envelope::payload(&value, INPUT)?;
    let project: Project = serde_json::from_value(payload).map_err(|problem| Error::Payload {
        path: INPUT.to_owned(),
        detail: problem.to_string(),
    })?;
    let serialized = serde_json::to_value(project).map_err(|problem| Error::Payload {
        path: INPUT.to_owned(),
        detail: problem.to_string(),
    })?;
    let fields: BTreeMap<String, Value> = serialized
        .as_object()
        .ok_or_else(|| Error::Payload {
            path: INPUT.to_owned(),
            detail: "project payload must be an object".to_owned(),
        })?
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let dataset_sources = vec![INPUT.to_owned()];
    let bytes = envelope::encode(
        DATASET,
        DATASET_NAMESPACE,
        "NRZ",
        &dataset_sources,
        &labels,
        &fields,
    )?;
    let dataset = serde_json::from_slice(&bytes).map_err(|problem| Error::JsonSyntax {
        path: DATASET.to_owned(),
        detail: problem.to_string(),
    })?;
    Ok((bytes, dataset))
}

/// Validate a committed project dataset before rendering uses it.
pub(crate) fn validate_dataset(value: &Value, owners: &BTreeSet<String>) -> Result<(), Error> {
    envelope::validate(value, DATASET, DATASET_NAMESPACE, owners)?;
    let object = value.as_object().ok_or_else(|| Error::Envelope {
        path: DATASET.to_owned(),
        detail: "dataset must be an object".to_owned(),
    })?;
    let sources = envelope::string_array(object, DATASET, "sources")?;
    let labels = envelope::string_array(object, DATASET, "labels")?;
    if value.get("owner").and_then(Value::as_str) != Some("NRZ")
        || sources.len() != 1
        || sources[0] != INPUT
        || labels.len() != 1
        || labels[0] != "guide:docgen:overview"
    {
        return Err(Error::Envelope {
            path: DATASET.to_owned(),
            detail: "project dataset owner, sources, or labels differ from its declaration"
                .to_owned(),
        });
    }
    let payload = envelope::payload(value, DATASET)?;
    drop(
        serde_json::from_value::<Project>(payload).map_err(|problem| Error::Payload {
            path: DATASET.to_owned(),
            detail: problem.to_string(),
        })?,
    );
    Ok(())
}
