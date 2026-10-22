// SPDX-License-Identifier: AGPL-3.0-only
//! Source currency, table cells and dated observations have explicit refusal rules.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::{invalid, record};
use crate::{Error, envelope, error, manifest};

/// This initial word fixes the byte recurrence (´rule:docgen:source-review-fingerprint´).
/// ´const:docgen:source-fingerprint-offset´ (´[ORCHESTRATION-alg:const:count]´)
/// ´const:docgen:source-fingerprint-offset-count-14695981039346656037´
const OFFSET: u64 = 14_695_981_039_346_656_037;
/// This multiplier fixes the byte recurrence (´rule:docgen:source-review-fingerprint´).
/// ´const:docgen:source-fingerprint-multiplier´ (´[ORCHESTRATION-alg:const:count]´)
/// ´const:docgen:source-fingerprint-multiplier-count-1099511628211´
const MULTIPLIER: u64 = 1_099_511_628_211;

/// Fingerprint source bytes for review currency, without asserting authenticity.
pub(super) fn fingerprint(bytes: &[u8]) -> String {
    let value = bytes.iter().fold(OFFSET, |state, byte| {
        (state ^ u64::from(*byte)).wrapping_mul(MULTIPLIER)
    });
    format!("{value:016x}")
}

/// Validate records and impose deterministic ordering within each table.
pub(super) fn records(root: &Path, records: &mut record::Records) -> Result<(), Error> {
    records.source_versions.sort_by(|a, b| a.path.cmp(&b.path));
    records.kernels.sort_by(|a, b| a.id.cmp(&b.id));
    records.refusals.sort_by(|a, b| {
        (&a.owner, &a.type_name, &a.variant).cmp(&(&b.owner, &b.type_name, &b.variant))
    });
    records.measurements.sort_by(|a, b| a.id.cmp(&b.id));
    let source_paths = reviewed_sources(root, records)?;
    kernels(&records.kernels, &source_paths)?;
    refusals(&records.refusals, &source_paths)?;
    measurements(&records.measurements)
}

/// Every catalogue source is a live regular file at its reviewed byte version.
fn reviewed_sources(root: &Path, records: &record::Records) -> Result<BTreeSet<String>, Error> {
    let mut seen = BTreeSet::new();
    for source in &records.source_versions {
        let code_source = Path::new(&source.path).extension() == Some(std::ffi::OsStr::new("rs"));
        if !envelope::normal_path(&source.path) || !code_source {
            return Err(invalid(
                "catalogue source must be a normal Rust source path",
            ));
        }
        if !seen.insert(source.path.clone()) {
            return Err(invalid("duplicate catalogue source"));
        }
        let guarded = guarded_rows(records, &source.path);
        if guarded.is_empty() {
            return Err(invalid(&format!(
                "source {} guards no reviewed row; remove its source_versions entry from {}",
                source.path,
                super::INPUT
            )));
        }
        manifest::check_symlinks(root, &source.path)?;
        let bytes =
            fs::read(root.join(&source.path)).map_err(|cause| error::io(&source.path, cause))?;
        if fingerprint(&bytes) != source.fingerprint {
            return Err(invalid(&format!(
                "source fingerprint changed: {}; review rows [{}], update their catalogue fields, then refresh this source_versions fingerprint in {}",
                source.path,
                guarded.join(", "),
                super::INPUT
            )));
        }
    }
    Ok(seen)
}

/// Report exactly which authored rows depend on a reviewed source version.
fn guarded_rows(records: &record::Records, path: &str) -> Vec<String> {
    let kernels = records
        .kernels
        .iter()
        .filter(|row| row.source == path)
        .map(|row| format!("kernel:{}", row.id));
    let refusals = records
        .refusals
        .iter()
        .filter(|row| row.source == path)
        .map(|row| format!("refusal:{}:{}::{}", row.owner, row.type_name, row.variant));
    kernels.chain(refusals).collect()
}

/// A cell cannot silently change the Markdown table structure.
fn cell(text: &str) -> Result<(), Error> {
    if text.trim().is_empty() || text.contains(['\n', '\r', '|']) {
        return Err(invalid(
            "table cells must be nonempty single lines without pipe characters",
        ));
    }
    Ok(())
}

/// The catalogue covers only fully adopted foundation owners.
fn owner_source(owner: &str, source: &str, sources: &BTreeSet<String>) -> Result<(), Error> {
    let prefix = match owner {
        "SCALAR" => "crates/scalar/",
        "ACCURACY" => "crates/accuracy/",
        "DOCGEN" => "tools/docgen/",
        _ => return Err(invalid("catalogue owner is outside the adopted wave")),
    };
    if !source.starts_with(prefix) || !sources.contains(source) {
        return Err(invalid(
            "catalogue row requires a reviewed source in its owning package",
        ));
    }
    Ok(())
}

/// Laws cite executable claims without minting a second test inventory.
fn kernels(kernels: &[record::Kernel], sources: &BTreeSet<String>) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    for kernel in kernels {
        if !seen.insert(&kernel.id) {
            return Err(invalid("duplicate kernel identity"));
        }
        owner_source(&kernel.owner, &kernel.source, sources)?;
        for value in [
            &kernel.id,
            &kernel.operation,
            &kernel.law,
            &kernel.domain,
            &kernel.guarantee,
            &kernel.shape,
            &kernel.refusals,
        ] {
            cell(value)?;
        }
        if kernel.claims.is_empty() {
            return Err(invalid("kernel law requires executable claim citations"));
        }
        for claim in &kernel.claims {
            cell(claim)?;
            let prefix = format!("[{}-claim:", kernel.owner);
            if !claim.starts_with(&prefix) || !claim.ends_with(']') {
                return Err(invalid("kernel claim must be qualified by its owner"));
            }
        }
    }
    Ok(())
}

/// Preserve enum identities and distinguish failures from successful stops.
fn refusals(refusals: &[record::Refusal], sources: &BTreeSet<String>) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    for refusal in refusals {
        if !seen.insert((&refusal.owner, &refusal.type_name, &refusal.variant)) {
            return Err(invalid("duplicate refusal variant"));
        }
        owner_source(&refusal.owner, &refusal.source, sources)?;
        for value in [&refusal.type_name, &refusal.variant, &refusal.meaning] {
            cell(value)?;
        }
        if !matches!(
            refusal.category.as_str(),
            "failure" | "resource" | "successful stop" | "refusing stop"
        ) {
            return Err(invalid("unknown refusal category"));
        }
    }
    Ok(())
}

/// Observations retain an exact decimal, a calendar date and a complete tree identity.
fn measurements(rows: &[record::Measurement]) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    for row in rows {
        if !seen.insert(&row.id) {
            return Err(invalid("duplicate measurement identity"));
        }
        for value in [&row.id, &row.metric, &row.unit, &row.command, &row.source] {
            cell(value)?;
        }
        let mut parts = row.value.split('.');
        let integer_valid = digits(parts.next().unwrap_or_default());
        let fraction_valid = parts.next().is_none_or(digits);
        if !integer_valid || !fraction_valid || parts.next().is_some() {
            return Err(invalid("measurement requires an exact nonnegative decimal"));
        }
        if row.commit.len() != 40 || !row.commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(invalid("measurement requires a full commit identifier"));
        }
        date(&row.date)?;
    }
    Ok(())
}

/// Every present decimal component contains only digits.
fn digits(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

/// Refuse impossible dates rather than retaining merely date-shaped strings.
fn date(value: &str) -> Result<(), Error> {
    let parts = value.split('-').collect::<Vec<_>>();
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return Err(invalid("measurement date must be YYYY-MM-DD"));
    }
    if !parts.iter().all(|part| digits(part)) {
        return Err(invalid("measurement date has nondecimal fields"));
    }
    let numbers = parts
        .iter()
        .map(|part| part.parse::<u32>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid("measurement date has nondecimal fields"))?;
    let (year, month, day) = (numbers[0], numbers[1], numbers[2]);
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => return Err(invalid("measurement date has an invalid month")),
    };
    if year == 0 || day == 0 || day > days {
        return Err(invalid("measurement date has an invalid day"));
    }
    Ok(())
}
