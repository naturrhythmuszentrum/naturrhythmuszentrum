// SPDX-License-Identifier: AGPL-3.0-only

//! Refuse schedule codes while preserving mathematical symbols and numeric data.
//!
//! Ordering words and numeric step heads accept only the recorded single-digit
//! plan domain, zero through eight, optionally followed by a letter through h.
//! Longer numbers remain numerical algorithm positions, such as the Wilkinson
//! elimination index. Spelled numbers remain ordinary English, not schedule codes.
//! CAS slice markers are uppercase or underscore prefixes; bare LC markers
//! require ordering prose or a list/title head. Mathematical products in cubic
//! denominators and lowercase source symbols remain outside these patterns.
//! No filename grants an exception.

/// Locate the first schedule-code span in one line.
pub(crate) fn find(line: &str) -> Option<&str> {
    let tokens = words(line);
    for (index, &(start, word)) in tokens.iter().enumerate() {
        if ordering_word(word)
            && let Some(&(offset, next)) = tokens.get(index + 1)
            && ordinal(next)
            && code_separator(line, start + word.len(), offset, next)
            && !decimal_tail(line, offset + next.len())
        {
            return line.get(start..range_end(line, &tokens, index + 1, offset));
        }
        if slice_code(line, start, word) || compact_code(word) {
            return line.get(start..start + word.len());
        }
        if lc_code(word) && ordering_context(line, &tokens, index) {
            return line.get(start..range_end(line, &tokens, index, start));
        }
    }
    None
}

/// Separate schedule words from code punctuation and ordinary enumerations.
fn code_separator(line: &str, end: usize, start: usize, next: &str) -> bool {
    let separator = line.get(end..start).unwrap_or_default();
    let numeric_head =
        separator.trim() == "(" && next.as_bytes().first().is_some_and(u8::is_ascii_digit);
    let ordinary = !separator.is_empty()
        && separator
            .chars()
            .all(|ch| ch.is_whitespace() || matches!(ch, '-' | '_' | '–' | '—' | '`'));
    numeric_head || ordinary
}

/// A decimal fraction is a measured number, not a schedule position.
fn decimal_tail(line: &str, end: usize) -> bool {
    let bytes = line.as_bytes();
    let dot = bytes.get(end) == Some(&b'.');
    let digit = bytes.get(end + 1).is_some_and(u8::is_ascii_digit);
    dot && digit
}

/// Retain byte offsets of ASCII words without consuming Unicode separators.
fn words(line: &str) -> Vec<(usize, &str)> {
    let mut result = Vec::new();
    let mut start = None;
    for (index, byte) in line.bytes().enumerate() {
        if byte.is_ascii_alphanumeric() {
            start.get_or_insert(index);
        } else if let Some(offset) = start.take()
            && let Some(word) = line.get(offset..index)
        {
            result.push((offset, word));
        }
    }
    if let Some(offset) = start
        && let Some(word) = line.get(offset..)
    {
        result.push((offset, word));
    }
    result
}

/// Recognize only scheduling vocabulary, never an arbitrary letter-digit pair.
fn ordering_word(word: &str) -> bool {
    matches!(
        word.to_ascii_lowercase().as_str(),
        "stage"
            | "stages"
            | "bite"
            | "bites"
            | "step"
            | "steps"
            | "wave"
            | "waves"
            | "phase"
            | "phases"
            | "tranche"
            | "tranches"
            | "round"
            | "rounds"
    )
}

/// Admit the historical single-digit numeric plan domain.
fn ordinal(word: &str) -> bool {
    let bytes = word.as_bytes();
    let digit = matches!(bytes.first(), Some(b'0'..=b'8'));
    let suffix = bytes.len() == 2 && matches!(bytes.get(1), Some(b'a'..=b'h' | b'A'..=b'H'));
    digit && (bytes.len() == 1 || suffix)
}

/// Recognize a joined ordering word and ordinal inside an identifier.
fn compact_code(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    ["stage", "bite", "step", "wave", "phase", "tranche", "round"]
        .iter()
        .any(|prefix| lower.strip_prefix(prefix).is_some_and(ordinal))
}

/// Recognize CAS slice words and lowercase underscore-prefixed identifiers.
fn slice_code(line: &str, start: usize, word: &str) -> bool {
    let bytes = word.as_bytes();
    if bytes.len() != 2 || !matches!(bytes.get(1), Some(b'0'..=b'4')) {
        return false;
    }
    let uppercase = bytes.first() == Some(&b'S');
    let underscore = line.as_bytes().get(start + word.len()) == Some(&b'_');
    let identifier = bytes.first() == Some(&b's') && underscore;
    uppercase || identifier
}

/// Recognize only the LC substep family, not coefficients or exponents generally.
fn lc_code(word: &str) -> bool {
    let bytes = word.as_bytes();
    bytes.len() == 2 && bytes.first() == Some(&b'3') && matches!(bytes.get(1), Some(b'a'..=b'h'))
}

/// Require prose ordering or the leading position of a list/title.
fn ordering_context(line: &str, words: &[(usize, &str)], index: usize) -> bool {
    if let Some(previous) = index.checked_sub(1).and_then(|i| words.get(i)) {
        let Some(&(start, word)) = words.get(index) else {
            return false;
        };
        let connected = code_separator(line, previous.0 + previous.1.len(), start, word);
        let context = matches!(
            previous.1.to_ascii_lowercase().as_str(),
            "after" | "before" | "then" | "in" | "on" | "through" | "until" | "from" | "and" | "or"
        ) || ordering_word(previous.1);
        return connected && context;
    }
    let Some(&(start, word)) = words.get(index) else {
        return false;
    };
    let prefix = line.get(..start).unwrap_or_default().trim();
    let suffix = line
        .get(start + word.len()..)
        .unwrap_or_default()
        .trim_start();
    let list = matches!(prefix, "-" | "*" | "|");
    let title = prefix.ends_with('(') && suffix.starts_with(':');
    list || title
}

/// Include a directly connected range endpoint in the refusal span.
fn range_end(line: &str, words: &[(usize, &str)], index: usize, start: usize) -> usize {
    let Some(&(_, word)) = words.get(index) else {
        return start;
    };
    let end = start + word.len();
    if let Some(&(next, endpoint)) = words.get(index + 1) {
        let separator = line.get(end..next).unwrap_or_default().trim();
        let range = matches!(separator, "-" | "–" | "—" | ".." | "..=");
        if range && ordinal(endpoint) {
            return next + endpoint.len();
        }
    }
    end
}

#[cfg(test)]
mod tests;
