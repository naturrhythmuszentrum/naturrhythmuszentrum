// SPDX-License-Identifier: AGPL-3.0-only
//! Fallible payload reservations and cooperative analysis checkpoints.

use super::{AnalysisError, AnalysisPhase, CapacityKind};

/// One caller-owned cancellation hook, with no hidden clock or global state.
pub(super) struct Control<'a> {
    /// Returning true stops admission of ordinary analysis.
    cancelled: &'a mut dyn FnMut(AnalysisPhase) -> bool,
}

impl<'a> Control<'a> {
    /// Borrow the caller's checkpoint policy.
    pub(super) const fn new(cancelled: &'a mut dyn FnMut(AnalysisPhase) -> bool) -> Self {
        Self { cancelled }
    }

    /// A checkpoint bounds the following fixed-size arithmetic or loop step.
    pub(super) fn check(&mut self, phase: AnalysisPhase) -> Result<(), AnalysisError> {
        if (self.cancelled)(phase) {
            Err(AnalysisError::Cancelled { phase })
        } else {
            Ok(())
        }
    }
}

/// Cumulative capacity for buffers that coexist for the whole analysis.
#[derive(Debug)]
pub(super) struct Memory {
    /// Independent channel being accounted.
    kind: CapacityKind,
    /// Caller-supplied maximum payload bytes.
    limit: usize,
    /// Reserved fixed records and actual vector capacities.
    used: usize,
}

impl Memory {
    /// Admit fixed workspace records before reserving dynamic buffers.
    pub(super) fn new(
        kind: CapacityKind,
        limit: usize,
        fixed: usize,
    ) -> Result<Self, AnalysisError> {
        let mut memory = Self {
            kind,
            limit,
            used: 0,
        };
        memory.charge(fixed)?;
        Ok(memory)
    }

    /// Checked accumulation rejects integer overflow and cap exhaustion alike.
    fn charge(&mut self, bytes: usize) -> Result<(), AnalysisError> {
        let total = self
            .used
            .checked_add(bytes)
            .ok_or(AnalysisError::Capacity { kind: self.kind })?;
        if total > self.limit {
            return Err(AnalysisError::Capacity { kind: self.kind });
        }
        self.used = total;
        Ok(())
    }

    /// Reserve capacity once, without initialization loops or infallible growth.
    pub(super) fn reserve<T>(
        &mut self,
        length: usize,
        control: &mut Control<'_>,
    ) -> Result<Vec<T>, AnalysisError> {
        control.check(AnalysisPhase::Allocation)?;
        let bytes = length
            .checked_mul(size_of::<T>())
            .ok_or(AnalysisError::Capacity { kind: self.kind })?;
        self.charge(bytes)?;
        let mut buffer = Vec::new();
        buffer
            .try_reserve_exact(length)
            .map_err(|_| AnalysisError::Allocation { kind: self.kind })?;
        let extra = buffer
            .capacity()
            .checked_sub(length)
            .and_then(|n| n.checked_mul(size_of::<T>()))
            .ok_or(AnalysisError::Capacity { kind: self.kind })?;
        self.charge(extra)?;
        control.check(AnalysisPhase::Allocation)?;
        Ok(buffer)
    }
}

/// A buffer may only fill capacity admitted before analysis; never grow it.
pub(super) fn push<T>(buffer: &mut Vec<T>, value: T) -> Result<(), AnalysisError> {
    if buffer.len() == buffer.capacity() {
        return Err(AnalysisError::InvalidIndex);
    }
    buffer.push(value);
    Ok(())
}
