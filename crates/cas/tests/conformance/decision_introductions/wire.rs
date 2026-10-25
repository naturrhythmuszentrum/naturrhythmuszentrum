// SPDX-License-Identifier: AGPL-3.0-only
//! Independent byte mutations target the retained introduction edge and auxiliary version.

use super::super::decision_baseline::Example;
use super::{frame_draft, membership, prepared_introduction};
use crate::testing::builders::admit;
use crate::testing::common::TestResult;
use crate::testing::readings::ledger;

fn number(bytes: &[u8], position: &mut usize) -> TestResult<usize> {
    let end = position.checked_add(4).ok_or("offset")?;
    let value = u32::from_le_bytes(bytes.get(*position..end).ok_or("number")?.try_into()?);
    *position = end;
    Ok(usize::try_from(value)?)
}

fn wire_inventory(bytes: &[u8]) -> TestResult<Vec<(u8, usize, usize)>> {
    let mut position = 8;
    let manifest = number(bytes, &mut position)?;
    for _ in 0..manifest {
        let count = number(bytes, &mut position)?;
        position += count + 4;
    }
    let count = number(bytes, &mut position)?;
    let mut records = Vec::new();
    for _ in 0..count {
        let tag = *bytes.get(position).ok_or("tag")?;
        position += 1;
        let length = number(bytes, &mut position)?;
        records.push((tag, position, length));
        position += length;
    }
    Ok(records)
}

fn exported_membership() -> TestResult<Vec<u8>> {
    let mut fixture = prepared_introduction(Example::Sign, false)?;
    let source = &mut fixture.partition.source;
    let intro = source
        .cas
        .check_proof(&source.context, &fixture.steps, &mut ledger()?)?;
    let draft = frame_draft(&fixture, &intro);
    let source = &mut fixture.partition.source;
    let frame = admit(&mut source.cas, draft, &mut ledger()?)?;
    let step = membership(&source.cas, &frame, &fixture.option)?;
    let proof = source.cas.check_proof(&frame, &[step], &mut ledger()?)?;
    Ok(source.cas.export_witness(&proof, &mut ledger()?)?)
}

fn refuses(bytes: &[u8]) -> TestResult {
    let mut receiver = nrz_cas::Cas::new(ledger()?.limits().clone())?;
    let before = receiver.counts();
    assert!(receiver.inspect(bytes, &mut ledger()?).is_err());
    assert_eq!(receiver.counts(), before);
    Ok(())
}

/// A valid source-equality proof cannot replace the checked introduction in portable context bytes.
/// ´test:integration:decision-certified-wire-introduction-edge´
#[test]
fn decision_certified_wire_introduction_edge() -> TestResult {
    let original = exported_membership()?;
    let inventory = wire_inventory(&original)?;
    let source = inventory
        .iter()
        .position(|(tag, _, _)| *tag == 11)
        .ok_or("source proof")?;
    let (_, start, _) = inventory
        .iter()
        .find(|(tag, _, _)| *tag == 37)
        .ok_or("certified context")?;
    let mut changed = original.clone();
    changed[start + 6..start + 10].copy_from_slice(&u32::try_from(source)?.to_le_bytes());
    assert_ne!(changed, original);
    refuses(&changed)?;
    changed.clone_from(&original);
    changed[start - 5] = 8;
    refuses(&changed)
}

/// Unknown auxiliary versions fail before a certified frame or membership is published.
/// ´test:integration:decision-certified-wire-version´
#[test]
fn decision_certified_wire_version() -> TestResult {
    let mut bytes = exported_membership()?;
    let inventory = wire_inventory(&bytes)?;
    for (tag, start, _) in inventory {
        if tag != 11 {
            continue;
        }
        let mut position = start + 14;
        let premises = number(&bytes, &mut position)?;
        position += 4 * premises;
        let substitutions = number(&bytes, &mut position)?;
        assert_eq!(substitutions, 0);
        if bytes[position] == 14 {
            bytes[position + 1..position + 9].copy_from_slice(&2_u64.to_le_bytes());
            return refuses(&bytes);
        }
    }
    Err("missing introduction certificate".into())
}

pub(super) fn unknown_version(original: &[u8], certificate: u8) -> TestResult {
    let mut bytes = original.to_vec();
    for (tag, start, _) in wire_inventory(&bytes)? {
        if tag != 11 {
            continue;
        }
        let mut position = start + 14;
        let premises = number(&bytes, &mut position)?;
        position += 4 * premises;
        assert_eq!(number(&bytes, &mut position)?, 0);
        if bytes[position] == certificate {
            bytes[position + 1..position + 9].copy_from_slice(&2_u64.to_le_bytes());
            return refuses(&bytes);
        }
    }
    Err("missing consequence certificate".into())
}
