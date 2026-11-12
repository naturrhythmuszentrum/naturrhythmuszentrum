// SPDX-License-Identifier: AGPL-3.0-only
//! A ten-record literal witness permits a complete independent charge inventory.

use super::common::{TestResult, real};
use super::defined_total::Fixture;
use super::series_residual::admit;
use crate::test_api::domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
use crate::test_api::limits::Limits;
use crate::test_api::syntax::{Draft, Environment, ExpressionDraft, Operator};
use crate::test_api::{Cas, Handle, Rational, Reference};

/// Admit the sole original literal before or after the context, without admitting a predicate.
pub(crate) fn fixture(reordered: bool) -> TestResult<(Fixture, Handle)> {
    let mut cas = Cas::new(Limits::default())?;
    let header = admit(
        &mut cas,
        Draft::Header(HeaderDraft::Core(Environment::default())),
    )?;
    let original = |cas: &mut Cas| {
        admit(
            cas,
            Draft::Expression(ExpressionDraft {
                header: Reference::Existing(header.clone()),
                space: None,
                scope: Vec::new(),
                binding: None,
                sort: real(),
                operator: Operator::Rational(Rational::one()),
                operands: Vec::new(),
            }),
        )
    };
    let early = if reordered {
        Some(original(&mut cas)?)
    } else {
        None
    };
    let region = admit(
        &mut cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::Ambient,
        }),
    )?;
    let context = admit(
        &mut cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header.clone()),
            admissible: Reference::Existing(region.clone()),
            assumptions: Vec::new(),
        }),
    )?;
    let function = if let Some(value) = early {
        value
    } else {
        original(&mut cas)?
    };
    Ok((
        Fixture {
            cas,
            header,
            region,
            context,
            scope: Vec::new(),
        },
        function,
    ))
}

/// Payload lengths in canonical order: two scalar registries, header, region, literal,
/// condition, context, statement, application and conclusion. Each framing adds eleven bytes.
pub(crate) const PAYLOADS: [u64; 10] = [19, 22, 23, 11, 39, 17, 14, 13, 17, 17];

/// Ordinary semantic references in the same independently specified record order.
pub(crate) const REFERENCES: [u64; 10] = [0, 0, 2, 1, 2, 2, 2, 3, 1, 2];

/// Eight magic bytes, three counts, and the sole thirteen-byte rule name and version.
pub(crate) const ENVELOPE: u64 = 8 + 3 * 4 + 4 + 13 + 4;

/// Local application: name/version, complete judgment, premises, substitutions and conditions.
pub(crate) const APPLICATION: u64 = (4 + 13) + 8 + (9 + 2 + 19 + 4) + 4 + 4 + 1 + 4 + 4;

/// Each owned checked application encodes its local payload and copies four payloads plus 256.
pub(crate) const COPY: u64 = APPLICATION + 4 * APPLICATION + 256;

/// A literal metadata comparison writes and compares twenty bytes and encodes two one-limb integers.
pub(crate) const SHAPE: u64 = 2 * (12 + 3 + 4 + 1) + 8 * (1 + 1) * (1 + 1);

/// Exact context/frame matching adds five fixed comparisons to the full metadata comparison.
pub(crate) const FRAME: u64 = SHAPE + 5;

/// One node, no edges: metadata, the visit, eight frame checks, constructor and literal checks.
pub(crate) const VISIT: u64 = SHAPE + 1 + 8 + 1 + 1;

/// The caller owns the original public draft and reference while the producer stages its output.
pub(crate) const PENDING: u64 = (std::mem::size_of::<crate::test_api::proof::ApplicationDraft>()
    + std::mem::size_of::<Draft>()
    + std::mem::size_of::<Reference>()) as u64;

/// Ten distinct descriptors entail 10+9+...+1 readiness visits. The application asks for
/// its thirty-byte statement key nine times, and the conclusion asks once after its proof.
pub(crate) fn seal_work() -> u64 {
    let payload: u64 = PAYLOADS.iter().sum();
    let edges: u64 = REFERENCES.iter().sum();
    let records = payload + 10 * 11;
    let gather_keys = 2 * 44 + 2 * 30;
    let describe = payload + 32;
    let ownership = 16 * payload + 256 * edges;
    let ordering = 55 + 10 * 30 + payload + records;
    3 * COPY + gather_keys + describe + ownership + ordering + ENVELOPE + records
}

/// Two twenty-byte measure passes are prepaid separately from the four fixed object slots.
pub(crate) const MEASURE: u64 = 4 * 128 + 2 * (19 + 1);

/// Local application and proof keys, each written once and probed by key length plus one.
pub(crate) const PUBLICATION: u64 = 2 * (2 + APPLICATION) + 1 + 2 * (1 + 9) + 1;

/// Predicate draft: 29 encoded bytes, its three-copy payload plus 256, checkpoint,
/// one draft visit, one argument edge, two typing comparisons, and a 28-byte key and probe.
pub(crate) const PREDICATE: u64 = 29 + (3 * 29 + 256) + 256 + 1 + 1 + 2 + 28 + 29;

/// Exact native matcher, including the public application preflight and final judgment checks.
pub(crate) const CHECKER: u64 = MEASURE + 5 + FRAME + VISIT + 24 + 3;

/// Independently inventoried record decoding: byte reads, allocations, comparisons, numeric
/// reduction and ordinary admission. Replayed existing regions still debit one region visit.
pub(crate) fn replay_work() -> u64 {
    let payload: u64 = PAYLOADS.iter().sum();
    let record_parts = payload + 10 * 6;
    let bytes = ENVELOPE + payload + 10 * 11;
    let read = bytes + record_parts;
    let outer_allocation = 64 + 2 * 13 + 10 * 512 + 2 * record_parts;
    let inner_allocation = 2 * (11 + 14) + 2 * 64 + 2 * 2 + 64;
    let comparison: u64 = PAYLOADS
        .iter()
        .zip(1..=10)
        .map(|(length, index)| (length + 6) * index)
        .sum();
    // One gcd copy/remainder, two quotients and one canonical magnitude copy, each 32.
    let rational = 5 * 8 * (1 + 1) * (1 + 1);
    // Scalar draft/key lengths 20 and 23: six encoded copies, two checkpoints and a probe.
    let scalars = (6 * 20 + 514) + (6 * 23 + 514);
    // Header draft (7), frozen environment (19), registries (88), header key/probe (69).
    let header = 7 + (3 * 7 + 256) + 256 + 1 + 19 + 88 + 69;
    // Region draft (16), two key/probe pairs (17+18), and the replay region visit.
    let region = 16 + (3 * 16 + 256) + 256 + 1 + 2 * (17 + 18) + 1;
    // Literal draft (35), magnitude copy (32), sort (2), eight-byte scope comparison (72),
    // key (52), second magnitude copy (32), and the successful existing-key probe (53).
    let literal = 35 + 32 + (3 * 35 + 256) + 256 + 1 + 2 + 72 + 52 + 32 + 53;
    // Context draft (25), header comparison, empty hypothesis count, local key/probe (39+40).
    let context = 25 + (3 * 25 + 256) + 256 + 1 + 1 + 4 + 39 + 40;
    // Two generic judgments each visit the statement and compare its two object headers.
    2 * 3
        + 256
        + read
        + outer_allocation
        + inner_allocation
        + comparison
        + rational
        + scalars
        + header
        + region
        + literal
        + PREDICATE
        + context
        + CHECKER
        + PUBLICATION
        + seal_work()
        + 2 * 34
}
