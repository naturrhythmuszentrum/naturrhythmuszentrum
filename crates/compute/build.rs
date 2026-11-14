// SPDX-License-Identifier: AGPL-3.0-only
//! Read-only compiler/source capture with explicit command failures.

use serde_json::{Value, json};
use std::{
    env, fs,
    io::{self},
    path::{Path, PathBuf},
};

mod build_capture;
use build_capture::{build_intent, capture, dependency_versions, evidence, source, variable};

/// Emit fallible runtime construction rather than a decoder with hidden temporary owners.
fn build_expression(value: &Value, field: &str) -> Result<String, Box<dyn std::error::Error>> {
    use std::fmt::Write as _;
    Ok(match value {
        Value::String(text) if field == "intent" => match text.as_str() {
            "Baseline" => "crate::BuildIntent::Baseline".to_owned(),
            "NativeRequested" => "crate::BuildIntent::NativeRequested".to_owned(),
            "ExplicitTarget" => "crate::BuildIntent::ExplicitTarget".to_owned(),
            _ => return Err("invalid generated build intent".into()),
        },
        Value::String(text) => format!("budget.text({text:?})?"),
        Value::Bool(value) => value.to_string(),
        Value::Array(values) => {
            let mut source = format!("{{ let mut values = budget.reserve({})?;", values.len());
            for value in values {
                write!(source, "values.push({});", build_expression(value, field)?)?;
            }
            source.push_str("values }");
            source
        }
        Value::Object(fields) if fields.contains_key("state") => {
            let variant = match fields["state"].as_str() {
                Some("Known") => "Known",
                Some("Unknown") => "Unknown",
                _ => return Err("invalid generated build evidence".into()),
            };
            let nested = if variant == "Known" { field } else { "reason" };
            format!(
                "crate::Evidence::{variant}({})",
                build_expression(&fields["value"], nested)?
            )
        }
        Value::Object(fields) => {
            let kind = match field {
                "build" => "BuildRecord",
                "lowering" => "LoweringRecord",
                _ => return Err("invalid generated build record".into()),
            };
            let mut source = format!("crate::{kind} {{");
            for (name, value) in fields {
                write!(source, "{name}: {},", build_expression(value, name)?)?;
            }
            source.push('}');
            source
        }
        Value::Null | Value::Number(_) => return Err("invalid generated build value".into()),
    })
}

/// Write captured constructor source only beneath Cargo's `OUT_DIR`.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = io::stdout().lock();
    let directory = PathBuf::from(
        variable("CARGO_MANIFEST_DIR", &mut output)?.ok_or("missing manifest directory")?,
    );
    let destination =
        PathBuf::from(variable("OUT_DIR", &mut output)?.ok_or("missing output directory")?);
    build_capture::environment(&mut output)?;
    let compiler = variable("RUSTC", &mut output)?
        .and_then(|compiler| capture(&compiler, &["-vV"], &directory));
    let flags = variable("CARGO_ENCODED_RUSTFLAGS", &mut output)?;
    let intent = evidence(flags.as_deref().map(build_intent));
    let target = variable("TARGET", &mut output)?;
    let profile = variable("PROFILE", &mut output)?;
    let (source_commit, dirty, lockfile) = source(&directory, &mut output)?;
    let dependencies = capture("git", &["rev-parse", "--show-toplevel"], &directory)
        .and_then(|root| dependency_versions(Path::new(&root)));
    let backend = evidence(
        dependencies
            .as_ref()
            .and_then(|versions| versions.iter().find(|entry| entry.starts_with("faer@")))
            .map(|version| format!("{version}; Par::Seq")),
    );
    let dependencies = dependencies.map_or_else(
        || evidence(None),
        |versions| json!({"state":"Known","value":versions}),
    );
    let mut features = vec!["nrz-matrix/benchmark".to_owned()];
    for name in env::vars_os()
        .filter_map(|(name, _)| name.into_string().ok())
        .filter(|name| name.starts_with("CARGO_FEATURE_"))
    {
        let _value = variable(&name, &mut output)?;
        features.push(
            name.trim_start_matches("CARGO_FEATURE_")
                .to_ascii_lowercase(),
        );
    }
    features.sort();
    let build = json!({"compiler":evidence(compiler), "source_commit":source_commit,
        "dirty":dirty, "lockfile":lockfile, "target":evidence(target),
        "profile":evidence(profile), "flags":evidence(flags), "intent":intent, "features":features,
        "backend":backend, "dependencies":dependencies,
        "lowering":{
            "inspected":{"state":"Unknown","value":"no machine-code inspection supplied"},
            "artifact":{"state":"Unknown","value":"no inspected artifact supplied"},
            "symbols":{"state":"Unknown","value":"no inspected symbols supplied"},
            "scalar_vectorization":{"state":"Unknown","value":"not inspected"},
            "splitting":{"state":"Unknown","value":"not inspected"},
            "helpers":{"state":"Unknown","value":"not inspected"},
            "native_fma":{"state":"Unknown","value":"not inspected"},
            "notes":{"state":"Unknown","value":"no inspection notes supplied"}}});
    fs::write(
        destination.join("compute-build.rs"),
        build_expression(&build, "build")?,
    )?;
    Ok(())
}
