// SPDX-License-Identifier: AGPL-3.0-only
//! Shared summary corpus: semantic expectations come from fixture roles, not codecs.

use std::{fs, path::Path};

/// Both independent structural decoders consume these exact committed summary fixtures.
#[test]
fn shared_summary_acceptance_and_refusal_corpus() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/fixtures/compute");
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("invalid fixture name")?;
        let expected = name.starts_with("valid-");
        let bytes = fs::read(&path)?;
        let decoded = nrz_compute::read_summary(bytes.as_slice(), u64::try_from(bytes.len())?);
        assert_eq!(decoded.is_ok(), expected, "{name}: {decoded:?}");
    }
    Ok(())
}
