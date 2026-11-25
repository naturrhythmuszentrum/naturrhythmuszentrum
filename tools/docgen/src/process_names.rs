// SPDX-License-Identifier: AGPL-3.0-only

//! Recognize narrow identifiers for the working process in public text.
//!
//! The patterns require a scheduling noun joined to a digit-bearing identifier,
//! an explicitly numbered review or diagnostic identifier, or a relative sweep
//! pointer. Generic product nouns and ordinary numbers carry no such meaning.

/// Return the first process-name spelling on a line, preserving its case.
pub(crate) fn find(line: &str) -> Option<&str> {
    for (start, _) in line.char_indices() {
        let before = line[..start].chars().next_back();
        if before.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
            continue;
        }
        if before == Some('-')
            && line[..start - 1]
                .chars()
                .next_back()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            continue;
        }
        let rest = &line[start..];
        if let Some(matched) = joined_identifier(rest)
            .or_else(|| numbered_record(rest))
            .or_else(|| numbered_diagnostic(rest))
            .or_else(|| relative_sweep(rest))
        {
            return Some(matched);
        }
    }
    None
}

/// A joined scheduling noun plus a digit in its first segment names an execution.
fn joined_identifier(rest: &str) -> Option<&str> {
    for prefix in [
        "lane-",
        "lane_",
        "run-",
        "run_",
        "round-",
        "round_",
        "step-",
        "step_",
        "gate-",
        "gate_",
        "candidate-",
        "candidate_",
    ] {
        if let Some(tail) = strip_ascii_prefix(rest, prefix) {
            let length = tail
                .bytes()
                .take_while(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_'))
                .count();
            let identifier = tail.get(..length)?;
            let first_segment = identifier.split(['-', '_']).next()?;
            let has_digit = first_segment.bytes().any(|byte| byte.is_ascii_digit());
            if has_digit && !product_shaped_tail(prefix, first_segment, identifier) {
                return rest.get(..prefix.len() + length);
            }
        }
    }
    None
}

/// Preserve common product indices, coordinate shapes and version tails.
fn product_shaped_tail(prefix: &str, segment: &str, identifier: &str) -> bool {
    if prefix.ends_with('_') {
        let index = segment.len() == 1 && segment.bytes().all(|byte| byte.is_ascii_digit());
        let shape = matches!(segment.to_ascii_lowercase().as_str(), "2d" | "3d" | "2x2");
        if index || shape {
            return true;
        }
    }
    prefix.ends_with('-')
        && identifier == segment
        && segment.strip_prefix('v').is_some_and(|digits| {
            !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
        })
}

/// Numbered finding and diagnostic names identify process records.
fn numbered_record(rest: &str) -> Option<&str> {
    for prefix in [
        "review-finding",
        "review_finding",
        "review finding",
        "diagnostic-run",
        "diagnostic_run",
        "diagnostic run",
    ] {
        if let Some(tail) = strip_ascii_prefix(rest, prefix) {
            let gap = tail
                .bytes()
                .take_while(|byte| matches!(*byte, b' ' | b'-' | b'_' | b'#'))
                .count();
            if gap == 0 {
                continue;
            }
            let suffix = tail.get(gap..)?;
            let length = suffix
                .bytes()
                .take_while(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_'))
                .count();
            let identifier = suffix.get(..length)?;
            if identifier.bytes().any(|byte| byte.is_ascii_digit()) {
                return rest.get(..prefix.len() + gap + length);
            }
        }
    }
    None
}

/// A numbered diagnostic reference or patch basename identifies a process run.
fn numbered_diagnostic(rest: &str) -> Option<&str> {
    for prefix in ["diagnostic ", "diagnostics ", "diag-", "diag_"] {
        if let Some(tail) = strip_ascii_prefix(rest, prefix) {
            let digits = tail.bytes().take_while(u8::is_ascii_digit).count();
            if digits == 0 {
                continue;
            }
            let mut length = prefix.len() + digits;
            if rest.as_bytes().get(length) == Some(&b'.')
                && rest
                    .as_bytes()
                    .get(length + 1)
                    .is_some_and(u8::is_ascii_digit)
            {
                continue;
            }
            let remainder = rest.get(length..)?;
            if let Some(separator @ ('-' | '–')) = remainder.chars().next() {
                let range = &remainder[separator.len_utf8()..];
                let next_digits = range.bytes().take_while(u8::is_ascii_digit).count();
                if next_digits > 0 {
                    length += separator.len_utf8() + next_digits;
                }
            }
            if rest[length..]
                .chars()
                .next()
                .is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_')
            {
                return rest.get(..length);
            }
        }
    }
    None
}

/// A relative sweep pointer cannot identify enduring evidence.
fn relative_sweep(rest: &str) -> Option<&str> {
    let tail = strip_ascii_prefix(rest, "previous")?;
    let separator = tail.chars().next()?;
    if !matches!(separator, ' ' | '-') {
        return None;
    }
    let suffix_text = tail.get(separator.len_utf8()..)?;
    let suffix = "sweep";
    if strip_ascii_prefix(suffix_text, suffix).is_some() {
        let length = "previous".len() + separator.len_utf8() + suffix.len();
        if rest[length..]
            .chars()
            .next()
            .is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_')
        {
            return rest.get(..length);
        }
    }
    None
}

/// Compare ASCII vocabulary without normalizing the returned source span.
fn strip_ascii_prefix<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

#[cfg(test)]
mod tests;
