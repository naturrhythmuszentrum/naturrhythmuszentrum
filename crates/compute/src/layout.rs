// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded worker-owned storage evidence for ``def:nrz:compute-design-case-identity``.

use crate::recipe::RecipeError;
use serde::{Deserialize, Serialize};

/// Numerical storage role; no address or allocator identifier is exported.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum BufferRole {
    /// Arithmetic seed or coefficient state.
    State,
    /// Immutable numerical input.
    Input,
    /// Mutable consumed numerical output.
    Output,
    /// Shared prepared numerical values.
    Prepared,
    /// Explicit integer inverse-query cycles.
    Cycles,
    /// Backend index permutation storage.
    Permutation,
    /// Backend-declared byte scratch storage.
    Scratch,
}

/// Actual storage extent and address residue captured outside timing.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BufferLayout {
    /// Stable role in the selected kernel.
    pub role: BufferRole,
    /// Actual allocated payload capacity, not just the logical extent.
    pub capacity_bytes: u64,
    /// Live logical payload bytes.
    pub live_bytes: u64,
    /// Base address modulo 64, without exposing the address.
    pub address_modulo_64: u64,
    /// Logical row stride in bytes.
    pub row_stride_bytes: u64,
    /// Logical column stride in bytes, zero for a one-dimensional buffer.
    pub column_stride_bytes: u64,
}

/// Page residence cannot be obtained from the safe standard library.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum NumaEvidence {
    /// The safe runtime has no physical page-residence query.
    UnavailableSafePageQuery,
}

/// Bounded observed affinity allowance; a mask never claims an exact binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", content = "value", deny_unknown_fields)]
pub enum Allowance {
    /// Least-significant word first; 32 words cover processor/node indices 0 through 1023.
    Known([u32; 32]),
    /// The bounded thread-local procfs probe was absent, denied or malformed.
    UnavailableProbe,
    /// The kernel mask exceeded the explicit representational extent.
    UnsupportedRange,
}

/// Startup evidence is per worker and never claims execution remained on one CPU.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerLayout {
    /// Worker index; zero is the direct caller for serial execution.
    pub worker: u64,
    /// Sum of actual numerical buffer capacities, including scratch and prepared storage.
    pub numerical_capacity_bytes: u64,
    /// Sum of live logical numerical payloads in those same buffers.
    pub numerical_live_bytes: u64,
    /// Worker that allocated and initialized the numerical payload.
    pub first_touch_worker: u64,
    /// Snapshot of the CPU reported by procfs, absent if the bounded probe failed.
    pub startup_cpu: Option<u64>,
    /// Actual worker CPU allowance, distinct from the requested process declaration.
    pub allowed_cpus: Allowance,
    /// Actual worker NUMA-node allowance; physical pages remain separately unavailable.
    pub allowed_nodes: Allowance,
    /// Explicit inability to infer physical page residence from first touch.
    pub numa: NumaEvidence,
    /// Ordered fixed storage slots; unused slots are null.
    pub buffers: [Option<BufferLayout>; 8],
}

/// Capture actual vector payload without allocating any metadata.
pub(crate) fn buffer<T>(
    values: &Vec<T>,
    role: BufferRole,
    rows: usize,
) -> Result<Option<BufferLayout>, RecipeError> {
    if values.capacity() == 0 {
        return Ok(None);
    }
    let bytes = |n: usize| {
        n.checked_mul(size_of::<T>())
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(RecipeError::Overflow)
    };
    Ok(Some(BufferLayout {
        role,
        capacity_bytes: bytes(values.capacity())?,
        live_bytes: bytes(values.len())?,
        address_modulo_64: u64::try_from(values.as_ptr().addr() % 64)
            .map_err(|_| RecipeError::Overflow)?,
        row_stride_bytes: bytes(1)?,
        column_stride_bytes: if rows == 0 { 0 } else { bytes(rows)? },
    }))
}

/// Proc stat's command may contain spaces and parentheses; processor is field 39.
fn processor(text: &str) -> Option<u64> {
    let (_, fields) = text.rsplit_once(')')?;
    fields.split_whitespace().nth(36)?.parse().ok()
}

/// Only a CPU number is retained from the bounded private procfs record.
pub(crate) fn startup_cpu() -> Option<u64> {
    let mut file = std::fs::File::open("/proc/thread-self/stat").ok()?;
    let text = crate::capture::bounded_read(&mut file, 4096).ok()?;
    processor(&text)
}

/// Parse only public hexadecimal masks, with no thread names or identifiers retained.
fn mask(text: &str, key: &str) -> Allowance {
    let Some(value) = text.lines().find_map(|line| line.strip_prefix(key)) else {
        return Allowance::UnavailableProbe;
    };
    let mut mask = [0_u32; 32];
    for (index, word) in value.trim().rsplit(',').enumerate() {
        let Some(slot) = mask.get_mut(index) else {
            return Allowance::UnsupportedRange;
        };
        let Ok(value) = u32::from_str_radix(word, 16) else {
            return Allowance::UnavailableProbe;
        };
        *slot = value;
    }
    Allowance::Known(mask)
}

/// Startup allowances are read on the owning thread before numerical timing.
pub(crate) fn allowances() -> (Allowance, Allowance) {
    let read = || {
        let mut file = std::fs::File::open("/proc/thread-self/status").ok()?;
        crate::capture::bounded_read(&mut file, 4096).ok()
    };
    read().map_or(
        (Allowance::UnavailableProbe, Allowance::UnavailableProbe),
        |text| (mask(&text, "Cpus_allowed:"), mask(&text, "Mems_allowed:")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_capacity_and_stride_are_distinct_from_live_extent() -> Result<(), RecipeError> {
        let mut values = Vec::<f64>::with_capacity(10);
        values.extend_from_slice(&[1.0; 6]);
        let record = buffer(&values, BufferRole::Input, 2)?.ok_or(RecipeError::Domain)?;
        assert_eq!(record.live_bytes, 48);
        assert_eq!(
            record.capacity_bytes,
            u64::try_from(values.capacity()).map_err(|_| RecipeError::Overflow)? * 8
        );
        assert_eq!(record.column_stride_bytes, 16);
        assert!(record.address_modulo_64 < 64);
        Ok(())
    }
    #[test]
    fn allowances_keep_word_order_and_refuse_unrepresentable_ranges() {
        let mut expected = [0; 32];
        expected[0] = 1;
        expected[1] = 2;
        assert_eq!(
            mask("Cpus_allowed:\t00000002,00000001", "Cpus_allowed:"),
            Allowance::Known(expected)
        );
        assert_eq!(
            mask("Cpus_allowed: xyz", "Cpus_allowed:"),
            Allowance::UnavailableProbe
        );
        let oversized = ["00000001"; 33].join(",");
        assert_eq!(
            mask(&format!("Cpus_allowed: {oversized}"), "Cpus_allowed:"),
            Allowance::UnsupportedRange
        );
    }
    #[test]
    fn processor_ignores_private_command_text() {
        let fields = (0..37).map(|n| n.to_string()).collect::<Vec<_>>().join(" ");
        assert_eq!(
            processor(&format!("1 (private (command)) {fields}")),
            Some(36)
        );
        assert_eq!(processor("truncated"), None);
    }
}
