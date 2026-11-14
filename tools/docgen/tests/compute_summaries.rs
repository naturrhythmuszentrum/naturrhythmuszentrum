// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`shared_compute_summary_corpus`] | docgen | The independent summary decoder accepts and rejects the shared current-schema fixtures. |

use std::{fs, path::Path};

/// The independent summary decoder accepts and rejects the shared current-schema fixtures.
/// ´claim:docgen:shared-compute-summary-corpus´
/// ´test:integration:shared-compute-summary-corpus´
#[test]
fn shared_compute_summary_corpus() -> Result<(), Box<dyn std::error::Error>> {
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
        let result = nrz_docgen::compute::validate_summary_bytes(&fs::read(&path)?);
        assert_eq!(result.is_ok(), expected, "{name}: {result:?}");
    }
    Ok(())
}
