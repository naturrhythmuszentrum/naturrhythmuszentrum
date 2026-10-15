// SPDX-License-Identifier: AGPL-3.0-only
//! Complete ordered partitions and bounded common refinement.

use super::{ShapeError, ShapeLimits};

/// Strictly increasing boundaries covering one axis; an empty axis has `[0]`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Partition {
    /// Validated boundaries including both endpoints.
    boundaries: Vec<usize>,
}

impl Partition {
    /// Validate complete nonempty segments without guessing an omitted endpoint.
    ///
    /// # Errors
    /// Refuses missing endpoints, repeated boundaries or exhausted limits.
    pub fn new(
        length: usize,
        boundaries: Vec<usize>,
        limits: ShapeLimits,
    ) -> Result<Self, ShapeError> {
        if length > limits.axis || boundaries.len().saturating_sub(1) > limits.blocks {
            return Err(ShapeError::Limit);
        }
        if boundaries.first() != Some(&0)
            || boundaries.last() != Some(&length)
            || boundaries.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ShapeError::Partition);
        }
        Ok(Self { boundaries })
    }

    /// Construct the single-segment partition, or `[0]` for an empty axis.
    ///
    /// # Errors
    /// Refuses an axis or segment count exceeding the supplied limits.
    pub fn whole(length: usize, limits: ShapeLimits) -> Result<Self, ShapeError> {
        let boundaries = if length == 0 {
            vec![0]
        } else {
            vec![0, length]
        };
        Self::new(length, boundaries, limits)
    }

    /// Complete endpoints, including zero and the axis length.
    #[must_use]
    pub fn boundaries(&self) -> &[usize] {
        &self.boundaries
    }

    /// Form the common refinement without losing any declared boundary.
    ///
    /// # Errors
    /// Refuses unequal lengths or a refinement exceeding the block limit.
    pub fn refine(&self, other: &Self, limits: ShapeLimits) -> Result<Self, ShapeError> {
        if self.boundaries.last() != other.boundaries.last() {
            return Err(ShapeError::Partition);
        }
        let mut boundaries = self.boundaries.clone();
        boundaries.extend_from_slice(&other.boundaries);
        boundaries.sort_unstable();
        boundaries.dedup();
        Self::new(
            *boundaries.last().ok_or(ShapeError::Partition)?,
            boundaries,
            limits,
        )
    }

    /// Require a nonempty block range to respect declared partition boundaries.
    pub(crate) fn contains_range(&self, range: &std::ops::Range<usize>) -> bool {
        range.start < range.end
            && self.boundaries.contains(&range.start)
            && self.boundaries.contains(&range.end)
    }
}
