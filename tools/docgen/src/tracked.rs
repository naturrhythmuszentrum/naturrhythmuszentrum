// SPDX-License-Identifier: AGPL-3.0-only

//! Inspect current bytes of Git-tracked text without traversing untracked files.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::error::{self, Error};
use crate::ordinals;

/// Refuse the first ordinal code in sorted tracked paths or UTF-8 text lines.
pub(crate) fn check(root: &Path) -> Result<(), Error> {
    for path in paths(root)? {
        check_path(root, &path)?;
    }
    Ok(())
}

/// Obtain the index inventory with NUL separation, including staged additions.
fn paths(root: &Path) -> Result<Vec<String>, Error> {
    let output = Command::new("git")
        .args(["ls-files", "--cached", "--full-name", "-z", "--"])
        .current_dir(root)
        .output()
        .map_err(|source| error::io(".", source))?;
    if !output.status.success() {
        return Err(Error::TrackedText {
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    let listing = String::from_utf8(output.stdout).map_err(|problem| Error::TrackedText {
        detail: problem.to_string(),
    })?;
    let mut paths: Vec<_> = listing.split_terminator('\0').map(str::to_owned).collect();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// Scan file names and regular UTF-8 text; binary data has no prose lines.
fn check_path(root: &Path, path: &str) -> Result<(), Error> {
    if let Some(matched) = ordinals::find(path) {
        return Err(refusal(path, 0, matched));
    }
    let full = root.join(path);
    let metadata = match fs::symlink_metadata(&full) {
        Ok(metadata) => metadata,
        Err(problem) if problem.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(error::io(path, source)),
    };
    if !metadata.is_file() {
        return Ok(());
    }
    let bytes = fs::read(&full).map_err(|source| error::io(path, source))?;
    if bytes.contains(&0) {
        return Ok(());
    }
    if let Ok(source) = std::str::from_utf8(&bytes) {
        for (line, text) in source.lines().enumerate() {
            if let Some(matched) = ordinals::find(text) {
                return Err(refusal(path, line + 1, matched));
            }
        }
    }
    Ok(())
}

/// Keep the exact matched spelling and one-based source location in the refusal.
fn refusal(path: &str, line: usize, matched: &str) -> Error {
    Error::OrdinalCode {
        path: path.to_owned(),
        line,
        matched: matched.to_owned(),
    }
}
