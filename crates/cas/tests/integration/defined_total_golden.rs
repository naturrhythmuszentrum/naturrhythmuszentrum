// SPDX-License-Identifier: AGPL-3.0-only
//! The complete literal witness is authored from its record grammar, independently of production.

use super::series_golden::{
    Part, Record, bytes, count, encode_records, expression, join, name, number,
};
use crate::testing::common::{TestResult, default_budget};
use crate::testing::defined_total_minimal as inventory;

const fn record(key: &'static str, tag: u8, parts: Vec<Part>) -> Record {
    Record {
        name: key,
        tag,
        layer: 0,
        parts,
    }
}

fn records() -> Vec<Record> {
    vec![
        record(
            "real",
            34,
            vec![bytes(join(&[name("scalar.real"), count(1)]))],
        ),
        record(
            "complex",
            34,
            vec![bytes(join(&[name("scalar.complex"), count(1)]))],
        ),
        record(
            "header",
            0,
            vec![
                bytes(join(&[vec![0, 1, 0], count(0), count(2)])),
                Part::Ref("real"),
                Part::Ref("complex"),
                bytes(count(0)),
            ],
        ),
        record(
            "region",
            1,
            vec![Part::Ref("header"), bytes(join(&[count(0), vec![0, 0, 0]]))],
        ),
        expression("one", join(&[vec![0], number(false, 1, 1)])),
        record(
            "defined",
            7,
            vec![
                Part::Ref("header"),
                bytes(join(&[count(0), vec![0], count(1)])),
                Part::Ref("one"),
            ],
        ),
        record(
            "context",
            8,
            vec![
                Part::Ref("header"),
                bytes(vec![0]),
                Part::Ref("region"),
                bytes(join(&[vec![0], count(0)])),
            ],
        ),
        record(
            "statement",
            10,
            vec![
                Part::Ref("context"),
                bytes(vec![1]),
                Part::Ref("region"),
                Part::Ref("defined"),
            ],
        ),
        record(
            "application",
            11,
            vec![
                bytes(count(0)),
                Part::Ref("statement"),
                bytes(join(&[count(0), count(0), vec![0]])),
            ],
        ),
        record(
            "conclusion",
            12,
            vec![
                bytes(vec![1]),
                Part::Ref("application"),
                Part::Ref("statement"),
                bytes(join(&[count(0), count(0)])),
            ],
        ),
    ]
}

#[test]
fn defined_total_independent_golden() -> TestResult {
    let records = records();
    for ((record, payload), references) in records
        .iter()
        .zip(inventory::PAYLOADS)
        .zip(inventory::REFERENCES)
    {
        let (length, edges) = record
            .parts
            .iter()
            .fold((0_u64, 0_u64), |(length, edges), part| match part {
                Part::Bytes(value) => (length + value.len() as u64, edges),
                Part::Ref(_) => (length + 4, edges + 1),
            });
        assert_eq!((length, edges), (payload, references), "{}", record.name);
    }
    let golden = encode_records(&records, join(&[count(1), name("defined.total"), count(1)]))?;
    assert_eq!(
        golden.len() as u64,
        inventory::ENVELOPE + inventory::PAYLOADS.iter().sum::<u64>() + 10 * 11
    );
    let (mut source, original) = inventory::fixture(false)?;
    let witness = source
        .cas
        .defined_total(&original, &source.context, &mut default_budget()?)?;
    assert_eq!(
        source
            .cas
            .export_witness(&witness, &mut default_budget()?)?,
        golden
    );
    let (mut receiver, original) = inventory::fixture(true)?;
    let expected = receiver.expected(&original)?;
    let replayed = receiver
        .cas
        .replay(&golden, &expected, &mut default_budget()?)?;
    assert!(replayed.is_unconditional());
    assert_eq!(replayed.judgment(), &expected);
    Ok(())
}
