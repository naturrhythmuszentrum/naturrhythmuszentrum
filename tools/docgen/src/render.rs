// SPDX-License-Identifier: AGPL-3.0-only

//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`refuses_requirement_mismatch`] | docgen | Reject a declared dataset that differs from the manifest row. |
//! | [`refuses_wrong_dataset_major`] | docgen | Reject a template major that differs from the dataset's major. |

//! Strict rendering of manifest templates.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use minijinja::{AutoEscape, Environment, UndefinedBehavior};
use serde_json::{Map, Value};

use crate::error::{self, Error};
use crate::manifest::{Document, template_path};

/// Render all manifest documents over their declared whole datasets.
pub(crate) fn all(
    root: &Path,
    documents: &[Document],
    datasets: &BTreeMap<String, Value>,
) -> Result<BTreeMap<String, Vec<u8>>, Error> {
    let mut environment = Environment::new();
    environment.set_keep_trailing_newline(true);
    environment.set_undefined_behavior(UndefinedBehavior::Strict);
    environment.set_auto_escape_callback(|_| AutoEscape::None);
    for document in documents {
        let template_path = template_path(&document.output);
        let bytes = fs::read(root.join(&template_path))
            .map_err(|source| error::io(&template_path, source))?;
        let source = String::from_utf8(bytes).map_err(|problem| Error::Template {
            path: template_path.clone(),
            detail: problem.to_string(),
        })?;
        validate_header(document, &source, datasets)?;
        environment
            .add_template_owned(template_path.clone(), source)
            .map_err(|problem| Error::Template {
                path: template_path,
                detail: problem.to_string(),
            })?;
    }
    let mut output = BTreeMap::new();
    for document in documents {
        let template_path = template_path(&document.output);
        let mut context = Map::new();
        for name in &document.requires {
            let value = datasets
                .get(name)
                .ok_or_else(|| Error::DatasetCompatibility {
                    document: document.output.clone(),
                    dataset: name.clone(),
                    detail: "required dataset is absent".to_owned(),
                })?;
            context.extend([(name.clone(), value.clone())]);
        }
        let template = environment
            .get_template(&template_path)
            .map_err(|problem| Error::Template {
                path: template_path.clone(),
                detail: problem.to_string(),
            })?;
        let rendered = template
            .render(context)
            .map_err(|problem| Error::Template {
                path: template_path,
                detail: problem.to_string(),
            })?;
        output.extend([(document.output.clone(), rendered.into_bytes())]);
    }
    Ok(output)
}

/// Validate the exact line-oriented header and its dataset versions.
fn validate_header(
    document: &Document,
    source: &str,
    datasets: &BTreeMap<String, Value>,
) -> Result<(), Error> {
    let template_path = template_path(&document.output);
    let mut lines = source.lines();
    if lines.next() != Some("{# SPDX-License-Identifier: CC0-1.0 -#}") {
        return Err(header(
            &template_path,
            "first line must be the CC0 SPDX header",
        ));
    }
    let mut seen = Vec::new();
    let mut first_body = None;
    for line in &mut lines {
        if line.starts_with("{# requires") {
            let declaration = line
                .strip_prefix("{# requires ")
                .and_then(|rest| rest.strip_suffix(" -#}"))
                .ok_or_else(|| header(&template_path, "malformed requires declaration"))?;
            let (name, version) = declaration.split_once(" v").ok_or_else(|| {
                header(&template_path, "requires declaration lacks a major version")
            })?;
            if !valid_name(name)
                || version.is_empty()
                || !version.bytes().all(|byte| byte.is_ascii_digit())
            {
                return Err(header(
                    &template_path,
                    "invalid dataset name or major version",
                ));
            }
            let major = version
                .parse::<u32>()
                .map_err(|problem| header(&template_path, &problem.to_string()))?;
            seen.push((name.to_owned(), major));
        } else {
            first_body = Some(line);
            break;
        }
    }
    if seen.len() != document.requires.len() {
        return Err(header(
            &template_path,
            &format!(
                "requires declarations differ from manifest datasets {:?}",
                document.requires
            ),
        ));
    }
    for ((name, major), required) in seen.iter().zip(&document.requires) {
        if name != required {
            return Err(header(
                &template_path,
                &format!("dataset {name} differs from manifest dataset {required}"),
            ));
        }
        let dataset = datasets
            .get(name)
            .ok_or_else(|| Error::DatasetCompatibility {
                document: document.output.clone(),
                dataset: name.clone(),
                detail: "required dataset is absent".to_owned(),
            })?;
        let actual = dataset
            .get("version")
            .and_then(Value::as_array)
            .and_then(|parts| parts.first())
            .and_then(Value::as_u64)
            .ok_or_else(|| Error::DatasetCompatibility {
                document: document.output.clone(),
                dataset: name.clone(),
                detail: "dataset version is missing".to_owned(),
            })?;
        if u64::from(*major) != actual {
            return Err(Error::DatasetCompatibility {
                document: document.output.clone(),
                dataset: name.clone(),
                detail: format!(
                    "template {template_path} requires v{major}, dataset has v{actual}",
                ),
            });
        }
    }
    if first_body != Some("<!-- SPDX-License-Identifier: CC0-1.0 -->") || lines.next() != Some("") {
        return Err(header(
            &template_path,
            "body must begin with the CC0 SPDX comment and a blank line",
        ));
    }
    Ok(())
}

/// Recognize a lower-case dataset name with optional digits or hyphens.
fn valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    matches!(bytes.next(), Some(first) if first.is_ascii_lowercase())
        && bytes.all(|part| part.is_ascii_lowercase() || part.is_ascii_digit() || part == b'-')
}

/// Construct a template-header error.
fn header(path: &str, detail: &str) -> Error {
    Error::TemplateHeader {
        path: path.to_owned(),
        detail: detail.to_owned(),
    }
}

/// Boundary tests for the template dependency header.
#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::validate_header;
    use crate::error::Error;
    use crate::manifest::Document;

    /// Reject a declared dataset that differs from the manifest row.
    /// ´claim:docgen:refuses-requirement-mismatch´
    /// ´test:unit:refuses-requirement-mismatch´
    #[test]
    fn refuses_requirement_mismatch() {
        let document = Document {
            output: "docs/design/example.md".to_owned(),
            requires: vec!["project".to_owned()],
        };
        let datasets = BTreeMap::from([("project".to_owned(), json!({ "version": [1, 0, 0] }))]);
        let source = "{# SPDX-License-Identifier: CC0-1.0 -#}\n{# requires census v1 -#}\n<!-- SPDX-License-Identifier: CC0-1.0 -->\n\n# Example\n";
        assert!(matches!(
            validate_header(&document, source, &datasets),
            Err(Error::TemplateHeader { .. })
        ));
    }

    /// Reject a template major that differs from the dataset's major.
    /// ´claim:docgen:refuses-wrong-dataset-major´
    /// ´test:unit:refuses-wrong-dataset-major´
    #[test]
    fn refuses_wrong_dataset_major() {
        let document = Document {
            output: "docs/design/example.md".to_owned(),
            requires: vec!["project".to_owned()],
        };
        let datasets = BTreeMap::from([("project".to_owned(), json!({ "version": [1, 0, 0] }))]);
        let source = "{# SPDX-License-Identifier: CC0-1.0 -#}\n{# requires project v2 -#}\n<!-- SPDX-License-Identifier: CC0-1.0 -->\n\n# Example\n";
        assert!(matches!(
            validate_header(&document, source, &datasets),
            Err(Error::DatasetCompatibility { .. })
        ));
    }
}
