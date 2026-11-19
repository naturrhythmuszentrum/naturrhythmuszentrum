// SPDX-License-Identifier: AGPL-3.0-only
//! Allocation-free terminal predicates retain their admitted reader and sealed domain.
use super::readers::LcResultReader;
use crate::{ExecutionAccount, LoanError};
use nrz_scalar::Scalar;

/// Closed published partition; Pending and Missing never cross publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LcOutcomeKind {
    /// Outside the immutable requested domain.
    NotRequested,
    /// All three numerical companions are available.
    Filled,
    /// An original operand enclosure is strictly negative.
    Invalid,
    /// A required positive-separation barrier remains unresolved.
    Undecided,
    /// A supported finite-format numerical image refused.
    Refused,
}
/// Lossless version-one record with its original logical tuple.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LcCellOutcome {
    /// Original logical tuple index.
    pub cell: usize,
    /// Terminal partition membership.
    pub kind: LcOutcomeKind,
    /// Target code: 0 none, 1 L, 2 C, 3 Product, 4 root, 5 reciprocal/omega.
    pub target: u8,
    /// Closed reason code, never an enum discriminant: 0 none, 1 invalid, 2 undecided.
    /// Domain causes are 0x0102 zero divisor, 0x0103 negative root, 0x0104 unbounded image.
    /// Arithmetic causes 0x0201..0x0205 denote nonfinite, inverted endpoints, zero divisor,
    /// negative root and overflow; 0x0401..0x0405 preserve the same causes inside accuracy.
    /// Invalid predecessor radius is 0x0301, attributed to the consuming target operation.
    pub reason: u16,
}
/// A predicate borrows an authenticated reader; it allocates no independent bitmap.
#[derive(Debug)]
pub struct LcOutcomeView<'r, 'a, P: Scalar> {
    /// Reader retains the same immutable bundle and original account identity.
    pub(super) reader: &'r LcResultReader<'a, P>,
    /// Fixed terminal predicate, evaluated only within demand.
    pub(super) kind: LcOutcomeKind,
}
impl<P: Scalar + 'static> LcOutcomeView<'_, '_, P> {
    /// Inspect one tuple on the original ledger; outside demand is false.
    /// Views are intersections with demand. `NotRequested` therefore denotes the empty view;
    /// use `LcResultReader::outcome` to inspect cells outside demand.
    /// # Errors
    /// Refuses foreign accounts, bounds or insufficient Work before inspection.
    pub fn contains(&self, cell: usize, account: &mut ExecutionAccount) -> Result<bool, LoanError> {
        let outcome = self.reader.outcome(cell, account)?;
        Ok(outcome.kind != LcOutcomeKind::NotRequested && outcome.kind == self.kind)
    }
}
/// The finite vocabulary admits only the target-specific lossless cause combinations.
pub(super) const fn decode(
    record: super::super::super::LcOutcome,
    cell: usize,
) -> Result<LcCellOutcome, LoanError> {
    let kind = match (record.status, record.target, record.reason) {
        (0, 0, 0) => LcOutcomeKind::NotRequested,
        (2, 0, 0) => LcOutcomeKind::Filled,
        (3, 1 | 2, 1) => LcOutcomeKind::Invalid,
        (4, 1..=4, 2) => LcOutcomeKind::Undecided,
        (5, 3..=5, 0x0201 | 0x0202 | 0x0205 | 0x0301 | 0x0401 | 0x0402 | 0x0405 | 0x0104)
        | (5, 4, 0x0103 | 0x0204 | 0x0404)
        | (5, 5, 0x0102 | 0x0203 | 0x0403) => LcOutcomeKind::Refused,
        _ => return Err(LoanError::CoverageMismatch),
    };
    Ok(LcCellOutcome {
        cell,
        kind,
        target: record.target,
        reason: record.reason,
    })
}
