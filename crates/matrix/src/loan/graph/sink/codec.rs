// SPDX-License-Identifier: AGPL-3.0-only
//! Closed scalar codecs; native structured refusals move back to the caller intact.

/// Original node and phase identity, independent of a reusable evidence slot.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub(crate) struct Position {
    /// Original graph owner, distinct from this sink's storage identity.
    pub(crate) owner: u64,
    /// Original graph node slot.
    pub(crate) node: u64,
    /// Original nonwrapping node occupation.
    pub(crate) generation: u64,
    /// Original native phase selected at admission.
    pub(crate) phase: u64,
}
/// Every admitted retained field has an intrinsic fixed encoding bound.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u64)]
pub(crate) enum Encoding {
    /// Caller spare storage is not a required outcome destination.
    #[default]
    Vacant,
    /// Closed terminal disposition plus original position.
    Outcome,
    /// Closed native-return tag plus original position; no native payload copy.
    Refusal,
    /// Exact original occupation and original capacity result.
    Probe,
    /// Original position with no probe outcome.
    Exhaustion,
}
impl Encoding {
    /// Fixed bytes include the one-byte closed discriminant and all scalar words.
    pub(crate) const fn bytes(self) -> usize {
        match self {
            Self::Vacant => 0,
            Self::Outcome | Self::Refusal => 41,
            Self::Probe => 57,
            Self::Exhaustion => 33,
        }
    }
    /// Native consuming-return classification prepays its maximum four fixed variant inspections.
    /// The original error is not traversed or copied; unused shorter-path visits stay prepaid.
    pub(crate) const fn inspection(self) -> usize {
        if matches!(self, Self::Refusal) { 4 } else { 0 }
    }
    /// Source words visited once by encoding, including each fixed discriminant.
    pub(crate) const fn fields(self) -> usize {
        match self {
            Self::Vacant => 0,
            Self::Outcome | Self::Refusal => 6,
            Self::Probe => 8,
            Self::Exhaustion => 5,
        }
    }
}
/// The three unbounded routes are explicit, never accepted as retained variable fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub(crate) enum Refusal {
    /// Caller owns `UnsupportedMeasureScale`'s intact String.
    UnsupportedMeasureScale,
    /// Caller owns `ConflictingKind`'s intact String.
    ConflictingKind,
    /// Caller owns the original Arc-held series evidence and Rational.
    SeriesEvidence,
    /// Other native loan causes are returned intact, without a copied payload.
    Loan,
    /// Other declaration causes are returned intact, without a copied payload.
    Declaration,
}
/// K's closed terminal vocabulary grants no numerical authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub(crate) enum Outcome {
    /// A native published outcome is separately held by its owner.
    Published,
    /// Declared missing input, not a numerical zero.
    Absent,
    /// Existing cancellation completed.
    Cancelled,
    /// A producer refused; its cause is separately returned.
    DependencyRefused,
}
/// Original walk capacity evidence is preserved without a second classification.
pub(crate) use super::super::walk::Capacity;

/// Only these bounded fields enter the sink; caller capabilities never enter this enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Evidence {
    /// Required terminal disposition.
    Outcome(Outcome),
    /// Structured native error is returned separately, intact.
    Refusal(Refusal),
    /// Required capacity receipt.
    Probe(Capacity),
    /// Required attempt exhaustion receipt, without a fictitious probe.
    Exhaustion,
}
impl Evidence {
    /// Closed schema selection is independent of caller-supplied byte counts.
    pub(crate) const fn encoding(self) -> Encoding {
        match self {
            Self::Outcome(_) => Encoding::Outcome,
            Self::Refusal(_) => Encoding::Refusal,
            Self::Probe(_) => Encoding::Probe,
            Self::Exhaustion => Encoding::Exhaustion,
        }
    }
    /// Encode directly into the reserved destination; there is no temporary encoded copy.
    pub(super) fn write(
        self,
        expected: Encoding,
        position: Position,
        words: &mut [u64],
    ) -> Result<(), Self> {
        let (encoding, tag, scalars) = match self {
            Self::Outcome(value) => (Encoding::Outcome, 1, [value as u64, 0, 0]),
            Self::Refusal(value) => (Encoding::Refusal, 2, [value as u64, 0, 0]),
            Self::Probe(Capacity::Fits) => (Encoding::Probe, 3, [0, 0, 0]),
            Self::Probe(Capacity::Wait {
                required,
                available,
            }) => (Encoding::Probe, 3, [1, required, available]),
            Self::Probe(Capacity::Intrinsic { required, limit }) => {
                (Encoding::Probe, 3, [2, required, limit])
            }
            Self::Exhaustion => (Encoding::Exhaustion, 4, [0, 0, 0]),
        };
        if encoding != expected {
            return Err(self);
        }
        let count = words.len();
        for (index, word) in words.iter_mut().enumerate() {
            *word = match index {
                0 => position.owner,
                1 => position.node,
                2 => position.generation,
                3 => position.phase,
                i if i == count - 1 => tag,
                i => scalars[i - 4],
            };
        }
        Ok(())
    }
    /// Controls compare the production writer with literal words, never a generated oracle.
    #[cfg(test)]
    pub(super) fn encode(self, position: Position) -> [u64; 8] {
        let mut words = [0; 8];
        self.write(
            self.encoding(),
            position,
            &mut words[..self.encoding().bytes().div_ceil(8)],
        )
        .expect("same closed codec");
        words
    }
}
